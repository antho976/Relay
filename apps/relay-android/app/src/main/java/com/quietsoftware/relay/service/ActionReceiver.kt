package com.quietsoftware.relay.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import com.quietsoftware.relay.data.Relay
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import javax.inject.Inject

/**
 * What a notification's keys do without opening the app: deny a held command, allow it once
 * (the system asks for the phone's unlock first, `setAuthenticationRequired`; [HoldWorker] does
 * it), or answer a blocked agent with priority mail, which it reads at its next step. A hold's
 * key that cannot reach the PC says so on its notification.
 */
@AndroidEntryPoint
class ActionReceiver : BroadcastReceiver() {
    @Inject lateinit var relay: Relay

    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            // Reaching the PC and running the confirm take longer than a receiver may: a worker does it.
            ACTION_DENY, ACTION_ALLOW -> HoldWorker.enqueue(context, intent)
            ACTION_REPLY -> reply(context, intent)
        }
    }

    private fun reply(context: Context, intent: Intent) {
        val pending = goAsync()
        val notification = intent.getIntExtra(EXTRA_NOTIFICATION, 0)
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            try {
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
                if (notification != 0) NotificationManagerCompat.from(context).cancel(notification)
            } catch (_: Exception) {
                // The notification stays; opening the app shows why.
            } finally {
                pending.finish()
            }
        }
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
