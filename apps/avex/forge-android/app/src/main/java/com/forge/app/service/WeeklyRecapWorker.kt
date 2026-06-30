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
import java.time.LocalDate
import java.time.ZoneId
import java.util.concurrent.TimeUnit

/**
 * Fires once a week and carries the three weekly engagement nudges (#31 + #13):
 *  - the "Your week in numbers" recap (when you trained),
 *  - a "your coach has an update" push when a new Week Brief is ready and unseen,
 *  - a gentle, non-guilt-y "come back" when a whole week passed with no sessions (suppressed on holiday).
 *
 * Weekly cadence keeps re-engagement from nagging. Scheduled on first app open; KEEP-idempotent.
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

        val stats = statsRepo.observeWeeklyStats().firstOrNull() ?: return Result.success()

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
            val useKg = settingsRepo.useKg.first()
            // A full-week streak milestone turns the recap into a small celebration via its title — so
            // when it does, the streak is dropped from the body line to avoid stating it twice.
            val isStreakMilestone = stats.streakDays >= 7 && stats.streakDays % 7 == 0
            val body = buildString {
                append("${stats.workouts} workout${if (stats.workouts != 1) "s" else ""}")
                if (stats.volumeLb > 0) append(" · ${formatWeight(stats.volumeLb, useKg)}")
                if (stats.cardioMinutes > 0) append(" · ${stats.cardioMinutes} min cardio")
                if (stats.streakDays > 0 && !isStreakMilestone) append(" · ${stats.streakDays}-day streak")
                // Retention hooks: the closest trophy you're chasing, and a memory from this date.
                trophyRepo.observeNearMisses().firstOrNull()?.firstOrNull()?.let { nmiss ->
                    append(" · Almost: ${nmiss.trophyName} (${nmiss.progress}/${nmiss.target})")
                }
                runCatching { statsRepo.findOnThisDayMemory() }.getOrNull()?.let { mem ->
                    val ago = if (mem.monthsAgo % 12 == 0) "${mem.monthsAgo / 12}yr" else "${mem.monthsAgo}mo"
                    append(" · On this day ($ago ago): ${mem.dayName}")
                }
            }
            val title = if (isStreakMilestone) "${stats.streakDays}-day streak!" else "Weekly recap"
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

        fun schedule(context: Context) {
            val constraints = Constraints.Builder()
                .setRequiresBatteryNotLow(true)
                .build()
            // Flex window: let WorkManager fire anywhere in the last 6h of each 7-day period so it can
            // batch with other jobs / pick a low-power moment, instead of pinning the exact 7-day mark.
            val request = PeriodicWorkRequestBuilder<WeeklyRecapWorker>(7, TimeUnit.DAYS, 6, TimeUnit.HOURS)
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
    }
}
