package com.forge.app.service

import android.app.NotificationManager
import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
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
import java.time.ZoneId
import java.time.ZonedDateTime
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
        // Don't nudge "train today" while a workout is literally in progress — they're already here.
        if (sessionDao.getActiveSession() != null) return Result.success()

        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val dayStart = today.atStartOfDay(zone).toInstant().toEpochMilli()
        val dayEnd = today.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
        // TRACKED: an untracked workout still suppressed the day's nudge and fed the schedule
        // resolution below it, so excluding a session from your record also silently cancelled the
        // reminder to actually train.
        val finishedToday = sessionDao.finishedInRangeTracked(dayStart, dayEnd)

        // Freestyle ("go with the flow"): there's no plan to name a day from — skip the resolution so we
        // never announce a phantom day (a stale seed plan could still resolve one). The reminder stays,
        // just with plan-agnostic copy.
        val freestyle = settingsRepo.freestyleMode.first()
        val dayName = if (freestyle) null else resolveTodayDayName(today, finishedToday.map { it.dayKey }.toSet())
        // No plan to name a day (freestyle, OR a custom user who hasn't built their plan yet) → use the
        // plan-agnostic nudge instead of going silent, so an opted-in user still gets their reminder.
        val noFixedPlan = freestyle || Program.dayKeys.isEmpty()
        // Focused two-query streak read — not the full weekly-stats flow — inside the worker's window.
        val streak = runCatching { statsRepo.currentStreakDays() }.getOrDefault(0)

        // A scheduled rest day = weekday mode with today's slot deliberately blank (a real program is
        // loaded). Sequence mode has no fixed rest days, so the gentle rest note only applies here.
        // Freestyle has no schedule, so it can't be a "scheduled rest day".
        val isRestDay = !freestyle && dayName == null &&
            Program.dayKeys.isNotEmpty() &&
            settingsRepo.scheduleMode.first() == WeeklySchedule.MODE_WEEKDAY &&
            settingsRepo.weeklySchedule.first().getOrNull(today.dayOfWeek.value - 1).isNullOrBlank()

        val nudge = TrainingReminder.build(
            trainedToday = finishedToday.isNotEmpty(),
            dayName = dayName,
            streakDays = streak,
            isScheduledRestDay = isRestDay,
            noFixedPlan = noFixedPlan
        ) ?: return Result.success()

        ForgeNotifications.ensureChannel(ctx, CHANNEL_ID, CHANNEL_NAME, CHANNEL_DESC)
        val notification = ForgeNotifications.build(ctx, CHANNEL_ID, nudge.title, nudge.body)
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
        private const val CHANNEL_NAME = "Training reminders"
        private const val CHANNEL_DESC = "A daily nudge to train on your scheduled days"
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

        /** Minutes from now until the next occurrence of [hour]:00 (always ≥ 1). Zone-aware (ZonedDateTime,
         *  not LocalDateTime) so the delay spans real elapsed time across a DST gap/overlap — a zone-less
         *  computation drifts an hour on transition days and fires the reminder early/late. */
        private fun initialDelayMinutes(hour: Int): Long {
            val zone = ZoneId.systemDefault()
            val now = ZonedDateTime.now(zone)
            var next = now.withHour(hour.coerceIn(0, 23)).withMinute(0).withSecond(0).withNano(0)
            if (!next.isAfter(now)) next = next.plusDays(1)
            return Duration.between(now, next).toMinutes().coerceAtLeast(1)
        }
    }
}
