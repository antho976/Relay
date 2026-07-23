package com.forge.wear.data

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.wear.ongoing.OngoingActivity
import androidx.wear.ongoing.Status

/**
 * The session's presence on the watch face (W1): an OngoingActivity chip while a session is live,
 * so leaving the app mid-workout is one tap from returning — without it the 2-second promise dies
 * at the launcher. Posted/cleared from wherever session state changes (activity or the background
 * listener); posting without the notification permission is a silent no-op (the session still
 * works, just without the chip).
 */
object SessionOngoing {

    private const val CHANNEL = "avex_session"
    private const val NOTIF_ID = 100

    fun show(context: Context, dayTitle: String) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Active session", NotificationManager.IMPORTANCE_LOW)
        )
        val launch = PendingIntent.getActivity(
            context, 0,
            Intent(context, Class.forName("com.forge.wear.MainActivity")),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(dayTitle)
            .setContentText("Session live")
            .setOngoing(true)
            .setContentIntent(launch)
        OngoingActivity.Builder(context, NOTIF_ID, builder)
            .setStaticIcon(android.R.drawable.ic_media_play)
            .setTouchIntent(launch)
            .setStatus(Status.Builder().addTemplate(dayTitle).build())
            .build()
            .apply(context)
        runCatching { nm.notify(NOTIF_ID, builder.build()) }
    }

    fun clear(context: Context) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.cancel(NOTIF_ID)
    }
}
