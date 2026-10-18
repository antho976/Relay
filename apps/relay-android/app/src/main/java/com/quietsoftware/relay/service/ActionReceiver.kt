package com.quietsoftware.relay.service

import android.app.Notification
import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.data.Relay
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import javax.inject.Inject

/**
 * What a notification's keys do without opening the app: deny a held command, allow it once
 * (the system asks for the phone's unlock first, `setAuthenticationRequired`), or answer a
 * blocked agent with priority mail, which it reads at its next step. What a full-access thread's
 * agent asks to do is denied or allowed once the same way. A hold's or an ask's key that cannot
 * reach the PC says so on its notification.
 */
@AndroidEntryPoint
class ActionReceiver : BroadcastReceiver() {
    @Inject lateinit var relay: Relay

    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        val notification = intent.getIntExtra(EXTRA_NOTIFICATION, 0)
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            try {
                when (intent.action) {
                    ACTION_DENY, ACTION_ALLOW -> {
                        val hold = intent.getLongExtra(EXTRA_HOLD, -1)
                        if (hold < 0) return@launch
                        if (!online()) {
                            unreachable(context, notification)
                            return@launch
                        }
                        val payload = buildJsonObject {
                            put("hold_id", hold)
                            if (intent.action == ACTION_DENY) put("reason", "Denied from the phone's notification")
                            // An exception request is let through for one use from here, whatever the agent
                            // asked for: the rest of a session is chosen in the app, where it says so.
                            if (intent.action == ACTION_ALLOW && intent.getStringExtra(EXTRA_OP) == "guardrail.request") put("scope", "once")
                        }
                        // The exact held request is read before it is answered, as on the PC: still
                        // open, and the one the notification showed (a hold id from another PC, or one
                        // already answered, is not allowed or denied blind). Either way the
                        // notification goes, since its keys can do nothing more.
                        val read = (relay.call("guardrail.hold.get", buildJsonObject { put("hold_id", hold) }) as? JsonObject)?.o("hold")
                        val same = read != null && read.s("state") == "open" &&
                            read.s("op") == intent.getStringExtra(EXTRA_OP) && read.s("created_at") == intent.getStringExtra(EXTRA_CREATED)
                        if (same) relay.call(if (intent.action == ACTION_ALLOW) "guardrail.confirm" else "guardrail.reject", payload)
                    }
                    ACTION_ASK_DENY, ACTION_ASK_ALLOW -> {
                        val ask = intent.getLongExtra(EXTRA_ASK, -1)
                        if (ask < 0) return@launch
                        if (!online()) {
                            unreachable(context, notification)
                            return@launch
                        }
                        // Read again before it is answered, as a hold is: still waiting, and the one
                        // the notification showed. Either way the notification goes.
                        val thread = intent.getLongExtra(EXTRA_THREAD, -1)
                        val waiting = (relay.call("thread.ask.list", buildJsonObject {
                            if (thread >= 0) put("thread_id", thread)
                            put("pending", true)
                        }) as? JsonObject)?.arr("asks")?.firstOrNull { (it as? JsonObject)?.l("id") == ask } as? JsonObject
                        if (waiting != null && waiting.s("created_at") == intent.getStringExtra(EXTRA_CREATED)) {
                            val allow = intent.action == ACTION_ASK_ALLOW
                            try {
                                relay.call("thread.ask.answer", buildJsonObject {
                                    put("id", ask)
                                    put("allow", allow)
                                    // Once from here, whatever the key could have said: the rest of the thread is chosen in the app.
                                    if (allow) put("scope", "once")
                                })
                            } catch (e: BusException) {
                                // Answered on the PC meanwhile, or the agent stopped first: nothing left to answer.
                                if (e.error.code != "thread.ask_answered" && e.error.code != "thread.ask_not_found") throw e
                            }
                        }
                    }
                    ACTION_REPLY -> {
                        val text = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(KEY_REPLY)?.toString()?.trim().orEmpty()
                        val session = intent.getStringExtra(EXTRA_SESSION) ?: return@launch
                        val project = intent.getLongExtra(EXTRA_PROJECT, -1)
                        if (text.isEmpty() || project < 0) return@launch
                        // Through the outbox: with the PC away it goes when the PC is back.
                        relay.change("mailbox.send", buildJsonObject {
                            put("project_id", project)
                            put("to", session)
                            put("text", text)
                            put("priority", true)
                        }, "Reply to $session")
                    }
                }
                if (notification != 0) NotificationManagerCompat.from(context).cancel(notification)
            } catch (e: BusException) {
                // The PC went out of reach on the way: say so. Any other refusal, opening the app shows why.
                if (e.error.transient) unreachable(context, notification)
            } catch (_: Exception) {
                // The notification stays; opening the app shows why.
            } finally {
                pending.finish()
            }
        }
    }

    /**
     * Notification [id] again, saying its key could not reach the PC; a tap still opens Relay where
     * it did. Without the keys, which could only fail the same way.
     */
    private fun unreachable(context: Context, id: Int) {
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

    private suspend fun online(): Boolean {
        relay.start()
        relay.kick()
        return withTimeoutOrNull(15_000) { relay.state.first { it is LinkState.Online } } != null
    }

    companion object {
        const val ACTION_ALLOW = "com.quietsoftware.relay.HOLD_ALLOW"
        const val ACTION_DENY = "com.quietsoftware.relay.HOLD_DENY"
        const val ACTION_REPLY = "com.quietsoftware.relay.REPLY"
        const val ACTION_ASK_ALLOW = "com.quietsoftware.relay.ASK_ALLOW"
        const val ACTION_ASK_DENY = "com.quietsoftware.relay.ASK_DENY"
        const val EXTRA_ASK = "ask"
        const val EXTRA_THREAD = "thread"
        const val EXTRA_HOLD = "hold"
        const val EXTRA_OP = "hold_op"
        const val EXTRA_CREATED = "hold_created"
        const val EXTRA_SESSION = "session"
        const val EXTRA_PROJECT = "project"
        const val EXTRA_NOTIFICATION = "notification"
        const val KEY_REPLY = "reply"
    }
}
