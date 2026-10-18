package com.quietsoftware.relay.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.data.Relay
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import javax.inject.Inject

/**
 * What a notification's keys do without opening the app: deny a held command, allow it once
 * (the system asks for the phone's unlock first, `setAuthenticationRequired`), or answer a
 * blocked agent with priority mail, which it reads at its next step.
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
                        if (hold < 0 || !online()) return@launch
                        val payload = buildJsonObject {
                            put("hold_id", hold)
                            if (intent.action == ACTION_DENY) put("reason", "Denied from the phone's notification")
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
            } catch (_: Exception) {
                // The notification stays; opening the app shows why.
            } finally {
                pending.finish()
            }
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
        const val EXTRA_HOLD = "hold"
        const val EXTRA_OP = "hold_op"
        const val EXTRA_CREATED = "hold_created"
        const val EXTRA_SESSION = "session"
        const val EXTRA_PROJECT = "project"
        const val EXTRA_NOTIFICATION = "notification"
        const val KEY_REPLY = "reply"
    }
}
