package com.forge.app.service

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.hilt.work.HiltWorker
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.forge.app.MainActivity
import com.forge.app.R
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.notify.TrainingReminder
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.Program
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first
import java.time.Duration
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.util.concurrent.TimeUnit

/**
 * Posts a once-a-day "train today" reminder (engagement thread). Folds in streak-at-risk: when a
 * streak is on the line the copy upgrades to protect it, so the user never gets two nudges in a day.
 *
 * Opt-in (off by default) and quiet-hours-aware. Stays silent if you've already trained today or
 * today is a rest day. The actual wording lives in the pure [TrainingReminder] so it's unit-tested.
 */
@HiltWorker
class TrainingReminderWorker @AssistedInject constructor(
    @Assisted private val ctx: Context,
    @Assisted params: WorkerParameters,
    private val sessionDao: SessionDao,
    private val settingsRepo: SettingsRepository,
    private val statsRepo: StatsRepository
) : CoroutineWorker(ctx, params) {

    override suspend fun doWork(): Result {
        if (!settingsRepo.trainingReminderEnabled.first()) return Result.success()
        if (settingsRepo.isQuietNow()) return Result.success() // skip; it fires again tomorrow

        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val dayStart = today.atStartOfDay(zone).toInstant().toEpochMilli()
        val dayEnd = today.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val finishedToday = sessionDao.finishedInRange(dayStart, dayEnd)

        val dayName = resolveTodayDayName(today, finishedToday.map { it.dayKey }.toSet())
        // Focused two-query streak read — not the full weekly-stats flow — inside the worker's window.
        val streak = runCatching { statsRepo.currentStreakDays() }.getOrDefault(0)

        val nudge = TrainingReminder.build(
            trainedToday = finishedToday.isNotEmpty(),
            dayName = dayName,
            streakDays = streak
        ) ?: return Result.success()

        ensureChannel(ctx)
        val tap = PendingIntent.getActivity(
            ctx, 0,
            Intent(ctx, MainActivity::class.java).apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notification = NotificationCompat.Builder(ctx, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_forge)
            .setContentTitle(nudge.title)
            .setContentText(nudge.body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(nudge.body))
            .setContentIntent(tap)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .build()
        (ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).notify(NOTIF_ID, notification)
        return Result.success()
    }

    /** Today's workout name: weekday mode reads today's slot (rest day ⇒ null); sequence mode the next-up. */
    private suspend fun resolveTodayDayName(today: LocalDate, trainedTodayKeys: Set<String>): String? {
        val dayKeys = Program.dayKeys
        if (dayKeys.isEmpty()) return null
        val todayIndex = today.dayOfWeek.value - 1
        val mode = settingsRepo.scheduleMode.first()
        val key = if (mode == WeeklySchedule.MODE_WEEKDAY) {
            settingsRepo.weeklySchedule.first().getOrNull(todayIndex)?.takeIf { it.isNotBlank() && it in dayKeys }
        } else {
            WeeklySchedule.resolveNextUp(
                mode = mode,
                todayIndex = todayIndex,
                schedule = emptyList(),
                dayKeys = dayKeys,
                lastFinishedDayKey = sessionDao.lastFinishedDayKey(),
                trainedTodayKeys = trainedTodayKeys
            )
        }
        return key?.let { k -> Program.days.firstOrNull { it.key == k }?.defaultName }
    }

    companion object {
        private const val CHANNEL_ID = "forge_training_reminder"
        private const val WORK_NAME = "forge_training_reminder"
        private const val NOTIF_ID = 2002

        /** Arm the daily reminder near [hour]. Use KEEP on boot, REPLACE when the user changes it. */
        fun schedule(context: Context, hour: Int, policy: ExistingPeriodicWorkPolicy) {
            val request = PeriodicWorkRequestBuilder<TrainingReminderWorker>(1, TimeUnit.DAYS)
                .setInitialDelay(initialDelayMinutes(hour), TimeUnit.MINUTES)
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.MINUTES)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(WORK_NAME, policy, request)
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(WORK_NAME)
        }

        /** Minutes from now until the next occurrence of [hour]:00 (always ≥ 1). */
        private fun initialDelayMinutes(hour: Int): Long {
            val now = LocalDateTime.now()
            var next = now.withHour(hour.coerceIn(0, 23)).withMinute(0).withSecond(0).withNano(0)
            if (!next.isAfter(now)) next = next.plusDays(1)
            return Duration.between(now, next).toMinutes().coerceAtLeast(1)
        }

        private fun ensureChannel(context: Context) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (nm.getNotificationChannel(CHANNEL_ID) != null) return
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Training reminders", NotificationManager.IMPORTANCE_DEFAULT).apply {
                    description = "A daily nudge to train on your scheduled days"
                }
            )
        }
    }
}
