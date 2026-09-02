package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.coach.PreSessionBrief
import com.forge.app.domain.coach.TodayDirective
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.Program
import com.forge.shared.weight.ProtocolWeightUnit
import kotlinx.coroutines.flow.first
import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.TemporalAdjusters
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Assembles today's directive and its brief (Coach v3 B2) — the impure half of Decision Zero.
 *
 * Everything the answer depends on is gathered here and handed to two pure functions, so the
 * decision itself stays testable and the same answer can be published to the Overview card, the
 * home widget and the watch tile without any of them re-deriving it.
 */
@Singleton
class DirectiveRepository @Inject constructor(
    private val adaptationRepository: AdaptationRepository,
    private val settingsRepository: SettingsRepository,
    private val coachGoalRepository: CoachGoalRepository,
    private val academyRepository: AcademyRepository,
    private val workoutRepository: WorkoutRepository,
    private val clock: Clock
) {

    data class TodayAnswer(
        val directive: TodayDirective.Directive,
        val brief: PreSessionBrief.Brief?,
        /**
         * The cold-start lesson to read today (B3). Present only while the coach is below its data
         * gates: the curriculum carries "what do I do?" until the advisors can, so the card
         * degrades from personalised to principled instead of going quiet.
         */
        val coldStartLesson: com.forge.app.domain.academy.Lesson? = null
    )

    /**
     * Today's one answer. Never throws and never returns null: a coach that goes blank on a bad
     * read has broken its only promise, so a failure degrades to the rest-day answer.
     */
    suspend fun today(): TodayAnswer = runCatching { compute() }.getOrElse {
        TodayAnswer(
            directive = TodayDirective.Directive(
                kind = TodayDirective.Kind.REST,
                headline = "Rest today",
                reason = "Nothing to plan against right now."
            ),
            brief = null
        )
    }

    private suspend fun compute(): TodayAnswer {
        val snapshot = adaptationRepository.snapshotCached()
        val life = adaptationRepository.lifeEvents(snapshot.nowMs)
        val readiness = adaptationRepository.readinessScale()
        val freestyle = settingsRepository.freestyleMode.first()

        val zone = ZoneId.systemDefault()
        val today = Instant.ofEpochMilli(snapshot.nowMs).atZone(zone).toLocalDate()
        val todayIndex = today.dayOfWeek.value - 1
        val weekStartMs = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
            .atStartOfDay(zone).toInstant().toEpochMilli()

        val mode = settingsRepository.scheduleMode.first()
        val schedule = settingsRepository.weeklySchedule.first()
        val dayKeys = Program.dayKeys
        val lastFinished = snapshot.sessions.filter { it.finishedAt != null && !it.isUntracked }
            .maxByOrNull { it.startedAt }?.dayKey
        val trainedTodayKeys = snapshot.sessions
            .filter { it.finishedAt != null && !it.isUntracked && it.startedAt >= todayStart(snapshot.nowMs, zone) }
            .map { it.dayKey }
            .toSet()

        val resolved = if (freestyle) null else WeeklySchedule.resolveNextUpWithOffset(
            mode = mode,
            todayIndex = todayIndex,
            schedule = schedule,
            dayKeys = dayKeys,
            lastFinishedDayKey = lastFinished,
            trainedTodayKeys = trainedTodayKeys
        )
        val weekdayMode = mode == WeeklySchedule.MODE_WEEKDAY
        // A blank weekday slot is a deliberate rest day. The resolver still names the next scheduled
        // workout so the rest can say what's coming, but only a day resolved for TODAY may become
        // "train". The offset used to be dropped here, so Wednesday's rest opened Thursday's session
        // as if it were today's. Sequence mode has no calendar; its answer is always today's.
        val restDayScheduled = weekdayMode && resolved != null && resolved.daysAhead > 0
        val nextUp = resolved?.dayKey?.takeUnless { restDayScheduled }

        // A consistency goal is the athlete's own weekly budget; without one the coach doesn't
        // invent a number to hold them to.
        val weeklyTarget = runCatching {
            coachGoalRepository.active()
                .firstOrNull { it.kind == com.forge.app.domain.coach.CoachGoalKind.CONSISTENCY.code }
                ?.targetValue?.toInt()
        }.getOrNull()

        val directive = TodayDirective.compute(
            s = snapshot,
            readiness = readiness,
            life = life,
            nextUpDayKey = nextUp,
            dayName = { key -> Program.day(key)?.defaultName ?: key },
            trainedToday = trainedTodayKeys.isNotEmpty(),
            weekdayMode = weekdayMode,
            sessionsThisWeek = TodayDirective.sessionsSince(snapshot, weekStartMs),
            weeklyTarget = weeklyTarget,
            freestyle = freestyle,
            upcomingDayKey = resolved?.dayKey?.takeIf { restDayScheduled },
            upcomingInDays = resolved?.daysAhead ?: 0
        )

        val brief = directive.dayKey?.let { key ->
            PreSessionBrief.build(
                s = snapshot,
                dayKey = key,
                readiness = readiness,
                life = life,
                weightUnit = protocolUnit(settingsRepository.weightUnit.first())
            )
        }
        // Below the gates the directive is curriculum-driven; past them the lesson drops away on
        // its own because the track has been read.
        val lesson = if (directive.kind == TodayDirective.Kind.LEARN ||
            snapshot.sessions.size < COLD_START_SESSIONS
        ) {
            runCatching { academyRepository.coldStartLesson() }.getOrNull()
        } else null
        return TodayAnswer(directive, brief, lesson)
    }

    private fun todayStart(nowMs: Long, zone: ZoneId): Long =
        Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate().atStartOfDay(zone).toInstant().toEpochMilli()

    private companion object {
        /** Sessions below which the cold-start track still leads the directive. */
        const val COLD_START_SESSIONS = 6
    }

    private fun protocolUnit(unit: WeightUnit): ProtocolWeightUnit = when (unit) {
        WeightUnit.KG -> ProtocolWeightUnit.KG
        WeightUnit.ST -> ProtocolWeightUnit.ST
        else -> ProtocolWeightUnit.LB
    }
}
