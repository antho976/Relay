package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.engine.AerobicBase
import com.forge.app.domain.engine.ConditioningLoad
import com.forge.app.domain.engine.ConditioningPlanner
import com.forge.app.domain.engine.ConditioningProfile
import com.forge.app.domain.schedule.WeekPlan
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
 * ## What is live
 *
 * [week] is the read behind the one-week plan (2026-11-18): the Today directive, Home's week strip
 * and the cardio log's prefill all come from it. [weekSummary] and [baseTrend] (E-D) are complete
 * and covered by `ConditioningTest`, and still wait on the Cardio tab growing a place to show them;
 * nothing calls them yet, on purpose.
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

    /**
     * The week as the plan lays it out, and what today asks for.
     *
     * @param days the seven days, Monday first; empty when there is no layout to make (sequence
     *   mode, freestyle, no program, or cardio switched out of the plan).
     * @param cardioDoneDays weekdays (0 = Monday) that already hold an active cardio session.
     * @param targetMinutes this week's cardio minutes after a deload's halving; 0 = none planned.
     * @param today the cardio to do today: the day's session on a cardio day, the easy finisher on a
     *   lifting day, null for none (rest, done, target met, sick or away).
     * @param todayIsCardioDay [today] is the day's main event rather than a finisher.
     */
    data class WeekRead(
        val days: List<WeekPlan.Day> = emptyList(),
        val todayIndex: Int = 0,
        val cardioDoneDays: Set<Int> = emptySet(),
        val targetMinutes: Int = 0,
        val doneMinutes: Int = 0,
        val doneToday: Boolean = false,
        /**
         * Recent days (as `LocalDate.toEpochDay`) that hold an active cardio session. A week strip
         * that starts on Sunday cannot use [cardioDoneDays], which is counted from Monday.
         */
        val cardioEpochDays: Set<Long> = emptySet(),
        val today: ConditioningPlanner.Prescription? = null,
        val todayIsCardioDay: Boolean = false,
        /** The athlete's zones and the block phase, for a caller that builds a day-by-day answer. */
        val profile: ConditioningProfile = ConditioningProfile(),
        val phase: com.forge.app.domain.coach.BlockPhase? = null
    ) {
        /** Minutes the week still owes, never below zero. */
        val owedMinutes: Int get() = (targetMinutes - doneMinutes).coerceAtLeast(0)

        companion object {
            val NONE = WeekRead()
        }
    }

    /**
     * Lay the week out and read today off it. Fail-soft like every read here: a bad one is a week
     * with no cardio in it, never a blank Home.
     */
    suspend fun week(life: com.forge.app.domain.coach.LifeEvents.State): WeekRead = runCatching {
        val now = clock.nowMs()
        val zone = ZoneId.systemDefault()
        val today = Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
        val todayIndex = today.dayOfWeek.value - 1
        val monday = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
        val weekStart = monday.atStartOfDay(zone).toInstant().toEpochMilli()

        if (!settingsRepository.cardioInPlan.first() || settingsRepository.freestyleMode.first()) return@runCatching WeekRead.NONE
        val phase = blockRepository.phase()
        val goal = settingsRepository.cardioWeeklyTargetMin.first().takeIf { it > 0 }
            ?: ConditioningPlanner.HEALTH_FLOOR_MINUTES
        val target = ConditioningPlanner.weekTarget(goal, phase)

        val recent = recentCardio()
        val active = recent.filter { it.restReason == null }
        val thisWeek = active.filter { it.date >= weekStart }
        val doneMinutes = thisWeek.sumOf { it.durationMin }
        val doneDays = thisWeek.map { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate().dayOfWeek.value - 1 }.toSet()
        val doneToday = todayIndex in doneDays

        // A base to stand on before anything hard: the two weeks before this one averaged at least
        // half the health floor, the week is not a deload, and the load is not already climbing fast.
        val baseFrom = weekStart - 14L * DAY_MS
        val baseMinutes = active.filter { it.date in baseFrom until weekStart }.sumOf { it.durationMin }
        val ramp = ConditioningLoad.rampRate(recent, now)
        val allowHard = baseMinutes >= ConditioningPlanner.HEALTH_FLOOR_MINUTES &&
            phase != com.forge.app.domain.coach.BlockPhase.DELOAD &&
            (ramp == null || ramp <= ConditioningPlanner.MAX_RAMP)

        val profile = profile()
        val weekday = settingsRepository.scheduleMode.first() == WeeklySchedule.MODE_WEEKDAY
        val days = if (!weekday || Program.days.isEmpty()) emptyList() else WeekPlan.layout(
            WeekPlan.Inputs(
                schedule = settingsRepository.weeklySchedule.first(),
                validKeys = Program.dayKeys.toSet(),
                lowerBodyKeys = Program.days.filter { WeekPlan.isLowerBody(it) }.map { it.key }.toSet(),
                targetMinutes = target,
                allowHard = allowHard
            )
        )

        // Illness and a live layoff outrank conditioning entirely.
        val resting = life.sick || life.layoff?.away == true
        val planned = days.getOrNull(todayIndex)
        val minutes = if (resting) 0 else WeekPlan.minutesToday(days, todayIndex, target, doneMinutes, doneToday)
        val prescription = when {
            planned == null || minutes <= 0 -> null
            planned.kind == WeekPlan.Kind.LIFT -> ConditioningPlanner.finisher(profile, minutes)
            planned.hard -> ConditioningPlanner.intervals(profile, weekdayMode = true)
            else -> ConditioningPlanner.steady(profile, minutes, phase)
        }
        WeekRead(
            days = days,
            todayIndex = todayIndex,
            cardioDoneDays = doneDays,
            targetMinutes = target,
            doneMinutes = doneMinutes,
            doneToday = doneToday,
            cardioEpochDays = active.filter { it.date >= weekStart - 7L * DAY_MS }
                .map { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate().toEpochDay() }.toSet(),
            today = prescription,
            todayIsCardioDay = prescription != null && planned?.kind == WeekPlan.Kind.CARDIO,
            profile = profile,
            phase = phase
        )
    }.getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else WeekRead.NONE }

    /** Is the aerobic base moving? Silent until there are comparable sessions to compare. */
    suspend fun baseTrend(): AerobicBase.BaseRead = runCatching {
        AerobicBase.assess(
            entries = recentCardio(),
            restingHr = adaptationRepository.snapshotCached().health.restingHr,
            nowMs = clock.nowMs()
        )
    }.getOrDefault(AerobicBase.BaseRead(AerobicBase.Trend.UNKNOWN, "", false))

    private suspend fun recentCardio() = cardioDao.since(clock.nowMs() - 120L * 24 * 60 * 60 * 1000)

    private companion object {
        const val DAY_MS = 24L * 60 * 60 * 1000
    }
}
