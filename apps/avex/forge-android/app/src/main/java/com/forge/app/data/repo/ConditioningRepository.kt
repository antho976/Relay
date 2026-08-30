package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.engine.AerobicBase
import com.forge.app.domain.engine.ConditioningLoad
import com.forge.app.domain.engine.ConditioningPlanner
import com.forge.app.domain.engine.ConditioningProfile
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.Program
import kotlinx.coroutines.flow.first
import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.TemporalAdjusters
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The Engine's data layer (E-A → E-D): zones, load, the planned conditioning week, and the base
 * trend, assembled from the same cardio rows the hub already shows.
 *
 * Every read is fail-soft. Conditioning is a support system, so a bad read degrades to "no
 * prescription" rather than taking the cardio screen down with it.
 *
 * ## Nothing injects this yet, and that is a HOLD, not an oversight
 *
 * E-A ([ConditioningLoad]) has shipped — `ReadinessAdvisor` reads its interference penalty, and
 * `AcademyRepository` teaches it. E-B through E-D are complete and covered by `ConditioningTest`,
 * and they are waiting on the cardio surface that prescribes from them; `TodayDirective` says as
 * much in its own docstring ("cardio stays a suggestion until Engine E-B can actually prescribe
 * one"). Until that surface lands nothing constructs this class, so an unreferenced-symbol sweep
 * reads the whole engine as dead.
 *
 * It is not. Say so here rather than in a review comment, the way `SessionBreakDao` does for the
 * same situation one layer down, so the next sweep stops at this paragraph instead of at a delete.
 */
@Singleton
class ConditioningRepository @Inject constructor(
    private val cardioDao: CardioDao,
    private val settingsRepository: SettingsRepository,
    private val adaptationRepository: AdaptationRepository,
    private val blockRepository: BlockRepository,
    private val clock: Clock
) {

    /** The athlete's zone model. No age and no override means no zone claims anywhere. */
    suspend fun profile(): ConditioningProfile = ConditioningProfile(
        maxHrOverride = settingsRepository.maxHrOverride.first().takeIf { it > 0 },
        ageYears = settingsRepository.userAgeYears.first().takeIf { it > 0 },
        restingHr = runCatching {
            adaptationRepository.snapshotCached().health.restingHr
                .maxByOrNull { it.timeMs }?.bpm
        }.getOrNull()
    )

    /** This week's conditioning load and its plain-language summary, for the cardio hub. */
    suspend fun weekSummary(): String = runCatching {
        ConditioningLoad.describeWeek(recentCardio(), clock.nowMs())
    }.getOrDefault("")

    /** What the coach would have you do for conditioning this week. */
    suspend fun plannedWeek(): List<ConditioningPlanner.Prescription> = runCatching {
        val now = clock.nowMs()
        val zone = ZoneId.systemDefault()
        val weekStart = Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
            .with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
            .atStartOfDay(zone).toInstant().toEpochMilli()

        val target = settingsRepository.cardioWeeklyTargetMin.first()
            .takeIf { it > 0 } ?: ConditioningPlanner.HEALTH_FLOOR_MINUTES
        val loggedThisWeek = cardioDao.since(weekStart)
        val liftingDaysAhead = daysLeftInWeek(now, zone)

        ConditioningPlanner.planWeek(
            profile = profile(),
            weeklyTargetMinutes = target,
            loggedThisWeek = loggedThisWeek,
            nowMs = now,
            liftingDaysAhead = liftingDaysAhead,
            weekdayMode = settingsRepository.scheduleMode.first() == WeeklySchedule.MODE_WEEKDAY,
            blockPhase = blockRepository.phase(),
            life = adaptationRepository.lifeEvents(now)
        )
    }.getOrDefault(emptyList())

    /** Is the aerobic base moving? Silent until there are comparable sessions to compare. */
    suspend fun baseTrend(): AerobicBase.BaseRead = runCatching {
        AerobicBase.assess(
            entries = recentCardio(),
            restingHr = adaptationRepository.snapshotCached().health.restingHr,
            nowMs = clock.nowMs()
        )
    }.getOrDefault(AerobicBase.BaseRead(AerobicBase.Trend.UNKNOWN, "", false))

    private suspend fun recentCardio() = cardioDao.since(clock.nowMs() - 120L * 24 * 60 * 60 * 1000)

    /** How many program days remain this week — conditioning fills the gaps around them. */
    private fun daysLeftInWeek(nowMs: Long, zone: ZoneId): Int {
        val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        val remaining = DayOfWeek.SUNDAY.value - today.dayOfWeek.value
        return minOf(remaining, Program.days.size)
    }
}
