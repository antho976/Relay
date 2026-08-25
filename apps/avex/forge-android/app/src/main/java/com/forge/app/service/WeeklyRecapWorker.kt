package com.forge.app.service

import android.app.NotificationManager
import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.VacationRepository
import com.forge.app.domain.units.formatWeight
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.firstOrNull
import java.time.DayOfWeek
import java.time.Duration
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.temporal.TemporalAdjusters
import java.util.concurrent.TimeUnit

/**
 * Fires once a week and carries the three weekly engagement nudges (#31 + #13):
 *  - the "Your week in numbers" recap (when you trained),
 *  - a "your coach has an update" push when a new Week Brief is ready and unseen,
 *  - a gentle, non-guilt-y "come back" when a whole week passed with no sessions (suppressed on holiday).
 *
 * Weekly cadence keeps re-engagement from nagging. Anchored to Monday, the day the week's numbers
 * are complete and the Week Brief is published; re-anchored on every app open, which is idempotent
 * because the delay targets the next Monday rather than a fixed interval from now.
 * All three honour quiet hours; only the recap honours the "Weekly recap" per-type opt-out (the
 * coach-brief and come-back nudges are separate features with their own channels).
 */
@HiltWorker
class WeeklyRecapWorker @AssistedInject constructor(
    @Assisted private val ctx: Context,
    @Assisted params: WorkerParameters,
    private val statsRepo: StatsRepository,
    private val settingsRepo: SettingsRepository,
    private val coachRepo: CoachRepository,
    private val trophyRepo: TrophyRepository,
    private val vacationRepo: VacationRepository,
    private val adaptationRepo: com.forge.app.data.repo.AdaptationRepository
) : CoroutineWorker(ctx, params) {

    override suspend fun doWork(): Result {
        // Quiet hours: defer (retry) the whole batch rather than dropping it. Each nudge below has
        // its own opt-out: the coach-brief push and the come-back nudge are SEPARATE features with
        // their own channels, so they are NOT gated by the weekly-recap toggle (that toggle, labelled
        // "Weekly recap", controls only the "your week in numbers" recap). Users can still mute the
        // coach / come-back channels individually in Android's per-channel notification settings.
        if (settingsRepo.isQuietNow()) return Result.retry()

        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        // The coach pushes (brief + deload) only apply when the user kept the coach AND follows a plan —
        // a freestyle user has nothing to coach against, and a coach-off user opted out entirely.
        val freestyle = settingsRepo.freestyleMode.first()
        val coachActive = settingsRepo.coachEnabled.first() && !freestyle

        // ── Coach-brief push (#13): notify when a NEW week brief is ready and unseen. pendingBanner()
        // compares the latest pass to lastSeenCoachWeekId, so a brief the user has already opened never
        // re-pushes. It self-guards on freestyle/coach-off too; the outer gate just avoids the work.
        if (coachActive) {
            runCatching { coachRepo.pendingBanner() }.getOrNull()?.let { banner ->
                ForgeNotifications.ensureChannel(ctx, COACH_CHANNEL_ID, "Coach updates", "When your weekly coach brief is ready")
                nm.notify(COACH_NOTIF_ID, ForgeNotifications.build(
                    ctx, COACH_CHANNEL_ID, "Your coach has an update", banner.text
                ))
            }
        }

        // ── Deload suggestion: when accumulated fatigue crosses the deload line (DeloadAdvisor, which
        // already self-suppresses right after a deload). Its own channel so it's independently mutable;
        // a fixed id so it never stacks week-over-week. Uses the focused deloadSuggestion() — one
        // snapshot, no plateau-ladder/arbitration fan-out — so the weekly worker stays inside its window.
        if (coachActive) {
            runCatching { adaptationRepo.deloadSuggestion() }.getOrNull()
                ?.let { rec ->
                    ForgeNotifications.ensureChannel(ctx, DELOAD_CHANNEL_ID, "Deload suggestions", "When your fatigue suggests a lighter week")
                    nm.notify(DELOAD_NOTIF_ID, ForgeNotifications.build(
                        ctx, DELOAD_CHANNEL_ID, "Time for a deload week", rec.reason
                    ))
                }
        }

        // The week that ENDED, not the one in progress. observeWeeklyStats counts from this Monday
        // 00:00, so a Monday-morning recap read a week a few hours old: a user training three times a
        // week got "0 workouts", the come-back nudge fired at them every single Monday, and everyone
        // else got a recap describing a partial week.
        val stats = statsRepo.lastCompletedWeekStats()
        val streakDays = statsRepo.currentStreakDays()

        if (stats.workouts == 0) {
            // ── Re-engagement (#13): a whole week with no sessions = a lapse. Nudge gently — unless
            // the user is on holiday, where a "come back" would be guilt-trippy, not helpful.
            if (!isOnVacationToday()) {
                // A freestyle user has no plan "waiting" — keep the nudge plan-agnostic so it doesn't
                // reference something they didn't set up.
                val body = if (freestyle)
                    "No pressure — log whatever you train, whenever. Even one session this week keeps your momentum going."
                else
                    "No pressure — your plan's right where you left it. Even one session this week keeps your momentum going."
                ForgeNotifications.ensureChannel(ctx, REENGAGE_CHANNEL_ID, "Come-back nudges", "A gentle nudge after a week away")
                nm.notify(REENGAGE_NOTIF_ID, ForgeNotifications.build(
                    ctx, REENGAGE_CHANNEL_ID,
                    "Ready when you are",
                    body
                ))
            }
            return Result.success()
        }

        // ── Weekly recap (#31): your week in numbers — gated by the per-type opt-out (N2).
        if (settingsRepo.weeklyRecapEnabled.first()) {
            val weightUnit = settingsRepo.weightUnit.first()
            // A full-week streak milestone turns the recap into a small celebration via its title — so
            // when it does, the streak is dropped from the body line to avoid stating it twice.
            val isStreakMilestone = streakDays >= 7 && streakDays % 7 == 0
            val body = buildString {
                append("${stats.workouts} workout${if (stats.workouts != 1) "s" else ""}")
                if (stats.volumeLb > 0) append(" · ${formatWeight(stats.volumeLb, weightUnit)}")
                if (stats.cardioMinutes > 0) append(" · ${stats.cardioMinutes} min cardio")
                if (streakDays > 0 && !isStreakMilestone) append(" · $streakDays-day streak")
                // Retention hooks: the closest trophy you're chasing, and a memory from this date.
                trophyRepo.observeNearMisses().firstOrNull()?.firstOrNull()?.let { nmiss ->
                    append(" · Almost: ${nmiss.trophyName} (${nmiss.progress}/${nmiss.target})")
                }
                runCatching { statsRepo.findOnThisDayMemory() }.getOrNull()?.let { mem ->
                    val ago = if (mem.monthsAgo % 12 == 0) "${mem.monthsAgo / 12}yr" else "${mem.monthsAgo}mo"
                    append(" · On this day ($ago ago): ${mem.dayName}")
                }
            }
            // DESIGN §11: no exclamation marks in a rendered string.
            val title = if (isStreakMilestone) "$streakDays-day streak" else "Weekly recap"
            ForgeNotifications.ensureChannel(ctx, CHANNEL_ID, "Weekly recap", "Your weekly training summary")
            nm.notify(NOTIF_ID, ForgeNotifications.build(ctx, CHANNEL_ID, title, body))
        }
        return Result.success()
    }

    /** True if today's date falls inside any saved holiday range (yyyy-MM-dd compares lexicographically). */
    private suspend fun isOnVacationToday(): Boolean {
        val today = LocalDate.now(ZoneId.systemDefault()).toString() // yyyy-MM-dd
        return vacationRepo.observeAll().firstOrNull().orEmpty()
            .any { today >= it.startDate && today <= it.endDate }
    }

    companion object {
        private const val CHANNEL_ID = "forge_weekly_recap"
        private const val COACH_CHANNEL_ID = "forge_coach_brief"
        private const val REENGAGE_CHANNEL_ID = "forge_reengage"
        private const val DELOAD_CHANNEL_ID = "forge_deload"
        private const val WORK_NAME = "forge_weekly_recap"
        private const val NOTIF_ID = 2001
        private const val COACH_NOTIF_ID = 2003
        private const val REENGAGE_NOTIF_ID = 2004
        private const val DELOAD_NOTIF_ID = 2005

        /** The recap's period boundary: Monday noon local, with the 6 h flex below putting the run
         *  somewhere in Monday morning. Quiet hours defer anything too early. */
        private const val RECAP_HOUR = 12

        fun schedule(context: Context) {
            val constraints = Constraints.Builder()
                .setRequiresBatteryNotLow(true)
                .build()
            // Flex window: let WorkManager fire anywhere in the last 6h of each 7-day period so it can
            // batch with other jobs / pick a low-power moment, instead of pinning the exact 7-day mark.
            //
            // The initial delay anchors that period to Monday, the day the week's numbers are
            // complete and the day the coach's Week Brief is published. Without it the first run was
            // ~7 days after the user's first-ever launch, on whatever weekday and hour that happened
            // to be, and KEEP meant it stayed on that phase forever — so a Wednesday-anchored user's
            // "week in numbers" described Monday to Wednesday, and the coach-brief push could land
            // six days after the brief was ready. UPDATE re-anchors installs already running on an
            // arbitrary phase; because the delay targets the NEXT Monday rather than "7 days from
            // now", re-running this on every app open can't starve the worker.
            val request = PeriodicWorkRequestBuilder<WeeklyRecapWorker>(7, TimeUnit.DAYS, 6, TimeUnit.HOURS)
                .setConstraints(constraints)
                .setInitialDelay(minutesUntilNextMonday(), TimeUnit.MINUTES)
                // Quiet-hours / transient failures return Result.retry(); back off instead of
                // hammering the default ~30s-then-immediate cadence.
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.MINUTES)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.UPDATE,
                request
            )
        }

        /** Minutes until the next Monday [RECAP_HOUR]:00 (always >= 1). ZonedDateTime, not
         *  LocalDateTime, so the delay spans real elapsed time across a DST transition. */
        private fun minutesUntilNextMonday(): Long {
            val zone = ZoneId.systemDefault()
            val now = ZonedDateTime.now(zone)
            var next = now.with(TemporalAdjusters.nextOrSame(DayOfWeek.MONDAY))
                .withHour(RECAP_HOUR).withMinute(0).withSecond(0).withNano(0)
            if (!next.isAfter(now)) next = next.plusWeeks(1)
            return Duration.between(now, next).toMinutes().coerceAtLeast(1)
        }
    }
}
