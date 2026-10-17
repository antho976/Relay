package com.forge.app.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import androidx.core.app.NotificationCompat
import com.forge.app.MainActivity
import com.forge.app.R
import com.forge.app.data.prefs.SettingsRepository
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Foreground service that keeps the process alive during an active workout so the
 * rest timer alert fires reliably even when the phone is pocketed.
 *
 * Driven by [WorkoutSessionBridge]:
 *  - Shows a persistent session notification (#48) updated every 60 s.
 *  - Vibrates and posts a "Rest timer done" notification on each [WorkoutSessionBridge.timerDone]
 *    event (#16).
 *  - Calls [stopSelf] when [WorkoutSessionBridge.sessionState] becomes null (session ended).
 */
@AndroidEntryPoint
class WorkoutSessionService : Service() {

    @Inject lateinit var bridge: WorkoutSessionBridge
    @Inject lateinit var settingsRepo: SettingsRepository
    @Inject lateinit var wearConnection: com.forge.app.service.wear.WearConnection
    @Inject lateinit var wearPublisher: com.forge.app.service.wear.WearStatePublisher
    @Inject lateinit var timerHolder: com.forge.app.service.wear.SessionTimerHolder

    private val serviceScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    private var currentState: SessionNotifState? = null
    /** The rest the session notification is showing, as of its last structural change. */
    private var currentRest: com.forge.app.domain.timer.RestTimerState? = null
    private var initialized = false
    /** Cached per-type opt-out (N2), kept live by a collector — defaults to the pref's own default. */
    private var restTimerAlertEnabled = true

    companion object {
        const val CHANNEL_SESSION = "forge_session"
        const val CHANNEL_TIMER = "forge_timer"
        private const val NOTIF_SESSION = 1
        private const val NOTIF_TIMER = 2
        /** How long the phone waits for the wrist's timer-done haptic ack before buzzing itself. */
        private const val HAPTIC_ACK_GRACE_MS = 2_500L
        /** Ack recency accepted as "the wrist buzzed for THIS timer" (covers the grace + BT lag). */
        private const val HAPTIC_ACK_WINDOW_MS = 8_000L

        fun createChannels(context: Context) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_SESSION,
                    "Workout session",
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "Shown while a workout is in progress"
                    setShowBadge(false)
                }
            )
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_TIMER,
                    "Rest timer",
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = "Alerts when the rest timer finishes"
                }
            )
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Show a notification immediately — required before any async work. A repeat start (the
        // service already running, a new sitting) shows the live session rather than regressing it
        // to "Starting…": the collector below may already have posted it, and would not re-post
        // until the next minute's refresh.
        val placeholder = bridge.sessionState.value?.let(::sessionNotification)
            ?: buildSessionNotification("Workout", "Starting…")
        // The typed startForeground is only required on API 34+, where the "specialUse"
        // FGS type is defined. "specialUse" needs no runtime permission, unlike "health"
        // (which threw SecurityException on Android 14+ because we hold no health permission).
        // On older APIs the 2-arg form is correct.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIF_SESSION, placeholder, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIF_SESSION, placeholder)
        }

        if (!initialized) {
            initialized = true

            // Track session state; stop service when null (session ended)
            serviceScope.launch {
                bridge.sessionState.collect { state ->
                    if (state == null) {
                        stopSelf()
                        return@collect
                    }
                    currentState = state
                    updateSessionNotification(state)
                }
            }

            // Keep the per-type opt-out cached + live (N2) so the timer-done path is a plain boolean
            // check, not a DataStore read on every event.
            serviceScope.launch {
                settingsRepo.restTimerAlertEnabled.collect { restTimerAlertEnabled = it }
            }

            // Handle timer-done events: vibrate + post alert notification (#16)
            //
            // Each event is handled in its OWN coroutine, so the collector never parks. The grace
            // wait below used to run inside `collect`, and the bridge's flow has a one-slot buffer
            // whose tryEmit result is discarded — so a second timer expiring inside that 2.5 s
            // window (a short-rest circuit, or a skipped timer restarted immediately) was dropped
            // silently. The user was left waiting on a cue that would never come, which is the exact
            // failure this foreground service exists to prevent.
            serviceScope.launch {
                bridge.timerDone.collect { serviceScope.launch { handleTimerDone() } }
            }

            // The rest countdown on the lock screen: with the phone pocketed between sets the
            // notification only said "23 min", once a minute, and the time left on the rest was
            // invisible until the done buzz. The system draws the countdown (a chronometer), so the
            // notification is re-posted only on a structural change (start, pause, resume, ±30 s,
            // stop, done), never on the timer's 1 Hz tick: see [RestNotifKey].
            serviceScope.launch {
                timerHolder.controller.state
                    .map { rest -> RestNotifKey.of(rest, System.currentTimeMillis()) to rest }
                    .distinctUntilChanged { a, b -> a.first.sameAs(b.first) }
                    .collect { (_, rest) ->
                        currentRest = rest
                        currentState?.let { updateSessionNotification(it) }
                    }
            }
        }

        return START_NOT_STICKY
    }

    private fun updateSessionNotification(state: SessionNotifState) {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIF_SESSION, sessionNotification(state))
    }

    private fun sessionNotification(state: SessionNotifState): Notification {
        val now = System.currentTimeMillis()
        val rest = currentRest?.takeIf { !it.isFinished }
        return when {
            rest != null && !rest.isPaused -> buildSessionNotification(
                "Rest · ${state.dayName}", "${formatRest(rest.totalSeconds)} rest", state.dayKey,
                chronometerAtMs = now + rest.secondsRemaining * 1000L, countDown = true
            )
            rest != null -> buildSessionNotification(
                "Rest paused · ${state.dayName}", "${formatRest(rest.secondsRemaining)} left", state.dayKey
            )
            // No rest running: the workout's own clock, counting up from its active-time anchor.
            else -> buildSessionNotification(state.dayName, "In progress", state.dayKey, chronometerAtMs = state.startedAtMs)
        }
    }

    private fun formatRest(seconds: Int): String = "%d:%02d".format(seconds / 60, seconds % 60)

    /** Opens the workout's own day (the widget's deep link), or the app when the day is unknown. */
    private fun openWorkoutIntent(requestCode: Int, dayKey: String?): PendingIntent = PendingIntent.getActivity(
        this, requestCode,
        Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
            dayKey?.let { putExtra(com.forge.app.widget.EXTRA_START_DAY_KEY, it) }
        },
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    )

    private fun buildSessionNotification(
        title: String,
        text: String,
        dayKey: String? = null,
        chronometerAtMs: Long? = null,
        countDown: Boolean = false
    ): Notification {
        return NotificationCompat.Builder(this, CHANNEL_SESSION)
            .setSmallIcon(R.drawable.ic_stat_forge)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(openWorkoutIntent(0, dayKey))
            .setOngoing(true)
            .setSilent(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_STATUS)
            .apply {
                if (chronometerAtMs != null) {
                    setWhen(chronometerAtMs)
                    setShowWhen(true)
                    setUsesChronometer(true)
                    setChronometerCountDown(countDown)
                }
            }
            .build()
    }

    private fun postTimerDoneNotification() {
        vibratePhone()
        val tapIntent = openWorkoutIntent(1, currentState?.dayKey)
        val notification = NotificationCompat.Builder(this, CHANNEL_TIMER)
            .setSmallIcon(R.drawable.ic_stat_forge)
            .setContentTitle("Rest done")
            .setContentText(currentState?.dayName?.let { "Next set · $it" } ?: "Next set")
            .setContentIntent(tapIntent)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .build()
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIF_TIMER, notification)
    }


    private suspend fun handleTimerDone() {
        // N2: respect the per-type opt-out (buzz + notify when the app is backgrounded).
        if (!restTimerAlertEnabled) return
        // Haptic handoff (W1, DESIGN.md §16): when the watch is reachable, IT owns the timer-done
        // buzz — the phone waits a short grace for the wrist's ack and stays silent when it lands.
        // No ack (watch app dead, BT dropped at the boundary) → the phone buzzes late rather than
        // nobody buzzing.
        // Read the timer's identity BEFORE the grace period: by the time the wrist's ack could
        // arrive, the controller may have stopped and the publisher cleared it.
        val timerEndAtMs = wearPublisher.lastPublishedTimerEndAtMs
        val watchReachable = wearConnection.reachableWearNodeId() != null
        if (watchReachable) {
            kotlinx.coroutines.delay(HAPTIC_ACK_GRACE_MS)
            // Matched on WHICH timer the wrist buzzed for, not merely on recency. Recency alone let
            // an ack for the rest that just ended silence a rest started seconds later, inside the
            // eight-second Bluetooth-lag window — and since the wrist had already buzzed for the
            // old timer, neither device buzzed for the new one.
            if (wearConnection.hapticAckedFor(timerEndAtMs, HAPTIC_ACK_WINDOW_MS, System.currentTimeMillis())) return
        }
        // Quiet hours: still buzz (you're mid-workout and want the cue) but skip the heads-up
        // notification so the phone stays visually silent.
        if (settingsRepo.isQuietNow()) vibratePhone() else postTimerDoneNotification()
    }

    private fun vibratePhone() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val vm = getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as VibratorManager
            vm.defaultVibrator.vibrate(
                VibrationEffect.createWaveform(longArrayOf(0, 300, 100, 300), -1)
            )
        } else {
            @Suppress("DEPRECATION")
            val v = getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
            v.vibrate(VibrationEffect.createWaveform(longArrayOf(0, 300, 100, 300), -1))
        }
    }

    override fun onDestroy() {
        serviceScope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}

/**
 * What the session notification shows about the rest, reduced to the parts that change its
 * content. A running rest is keyed on its END instant (now + remaining), which stays put while the
 * timer ticks, so a once-a-second state only re-posts when the end actually moves (±30 s, a reset)
 * or the rest pauses, resumes, finishes or stops.
 */
internal data class RestNotifKey(val active: Boolean, val paused: Boolean, val value: Long) {
    /** Equal for notification purposes: a running end may jitter by a second at a tick boundary. */
    fun sameAs(other: RestNotifKey): Boolean =
        active == other.active && paused == other.paused &&
            (if (active && !paused) kotlin.math.abs(value - other.value) <= 1_500 else value == other.value)

    companion object {
        fun of(rest: com.forge.app.domain.timer.RestTimerState?, nowMs: Long): RestNotifKey = when {
            rest == null || rest.isFinished -> RestNotifKey(active = false, paused = false, value = 0)
            rest.isPaused -> RestNotifKey(active = true, paused = true, value = rest.secondsRemaining.toLong())
            else -> RestNotifKey(active = true, paused = false, value = nowMs + rest.secondsRemaining * 1000L)
        }
    }
}
