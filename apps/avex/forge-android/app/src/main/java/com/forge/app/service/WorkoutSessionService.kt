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
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
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

    private val serviceScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    private var currentState: SessionNotifState? = null
    private var initialized = false

    companion object {
        const val CHANNEL_SESSION = "forge_session"
        const val CHANNEL_TIMER = "forge_timer"
        private const val NOTIF_SESSION = 1
        private const val NOTIF_TIMER = 2

        fun createChannels(context: Context) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_SESSION,
                    "Workout Session",
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "Shown while a workout is in progress"
                    setShowBadge(false)
                }
            )
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_TIMER,
                    "Rest Timer",
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = "Alerts when the rest timer finishes"
                }
            )
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Show placeholder notification immediately — required before any async work
        val placeholder = buildSessionNotification("Workout", "Starting…")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            startForeground(NOTIF_SESSION, placeholder, ServiceInfo.FOREGROUND_SERVICE_TYPE_HEALTH)
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

            // Handle timer-done events: vibrate + post alert notification (#16)
            serviceScope.launch {
                bridge.timerDone.collect {
                    postTimerDoneNotification()
                }
            }

            // Refresh elapsed time in the session notification every 60 s (#48)
            serviceScope.launch {
                while (true) {
                    delay(60_000)
                    currentState?.let { updateSessionNotification(it) }
                }
            }
        }

        return START_NOT_STICKY
    }

    private fun updateSessionNotification(state: SessionNotifState) {
        val elapsedMin = ((System.currentTimeMillis() - state.startedAtMs) / 60_000).toInt()
            .coerceAtLeast(0)
        val text = if (elapsedMin < 1) "Just started" else "$elapsedMin min"
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIF_SESSION, buildSessionNotification(state.dayName, text))
    }

    private fun buildSessionNotification(title: String, text: String): Notification {
        val tapIntent = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        return NotificationCompat.Builder(this, CHANNEL_SESSION)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(tapIntent)
            .setOngoing(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_STATUS)
            .build()
    }

    private fun postTimerDoneNotification() {
        vibratePhone()
        val tapIntent = PendingIntent.getActivity(
            this, 1,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notification = NotificationCompat.Builder(this, CHANNEL_TIMER)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle("Rest timer done")
            .setContentText("Time to get back to it")
            .setContentIntent(tapIntent)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .build()
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIF_TIMER, notification)
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
