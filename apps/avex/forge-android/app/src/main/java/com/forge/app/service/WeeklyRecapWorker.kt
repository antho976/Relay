package com.forge.app.service

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.hilt.work.HiltWorker
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.forge.app.MainActivity
import com.forge.app.R
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.units.formatWeight
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first
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
        // Per-type opt-out (Settings → Notifications, N2). Disabled ⇒ nothing to post.
        if (!settingsRepo.weeklyRecapEnabled.first()) return Result.success()
        // Defer (retry) rather than silently drop the recap if we land in quiet hours.
        if (settingsRepo.isQuietNow()) return Result.retry()
        val stats = statsRepo.observeWeeklyStats().firstOrNull() ?: return Result.success()
        if (stats.workouts == 0) return Result.success() // nothing to recap
        val useKg = settingsRepo.useKg.first()

        ensureChannel(ctx)
        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val body = buildString {
            append("${stats.workouts} workout${if (stats.workouts != 1) "s" else ""}")
            if (stats.volumeLb > 0) append(" · ${formatWeight(stats.volumeLb, useKg)}")
            if (stats.cardioMinutes > 0) append(" · ${stats.cardioMinutes} min cardio")
            if (stats.streakDays > 0) append(" · ${stats.streakDays}-day streak")
        }
        // Tapping the recap opens the app (lands on Overview, the home/start destination) instead of
        // doing nothing — the recap is a stats nudge, so it should at least bring the user back in.
        val tap = PendingIntent.getActivity(
            ctx, 0,
            Intent(ctx, MainActivity::class.java).apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notification = NotificationCompat.Builder(ctx, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_forge)
            .setContentTitle("Weekly recap")
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(tap)
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
            val constraints = Constraints.Builder()
                .setRequiresBatteryNotLow(true)
                .build()
            val request = PeriodicWorkRequestBuilder<WeeklyRecapWorker>(7, TimeUnit.DAYS)
                .setConstraints(constraints)
                // Quiet-hours / transient failures return Result.retry(); back off instead of
                // hammering the default ~30s-then-immediate cadence.
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.MINUTES)
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
