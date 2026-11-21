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
    private val blockRepository: BlockRepository,
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
        val coldStartLesson: com.forge.app.domain.academy.Lesson? = null,
        /**
         * The readiness this answer was computed with (P-02).
         *
         * Carried rather than left for the caller to ask for again: `readinessScale()` walks every
         * finished session, its exercises and its sets, and the glance publisher was calling it
         * beside `today()` — which had just done the same walk to reach the same number. One
         * assembly, one answer, and the two cannot disagree.
         */
        val readiness: com.forge.app.domain.adapt.Recommendation.ReadinessScale? = null
    )

    /**
     * Today's one answer. Never throws and never returns null: a coach that goes blank on a bad
     * read has broken its only promise, so a failure degrades to the rest-day answer.
     */
    suspend fun today(): TodayAnswer = runCatching { kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) { compute() } }.getOrElse {
        if (it is kotlinx.coroutines.CancellationException) throw it
        TodayAnswer(
            directive = TodayDirective.Directive(
                kind = TodayDirective.Kind.REST,
                headline = "Rest today",
                reason = "Nothing to plan against right now."
            ),
            brief = null
        )
    }

    /** A lift's next prescribed session: the next program day that has it, and the coach's target there. */
    data class NextLiftTarget(val dayKey: String, val dayName: String, val weightLb: Double?, val repsText: String)

    /**
     * For Stats' lift page (2026-10-21 redesign): walks the program from today's day (or the start)
     * to the first day that trains [exerciseId], and reads the same brief the session will open
     * with. Null without a program, when no day trains the lift, or on any failed read.
     */
    suspend fun nextTargetFor(exerciseId: String): NextLiftTarget? = runCatching {
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) {
            val days = Program.days
            if (days.isEmpty()) return@withContext null
            val answer = today()
            val from = days.indexOfFirst { it.key == answer.directive.dayKey }.coerceAtLeast(0)
            val day = (days.drop(from) + days.take(from)).firstOrNull { d -> d.exercises.any { it.id == exerciseId } }
                ?: return@withContext null
            val brief = answer.brief?.takeIf { it.dayKey == day.key } ?: run {
                val snapshot = adaptationRepository.snapshotCached()
                PreSessionBrief.build(
                    s = snapshot,
                    dayKey = day.key,
                    readiness = null,
                    life = com.forge.app.domain.coach.LifeEvents.State.NONE,
                    weightUnit = protocolUnit(settingsRepository.weightUnit.first())
                )
            }
            val t = brief?.targets?.firstOrNull { it.exerciseId == exerciseId } ?: return@withContext null
            NextLiftTarget(day.key, day.defaultName, t.targetWeightLb, t.repsText)
        }
    }.getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else null }

    private suspend fun compute(): TodayAnswer {
        val snapshot = adaptationRepository.snapshotCached()
        val life = adaptationRepository.lifeEvents(snapshot.nowMs, snapshot.sessions)
        val readiness = adaptationRepository.readinessScale(snapshot, life)
        val freestyle = settingsRepository.freestyleMode.first()

        val zone = ZoneId.systemDefault()
        val today = Instant.ofEpochMilli(snapshot.nowMs).atZone(zone).toLocalDate()
        val todayIndex = today.dayOfWeek.value - 1
        val weekStartMs = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
            .atStartOfDay(zone).toInstant().toEpochMilli()

        val mode = settingsRepository.scheduleMode.first()
        val schedule = settingsRepository.weeklySchedule.first()
        val dayKeys = Program.dayKeys
        // A freestyle/imported session is not a program day: as the newest one it made the rotation
        // lookup miss and next-up snap back to Day 1.
        val lastFinished = snapshot.sessions
            .filter { it.finishedAt != null && !it.isUntracked && it.dayKey != Program.FREESTYLE_DAY_KEY }
            .maxByOrNull { it.finishedAt!! }?.dayKey
        // Anchored on finish time like Stats and the day list, so a session that runs past midnight
        // counts as today's.
        val trainedTodayKeys = snapshot.sessions
            .filter { it.finishedAt != null && !it.isUntracked && it.finishedAt!! >= todayStart(snapshot.nowMs, zone) }
            .map { it.dayKey }
            .toSet()

        val resolved = if (freestyle) null else WeeklySchedule.resolveNextUpWithOffset(
            mode = mode,
            todayIndex = todayIndex,
            schedule = schedule,
            dayKeys = dayKeys,
            lastFinishedDayKey = lastFinished,
            trainedTodayKeys = trainedTodayKeys,
            recoveryDays = com.forge.app.domain.schedule.TrainingRecovery.daysUntilRecovered(
                Program.days,
                snapshot.sessions.filter { !it.isUntracked }.mapNotNull { session ->
                    session.finishedAt?.let { session.dayKey to java.time.Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
                }, today
            )
        )
        val weekdayMode = mode == WeeklySchedule.MODE_WEEKDAY
        // A blank weekday slot is a deliberate rest day. The resolver still names the next scheduled
        // workout so the rest can say what's coming, but only a day the resolver places TODAY may
        // become "train".
        //
        // Read the placement, not the offset. Offset zero is also what the sequence FALLBACK carries
        // when a weekday schedule resolves nothing at all — every slot rest, or every slot naming a
        // day a regenerate has since dropped — and taking that as today's scheduled session put the
        // user in a workout their own schedule does not contain, on a day it marks as rest.
        val nextUp = WeeklySchedule.trainTodayKey(resolved)
        val upcoming = WeeklySchedule.upcomingKey(resolved)

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
            upcomingDayKey = upcoming,
            upcomingInDays = if (upcoming != null) resolved?.daysAhead ?: 0 else 0
        )

        // The active block's phase, so the brief the athlete reads before the session describes the
        // same week the Coach tab does (H-02). Fail-soft: no block, or an unreadable one, is the
        // no-phase case, which composes to exactly the previous behaviour.
        val phase = runCatching { blockRepository.active() }.getOrNull()
            ?.let { com.forge.app.domain.coach.BlockPhase.fromCode(it.phase) }
        val brief = directive.dayKey?.let { key ->
            PreSessionBrief.build(
                s = snapshot,
                dayKey = key,
                readiness = readiness,
                life = life,
                weightUnit = protocolUnit(settingsRepository.weightUnit.first()),
                phase = phase
            )
        }
        // Below the gates the directive is curriculum-driven; past them the lesson drops away on
        // its own because the track has been read.
        val lesson = if (directive.kind == TodayDirective.Kind.LEARN ||
            snapshot.sessions.size < COLD_START_SESSIONS
        ) {
            runCatching { academyRepository.coldStartLesson() }.getOrNull()
        } else null
        return TodayAnswer(directive, brief, lesson, readiness)
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
