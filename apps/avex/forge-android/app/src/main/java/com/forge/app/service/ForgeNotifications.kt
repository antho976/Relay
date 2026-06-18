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
}
