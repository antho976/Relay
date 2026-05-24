package com.forge.app.service

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import androidx.core.app.NotificationCompat
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.StatsRepository
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.firstOrNull
import java.util.concurrent.TimeUnit

/**
 * Fires once a week and posts a "Your week in numbers" notification (#31).
 * Scheduled on first app open. Re-scheduling is idempotent (KEEP policy).
 */
@HiltWorker
class WeeklyRecapWorker @AssistedInject constructor(
    @Assisted private val ctx: Context,
    @Assisted params: WorkerParameters,
    private val statsRepo: StatsRepository,
    private val settingsRepo: SettingsRepository
) : CoroutineWorker(ctx, params) {

    override suspend fun doWork(): Result {
        if (settingsRepo.isQuietNow()) return Result.success()
        val stats = statsRepo.observeWeeklyStats().firstOrNull() ?: return Result.success()
        if (stats.workouts == 0) return Result.success() // nothing to recap

        ensureChannel(ctx)
        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val body = buildString {
            append("${stats.workouts} workout${if (stats.workouts != 1) "s" else ""}")
            if (stats.volumeLb > 0) append(" · ${stats.volumeLb.toInt()} lb")
            if (stats.cardioMinutes > 0) append(" · ${stats.cardioMinutes} min cardio")
            if (stats.streakDays > 0) append(" · ${stats.streakDays}-day streak")
        }
        val notification = NotificationCompat.Builder(ctx, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle("Weekly recap")
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .build()
        nm.notify(NOTIF_ID, notification)
        return Result.success()
    }

    companion object {
        private const val CHANNEL_ID = "forge_weekly_recap"
        private const val WORK_NAME = "forge_weekly_recap"
        private const val NOTIF_ID = 2001

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<WeeklyRecapWorker>(7, TimeUnit.DAYS)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
        }

        private fun ensureChannel(context: Context) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (nm.getNotificationChannel(CHANNEL_ID) != null) return
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Weekly recap", NotificationManager.IMPORTANCE_DEFAULT).apply {
                    description = "Your weekly training summary"
                }
            )
        }
    }
}
