package com.forge.wear.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.os.Build
import android.os.SystemClock
import androidx.health.services.client.ExerciseUpdateCallback
import androidx.health.services.client.HealthServices
import androidx.health.services.client.data.Availability
import androidx.health.services.client.data.DataType
import androidx.health.services.client.data.ExerciseConfig
import androidx.health.services.client.data.ExerciseLapSummary
import androidx.health.services.client.data.ExerciseType
import androidx.health.services.client.data.ExerciseUpdate
import com.forge.shared.protocol.HrBatchDto
import com.forge.wear.WearHealthPermissions
import com.forge.wear.data.WearDataRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import androidx.concurrent.futures.await
import kotlinx.coroutines.launch
import java.time.Instant

/**
 * Live HR during a session (W3): a health foreground service holding a Health Services
 * [ExerciseType.STRENGTH_TRAINING] exercise while the phone session is active. Sensor-fused HR
 * (+ the exercise's cumulative calories) batches to the phone every ~5 s over /hr/batch — active
 * session only, never a daily stream. Degrades quietly: no heart-rate permission, a registration
 * failure or another app owning the exercise (ExerciseClient is exclusive) just means no HR — the
 * session itself is untouched.
 *
 * "No heart-rate permission" is api-level-dependent — see [WearHealthPermissions]. It is checked in
 * [onStartCommand] rather than only by the caller, because START_STICKY hands the system a licence
 * to recreate this service without one.
 */
class WearHrService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val pending = ArrayDeque<HrBatchDto.Sample>()
    @Volatile private var totalKcal: Double? = null
    @Volatile private var sessionId: Long = -1L
    private var exerciseStarted = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        sessionId = intent?.getLongExtra(EXTRA_SESSION_ID, -1L) ?: -1L
        if (sessionId <= 0) { stopSelf(); return START_NOT_STICKY }
        // Checked HERE, not only at the call site. START_STICKY means the system recreates this
        // service on its own after the process is reclaimed, and a Data Layer wake can start it
        // from a path that never consulted the permission at all — by which time the user may have
        // revoked it. From API 34 a health-typed foreground service whose app holds none of the
        // health permissions throws SecurityException out of startForeground, which lands as a
        // crash in a background process nobody is watching. No permission is simply no HR.
        if (!WearHealthPermissions.canStreamHeartRate(this)) {
            stopSelf()
            return START_NOT_STICKY
        }
        startInForeground()
        if (!exerciseStarted) {
            exerciseStarted = true
            scope.launch { runExercise() }
            scope.launch { batchLoop() }
        }
        return START_STICKY
    }

    private fun startInForeground() {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Heart rate", NotificationManager.IMPORTANCE_LOW)
        )
        val notification: Notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Heart rate")
            .setContentText("Streaming during your session")
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_HEALTH)
        } else {
            startForeground(NOTIF_ID, notification)
        }
    }

    private suspend fun runExercise() {
        val client = HealthServices.getClient(this).exerciseClient
        val callback = object : ExerciseUpdateCallback {
            override fun onExerciseUpdateReceived(update: ExerciseUpdate) {
                val bootInstant = Instant.ofEpochMilli(System.currentTimeMillis() - SystemClock.elapsedRealtime())
                update.latestMetrics.getData(DataType.HEART_RATE_BPM).forEach { point ->
                    val bpm = point.value.toInt()
                    val atMs = point.getTimeInstant(bootInstant).toEpochMilli()
                    if (bpm > 0) synchronized(pending) {
                        pending.addLast(HrBatchDto.Sample(atMs = atMs, bpm = bpm))
                        while (pending.size > PENDING_CAP) pending.removeFirst()
                    }
                    liveBpm.value = bpm
                }
                update.latestMetrics.getData(DataType.CALORIES_TOTAL)?.let { cumulative ->
                    totalKcal = cumulative.total
                }
            }

            override fun onLapSummaryReceived(lapSummary: ExerciseLapSummary) = Unit
            override fun onRegistered() = Unit
            override fun onRegistrationFailed(throwable: Throwable) { stopSelf() }
            override fun onAvailabilityChanged(dataType: DataType<*, *>, availability: Availability) = Unit
        }
        runCatching {
            client.setUpdateCallback(callback)
            client.startExerciseAsync(
                ExerciseConfig(
                    exerciseType = ExerciseType.STRENGTH_TRAINING,
                    dataTypes = setOf(DataType.HEART_RATE_BPM, DataType.CALORIES_TOTAL),
                    isAutoPauseAndResumeEnabled = false,
                    isGpsEnabled = false
                )
            ).await()
        }.onFailure {
            // Another app owns the exercise, or sensors refused — degrade to no-HR, never retry-loop.
            stopSelf()
        }
    }

    private suspend fun batchLoop() {
        val repo = WearDataRepository.instance(this)
        while (true) {
            delay(BATCH_INTERVAL_MS)
            // TAKE a snapshot; don't clear yet. The buffer used to be emptied BEFORE the send was
            // known to have worked, and the send swallows failure — so three minutes out of
            // Bluetooth range silently lost ~36 batches, while the PENDING_CAP buffer that exists to
            // ride out exactly that was emptied before it could help. The samples are keyed
            // (session_id, at_ms) with IGNORE-on-conflict, so a re-send is free.
            val batch = synchronized(pending) { pending.toList() }
            if (batch.isEmpty()) continue
            if (!repo.sendHrBatchAwait(sessionId, batch, totalKcal)) continue
            // Drop the samples that were actually delivered, BY IDENTITY. Removing batch.size
            // items from the front instead assumed the front of the deque was still the snapshot,
            // which stops being true the moment PENDING_CAP evicts: a send that takes long enough
            // for the buffer to fill — the slow-delivery case this buffer exists for — has already
            // dropped some of the snapshot off the front, so counting off batch.size then deletes
            // that many NEWLY ARRIVED samples which were never sent at all. Samples are keyed
            // (session_id, at_ms) with IGNORE-on-conflict on the phone, so matching on at_ms is
            // exactly as precise as the storage is.
            val delivered = batch.mapTo(HashSet(batch.size)) { it.atMs }
            synchronized(pending) { pending.removeAll { it.atMs in delivered } }
        }
    }

    override fun onDestroy() {
        scope.launch {
            runCatching { HealthServices.getClient(this@WearHrService).exerciseClient.endExerciseAsync().await() }
            scope.cancel()
        }
        liveBpm.value = null
        super.onDestroy()
    }

    companion object {
        private const val CHANNEL = "avex_hr"
        private const val NOTIF_ID = 101
        private const val BATCH_INTERVAL_MS = 5_000L
        private const val PENDING_CAP = 240
        const val EXTRA_SESSION_ID = "session_id"

        /** The latest wrist reading — the set screen's live "· N BPM" line. Null = not streaming. */
        val liveBpm = MutableStateFlow<Int?>(null)

        fun start(context: Context, sessionId: Long) {
            runCatching {
                context.startForegroundService(
                    Intent(context, WearHrService::class.java).putExtra(EXTRA_SESSION_ID, sessionId)
                )
            }
        }

        fun stop(context: Context) {
            runCatching { context.stopService(Intent(context, WearHrService::class.java)) }
        }
    }
}

/** Read-only view for the UI. */
val wearLiveBpm: StateFlow<Int?> get() = WearHrService.liveBpm
