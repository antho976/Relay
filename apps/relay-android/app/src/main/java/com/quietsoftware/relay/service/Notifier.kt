package com.quietsoftware.relay.service

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import android.os.Build
import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.JsonObject
import androidx.core.content.ContextCompat
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.quietsoftware.relay.MainActivity
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.model.Notification
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.ui.shell.reveal
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

private val Context.notifyState by preferencesDataStore(name = "notified")

/**
 * Says on the phone what the PC says needs the person: a held command, a blocked agent, an agent
 * that finished. It reads the replica, not the wire, so the same rule covers the open link and a
 * background sync: anything newer than what was already shown, while the app is not on screen.
 */
class Notifier(private val context: Context, private val relay: Relay, private val settings: Settings) {
    fun start(scope: CoroutineScope, foreground: () -> Boolean) {
        channels()
        scope.launch {
            combine(relay.holds(), relay.notifications(), settings.prefs) { h, n, p -> Triple(h, n, p) }.collect { (holds, notes, prefs) ->
                // With notifications off what comes is still recorded, so turning them on announces nothing old.
                post(holds, notes, quiet = foreground() || !prefs.notify)
            }
        }
    }

    /**
     * Post what is new, or only record it as seen when the person is looking at the app. A hold
     * answered or gone takes its notification with it, whose keys could no longer do anything.
     */
    suspend fun post(holds: List<Hold>, notes: List<Notification>, quiet: Boolean) {
        val state = context.notifyState.data.first()
        // The first look records what is already there instead of announcing all of it.
        val seenHolds = state[HOLDS]
        val lastNote = state[LAST_NOTE] ?: -1L
        val freshHolds = if (seenHolds == null) emptyList() else holds.filter { it.id.toString() !in seenHolds }
        val freshNotes = if (lastNote < 0) emptyList() else notes.filter { it.id > lastNote && !it.read && it.category in LOUD }
        context.notifyState.edit { s ->
            s[HOLDS] = holds.map { it.id.toString() }.toSet()
            s[LAST_NOTE] = maxOf(lastNote, notes.maxOfOrNull { it.id } ?: 0L)
        }
        val nm = NotificationManagerCompat.from(context)
        val open = holds.map { it.id.toString() }.toSet()
        for (gone in seenHolds.orEmpty() - open) gone.toLongOrNull()?.let { nm.cancel(holdId(it)) }
        if (quiet || !allowed()) return
        for (h in freshHolds) {
            val id = holdId(h.id)
            // What Allow once would let through, as the hold card shows it, not only the op's name.
            val subject = (h.details as? JsonObject)?.let { d -> d.s("command") ?: d.s("path") ?: d.s("value") }?.let { reveal(it).first } ?: h.op
            val b = base(CH_ATTENTION)
                .setContentTitle("${h.session ?: "An agent"} needs permission")
                .setContentText("Held by ${h.policy.ifBlank { "a guardrail" }}: $subject")
                .setStyle(NotificationCompat.BigTextStyle().bigText("Held by ${h.policy.ifBlank { "a guardrail" }}: $subject"))
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .setContentIntent(open("inbox"))
                .addAction(
                    NotificationCompat.Action.Builder(0, "Deny", action(ActionReceiver.ACTION_DENY, id) {
                        putExtra(ActionReceiver.EXTRA_HOLD, h.id)
                        putExtra(ActionReceiver.EXTRA_OP, h.op)
                        putExtra(ActionReceiver.EXTRA_CREATED, h.createdAt)
                    }).build(),
                )
            // Allowing from a notification needs the phone unlocked, which only Android 12 can require.
            if (Build.VERSION.SDK_INT >= 31) {
                b.addAction(
                    NotificationCompat.Action.Builder(0, "Allow once", action(ActionReceiver.ACTION_ALLOW, id) {
                        putExtra(ActionReceiver.EXTRA_HOLD, h.id)
                        putExtra(ActionReceiver.EXTRA_OP, h.op)
                        putExtra(ActionReceiver.EXTRA_CREATED, h.createdAt)
                    })
                        .setAuthenticationRequired(true)
                        .build(),
                )
            }
            show(nm, id, b.build())
        }
        for (n in freshNotes) {
            val id = NOTE_BASE + (n.id % 10_000).toInt()
            val b = base(if (n.category == "agent_done") CH_DONE else CH_ATTENTION)
                .setContentTitle(n.title.ifBlank { if (n.category == "agent_done") "Agent finished" else "Agent blocked" })
                .setContentText(n.body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(n.body))
            val session = ((n.link as? JsonObject)?.get("payload") as? JsonObject)?.s("session")
            val project = n.projectId
            // A tap opens the agent's conversation, where its answer is; one without an agent, the inbox.
            b.setContentIntent(open(session?.let { MainActivity.OPEN_CHAT + it } ?: "inbox"))
            if (session != null && project != null) {
                // Answer the agent from here: priority mail, read at its next step.
                b.addAction(
                    NotificationCompat.Action.Builder(0, "Reply", action(ActionReceiver.ACTION_REPLY, id, mutable = true) {
                        putExtra(ActionReceiver.EXTRA_SESSION, session)
                        putExtra(ActionReceiver.EXTRA_PROJECT, project)
                    })
                        .addRemoteInput(RemoteInput.Builder(ActionReceiver.KEY_REPLY).setLabel("Message to $session").build())
                        .build(),
                )
            }
            show(nm, id, b.build())
        }
    }

    /** Post one notification, if the person allows them; they can take that back at any time. */
    private fun show(nm: NotificationManagerCompat, id: Int, notification: android.app.Notification) {
        if (!allowed()) return
        try {
            nm.notify(id, notification)
        } catch (_: SecurityException) {
            // Taken back between the check and the post.
        }
    }

    fun clear() = NotificationManagerCompat.from(context).cancelAll()

    private fun holdId(hold: Long) = HOLD_BASE + (hold % 10_000).toInt()

    private fun base(channel: String) = NotificationCompat.Builder(context, channel)
        .setSmallIcon(R.drawable.ic_stat_relay)
        .setAutoCancel(true)
        .setColor(0xFFEDE9E2.toInt())

    private fun action(name: String, notification: Int, mutable: Boolean = false, extras: Intent.() -> Unit): PendingIntent = PendingIntent.getBroadcast(
        context,
        (name + notification).hashCode(),
        Intent(context, ActionReceiver::class.java).setAction(name).putExtra(ActionReceiver.EXTRA_NOTIFICATION, notification).apply(extras),
        PendingIntent.FLAG_UPDATE_CURRENT or if (mutable) PendingIntent.FLAG_MUTABLE else PendingIntent.FLAG_IMMUTABLE,
    )

    private fun open(where: String): PendingIntent = PendingIntent.getActivity(
        context,
        where.hashCode(),
        Intent(context, MainActivity::class.java).putExtra(MainActivity.EXTRA_OPEN, where).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /** Before Android 13 there is nothing to ask; from it, the person's answer. */
    fun allowed() = Build.VERSION.SDK_INT < 33 ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    private fun channels() {
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CH_ATTENTION, "Agents that need you", NotificationManager.IMPORTANCE_HIGH).apply {
            description = "A held command or a blocked agent on your PC"
        })
        nm.createNotificationChannel(NotificationChannel(CH_DONE, "Finished agents", NotificationManager.IMPORTANCE_DEFAULT).apply {
            description = "An agent reports done and waits for review"
        })
        nm.createNotificationChannel(NotificationChannel(CH_LINK, "Connection to your PC", NotificationManager.IMPORTANCE_MIN).apply {
            description = "Shown while Relay keeps the link to your PC open in the background"
            setShowBadge(false)
        })
    }

    companion object {
        const val CH_ATTENTION = "attention"
        const val CH_DONE = "done"
        const val CH_LINK = "link"
        const val LINK_ID = 1
        private const val HOLD_BASE = 10_000
        private const val NOTE_BASE = 20_000
        private val LOUD = setOf("agent_done", "agent_blocked", "guardrail")
        private val HOLDS = stringSetPreferencesKey("holds")
        private val LAST_NOTE = longPreferencesKey("last_note")
    }
}
