package com.forge.app.service

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.forge.app.MainActivity
import com.forge.app.R

/**
 * Shared notification plumbing for the engagement workers. One place for the small icon, channel
 * shape, priority, auto-cancel, and the "tap opens the app (Overview)" PendingIntent — so
 * [WeeklyRecapWorker] and [TrainingReminderWorker] can't drift apart on icon/flags/behaviour.
 */
internal object ForgeNotifications {

    /** Idempotently create [channelId] (DEFAULT importance) with the given [name]/[desc]. */
    fun ensureChannel(context: Context, channelId: String, name: String, desc: String) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (nm.getNotificationChannel(channelId) != null) return
        nm.createNotificationChannel(
            NotificationChannel(channelId, name, NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = desc
            }
        )
    }

    /**
     * A notification whose tap opens the app (lands on Overview) and auto-cancels. [requestCode]
     * keeps each notification's PendingIntent distinct when several are posted in one pass; it
     * defaults to the channel-id hash so callers that post one notification per channel get a
     * stable, collision-free code for free.
     */
    fun build(
        context: Context,
        channelId: String,
        title: String,
        body: String,
        requestCode: Int = channelId.hashCode()
    ) = NotificationCompat.Builder(context, channelId)
        .setSmallIcon(R.drawable.ic_stat_forge)
        .setContentTitle(title)
        .setContentText(body)
        .setStyle(NotificationCompat.BigTextStyle().bigText(body))
        .setContentIntent(
            PendingIntent.getActivity(
                context, requestCode,
                Intent(context, MainActivity::class.java).apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP },
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        )
        .setPriority(NotificationCompat.PRIORITY_DEFAULT)
        .setAutoCancel(true)
        .build()

    /**
     * Post the PR-milestone celebration — the one engagement notification fired from the UI (when a
     * finished session crosses a lifetime-PR milestone). The channel + id live here alongside the
     * worker notifications so the id space can't drift or collide, and posting is wrapped so a missing
     * POST_NOTIFICATIONS permission (Android silently drops) or a manager hiccup can't crash the
     * caller's composition.
     */
    fun postPrMilestone(context: Context, title: String, body: String) {
        runCatching {
            ensureChannel(context, PR_MILESTONE_CHANNEL_ID, "PR milestones", "When you reach a personal-record milestone")
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.notify(PR_MILESTONE_NOTIF_ID, build(context, PR_MILESTONE_CHANNEL_ID, title, body))
        }
    }

    // Notification ids live in the 200x band (recap 2001 · coach 2003 · re-engage 2004 · deload 2005).
    private const val PR_MILESTONE_CHANNEL_ID = "forge_pr_milestone"
    private const val PR_MILESTONE_NOTIF_ID = 2006
}
