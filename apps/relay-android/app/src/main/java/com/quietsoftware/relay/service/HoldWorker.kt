package com.quietsoftware.relay.service

import android.app.Notification
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
import androidx.work.OneTimeWorkRequest
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * A hold's Allow once or Deny from its notification, done here rather than in [ActionReceiver]:
 * reaching the PC can take 15 s and the confirm runs the held command, far past what a
 * broadcast may take before the system calls it unresponsive. Expedited, so it starts at once.
 */
@HiltWorker
class HoldWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val relay: Relay,
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val action = inputData.getString(ACTION) ?: return Result.success()
        val hold = inputData.getLong(ActionReceiver.EXTRA_HOLD, -1)
        val notification = inputData.getInt(ActionReceiver.EXTRA_NOTIFICATION, 0)
        if (hold < 0) return Result.success()
        try {
            if (!online()) {
                unreachable(applicationContext, notification)
                return Result.success()
            }
            val allow = action == ActionReceiver.ACTION_ALLOW
            val op = inputData.getString(ActionReceiver.EXTRA_OP)
            val payload = buildJsonObject {
                put("hold_id", hold)
                if (!allow) put("reason", "Denied from the phone's notification")
                // An exception request is let through for one use from here, whatever the agent
                // asked for: the rest of a session is chosen in the app, where it says so.
                if (allow && op == "guardrail.request") put("scope", "once")
            }
            // The exact held request is read before it is answered, as on the PC: still open, and
            // the one the notification showed (a hold id from another PC, or one already
            // answered, is not allowed or denied blind). Either way the notification goes, since
            // its keys can do nothing more.
            val read = (relay.call("guardrail.hold.get", buildJsonObject { put("hold_id", hold) }) as? JsonObject)?.o("hold")
            val same = read != null && read.s("state") == "open" &&
                read.s("op") == op && read.s("created_at") == inputData.getString(ActionReceiver.EXTRA_CREATED)
            if (same) relay.call(if (allow) "guardrail.confirm" else "guardrail.reject", payload)
            if (notification != 0) NotificationManagerCompat.from(applicationContext).cancel(notification)
        } catch (e: BusException) {
            // The PC went out of reach on the way: say so. Any other refusal, opening the app shows why.
            if (e.error.transient) unreachable(applicationContext, notification)
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (_: Exception) {
            // The notification stays; opening the app shows why.
        }
        return Result.success()
    }

    /** Before Android 12 expedited work runs as a foreground service, which shows this. */
    override suspend fun getForegroundInfo(): ForegroundInfo = ForegroundInfo(
        WORKING_ID,
        NotificationCompat.Builder(applicationContext, Notifier.CH_LINK)
            .setSmallIcon(R.drawable.ic_stat_relay)
            .setContentTitle("Answering the PC")
            .setOngoing(true)
            .build(),
    )

    private suspend fun online(): Boolean {
        relay.start()
        relay.kick()
        return withTimeoutOrNull(15_000) { relay.state.first { it is LinkState.Online } } != null
    }

    companion object {
        private const val ACTION = "action"
        private const val WORKING_ID = 3

        /** The work for a hold key's [intent], as [ActionReceiver] got it. */
        fun request(intent: Intent): OneTimeWorkRequest = OneTimeWorkRequestBuilder<HoldWorker>()
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .setInputData(
                Data.Builder()
                    .putString(ACTION, intent.action)
                    .putLong(ActionReceiver.EXTRA_HOLD, intent.getLongExtra(ActionReceiver.EXTRA_HOLD, -1))
                    .putString(ActionReceiver.EXTRA_OP, intent.getStringExtra(ActionReceiver.EXTRA_OP))
                    .putString(ActionReceiver.EXTRA_CREATED, intent.getStringExtra(ActionReceiver.EXTRA_CREATED))
                    .putInt(ActionReceiver.EXTRA_NOTIFICATION, intent.getIntExtra(ActionReceiver.EXTRA_NOTIFICATION, 0))
                    .build(),
            )
            .build()

        /** One answer per hold: a second tap while the first is on its way does not send another. */
        fun enqueue(context: Context, intent: Intent) {
            val hold = intent.getLongExtra(ActionReceiver.EXTRA_HOLD, -1)
            WorkManager.getInstance(context).enqueueUniqueWork("hold-$hold", ExistingWorkPolicy.KEEP, request(intent))
        }

        /**
         * Notification [id] again, saying its key could not reach the PC; a tap still opens Relay
         * where it did. Without the keys, which could only fail the same way.
         */
        fun unreachable(context: Context, id: Int) {
            if (id == 0) return
            val shown = context.getSystemService(NotificationManager::class.java)?.activeNotifications?.firstOrNull { it.id == id }?.notification ?: return
            val text = "Could not reach the PC. Open Relay to answer."
            val again = NotificationCompat.Builder(context, shown.channelId ?: Notifier.CH_ATTENTION)
                .setSmallIcon(R.drawable.ic_stat_relay)
                .setColor(shown.color)
                .setAutoCancel(true)
                .setOnlyAlertOnce(true)
                .setContentTitle(shown.extras.getCharSequence(Notification.EXTRA_TITLE))
                .setContentText(text)
                .setStyle(NotificationCompat.BigTextStyle().bigText(text))
                .setContentIntent(shown.contentIntent)
                .build()
            try {
                NotificationManagerCompat.from(context).notify(id, again)
            } catch (_: SecurityException) {
                // Notifications were taken back meanwhile.
            }
        }
    }
}
