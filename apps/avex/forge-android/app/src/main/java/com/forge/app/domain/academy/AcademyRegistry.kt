package com.forge.app.domain.academy

import com.forge.app.data.db.entities.LessonEvent

/**
 * The Academy (Coach v3 A2): the knowledge layer whose whole purpose is to make the coach
 * optional. Every decision the coach makes should be one a user can learn to make themselves.
 *
 * Two rules hold this together, and both are enforced here rather than by convention:
 *  - **No lesson without a moment.** A lesson exists because a coach reason or an app moment
 *    unlocks it ([Lesson.unlockedBy] is user-facing, [unlockKeyFor] is the machine side).
 *  - **State is recomputed, never mutated.** [stateFrom] derives everything from the append-only
 *    `lesson_event` ledger, the same idempotent pattern as `CoachGenBias.from(decisions)`, so
 *    read state can't drift or double-count.
 *
 * Content arrives per phase, never ahead of the machinery it describes (`docs/ACADEMY_LESSONS.md`
 * is the authoring order). A2 ships exactly one lesson, because A2 ships exactly one new coach
 * concept a user can see: phase-aware stall interpretation.
 */
object AcademyRegistry {

    /** Every lesson that currently ships. Ordered by track, then authoring order. */
    val lessons: List<Lesson> = AcademyFundamentals.ordered + listOf(
        AcademyContent.readinessBuiltFrom,
        AcademyContent.whyGoalsFight,
        AcademyContent.strengthOnACut
    ) + AcademyProgramming.all + AcademyYourNumbers.all + AcademyAutonomy.all + AcademySignals.all + AcademyEngine.all

    /**
     * The cold-start curriculum, in reading order (B3). Below the coach's data gates these lessons
     * ARE the Today Directive: the card degrades from personalised to principled, never to silence.
     */
    val coldStartTrack: List<Lesson> = AcademyFundamentals.ordered

    /**
     * The next cold-start lesson for a reader — the one the directive should carry today. Null once
     * the track is finished, at which point the coach has enough data to speak for itself.
     */
    fun nextColdStartLesson(events: List<com.forge.app.data.db.entities.LessonEvent>): Lesson? {
        val opened = events.filter { it.kind == LessonEventKind.OPENED.code }.map { it.lessonId }.toSet()
        return coldStartTrack.firstOrNull { it.id !in opened }
    }

    fun lesson(id: String): Lesson? = lessons.firstOrNull { it.id == id }

    fun byTrack(track: LessonTrack): List<Lesson> = lessons.filter { it.track == track }

    /** One lesson's state for this reader, derived from the ledger. */
    data class LessonState(
        val lesson: Lesson,
        val unlocked: Boolean,
        val opened: Boolean,
        val completed: Boolean,
        val unlockedAtMs: Long?
    ) {
        /** Unlocked but never opened — what the "new lesson" chip counts. */
        val isNew: Boolean get() = unlocked && !opened
    }

    /**
     * Fold the ledger into per-lesson state. Unknown lesson ids in the ledger are ignored rather
     * than dropped from history: content can be renamed or retired without corrupting the record.
     */
    fun stateFrom(events: List<LessonEvent>): List<LessonState> {
        val byLesson = events.groupBy { it.lessonId }
        return lessons.map { lesson ->
            val own = byLesson[lesson.id].orEmpty()
            val unlockedAt = own.filter { it.kind == LessonEventKind.UNLOCKED.code }.minOfOrNull { it.atMs }
            LessonState(
                lesson = lesson,
                unlocked = unlockedAt != null,
                opened = own.any { it.kind == LessonEventKind.OPENED.code },
                completed = own.any { it.kind == LessonEventKind.COMPLETED.code },
                unlockedAtMs = unlockedAt
            )
        }
    }

    fun stateOf(lessonId: String, events: List<LessonEvent>): LessonState? =
        stateFrom(events).firstOrNull { it.lesson.id == lessonId }

    /** Lessons the reader has unlocked, newest unlock first — the Academy section's live list. */
    fun unlocked(events: List<LessonEvent>): List<LessonState> =
        stateFrom(events).filter { it.unlocked }.sortedByDescending { it.unlockedAtMs ?: 0L }

    /**
     * Lessons that exist but haven't happened to this reader yet. Shown visible-but-locked, like a
     * COMING_SOON signal slot: the product should be visibly growing, and a locked lesson names the
     * moment that will open it.
     */
    fun upcoming(events: List<LessonEvent>): List<LessonState> =
        stateFrom(events).filter { !it.unlocked }

    /**
     * The audit the plan promises: every shipped lesson must be reachable from a real moment. A
     * lesson with no unlock key is content in search of a coach, and fails the check.
     */
    fun orphanLessons(): List<Lesson> = lessons.filter { unlockKeyFor(it.id) == null }

    /**
     * The machine-side trigger for a lesson: the coach/app moment that unlocks it. Kept beside the
     * content so adding a lesson without wiring its moment is a visible omission, not a silent one.
     */
    fun unlockKeyFor(lessonId: String): String? = when (lessonId) {
        AcademyContent.strengthOnACut.id -> UNLOCK_CUT_STALL_SUPPRESSED
        AcademyContent.readinessBuiltFrom.id -> UNLOCK_READINESS_SHOWN
        AcademyContent.whyGoalsFight.id -> UNLOCK_GOAL_CONFLICT
        AcademyFundamentals.whatAProgramIs.id -> UNLOCK_COLD_START
        AcademyFundamentals.setsRepsRpe.id -> UNLOCK_FIRST_SET
        AcademyFundamentals.formVsLoad.id -> UNLOCK_TECHNIQUE_SESSION
        AcademyFundamentals.progressiveOverload.id -> UNLOCK_FIRST_PROGRESSION
        AcademyFundamentals.restAndRecovery.id -> UNLOCK_REST_TIMER
        AcademyFundamentals.sorenessVsInjury.id -> UNLOCK_SORENESS_FLAG
        AcademyFundamentals.warmups.id -> UNLOCK_WARMUP_SHOWN
        AcademyFundamentals.howTheCoachWorks.id -> UNLOCK_WEEK_BRIEF
        AcademyFundamentals.whatReadinessMeans.id -> UNLOCK_READINESS_SHOWN
        AcademyFundamentals.logHonestly.id -> UNLOCK_COLD_START_DONE
        AcademyProgramming.whatABlockIs.id -> UNLOCK_BLOCK_STARTED
        AcademyProgramming.fourPhases.id -> UNLOCK_PHASE_CHANGE
        AcademyProgramming.deloadsAreEarned.id -> UNLOCK_SCHEDULED_DELOAD
        AcademyProgramming.readingYourBlock.id -> UNLOCK_BLOCK_STARTED
        AcademyYourNumbers.volumeLandmarks.id -> UNLOCK_PERSONAL_CAP
        AcademyYourNumbers.recoveryCurve.id -> UNLOCK_PERSONAL_SPACING
        AcademyYourNumbers.sweetSpotReps.id -> UNLOCK_PERSONAL_REPS
        AcademyYourNumbers.imbalances.id -> UNLOCK_IMBALANCE_PROJECT
        AcademyYourNumbers.whatAProjectIs.id -> UNLOCK_PROJECT_PROPOSED
        AcademyAutonomy.trustTiers.id -> UNLOCK_TIER_CHANGE
        AcademyAutonomy.takingDecisionsBack.id -> UNLOCK_AUTONOMOUS_ACT
        AcademySignals.stressHrv.id -> UNLOCK_HRV_TREND
        AcademyEngine.whyAerobicBase.id -> UNLOCK_CARDIO_PRESCRIPTION
        AcademyEngine.whatZone2Is.id -> UNLOCK_CARDIO_PRESCRIPTION
        AcademyEngine.interference.id -> UNLOCK_INTERFERENCE
        AcademyEngine.readingHr.id -> UNLOCK_LIVE_HR
        AcademyEngine.intervals.id -> UNLOCK_INTERVAL_PRESCRIPTION
        AcademyEngine.baseWithoutALab.id -> UNLOCK_BASE_TREND
        else -> null
    }

    /** Fired the first time the plateau ladder holds its fire because the athlete is cutting. */
    const val UNLOCK_CUT_STALL_SUPPRESSED = "coach.cut_stall_suppressed"

    /** App-usage moments — these write no coach row, which is why the ledger exists. */
    const val UNLOCK_COLD_START = "app.first_open"
    const val UNLOCK_FIRST_SET = "app.first_set_logged"
    const val UNLOCK_TECHNIQUE_SESSION = "app.technique_session"
    const val UNLOCK_FIRST_PROGRESSION = "coach.first_progression"
    const val UNLOCK_REST_TIMER = "app.rest_timer_used"
    const val UNLOCK_SORENESS_FLAG = "app.soreness_flagged"
    const val UNLOCK_WARMUP_SHOWN = "app.warmup_shown"
    const val UNLOCK_WEEK_BRIEF = "coach.first_week_brief"
    const val UNLOCK_READINESS_SHOWN = "coach.readiness_shown"
    const val UNLOCK_GOAL_CONFLICT = "coach.goal_conflict"
    const val UNLOCK_COLD_START_DONE = "app.cold_start_finished"

    /** Block moments (C). */
    const val UNLOCK_BLOCK_STARTED = "coach.block_started"
    const val UNLOCK_PHASE_CHANGE = "coach.block_phase_changed"
    const val UNLOCK_SCHEDULED_DELOAD = "coach.scheduled_deload"

    /** Learning-loop moments (D) — each fires when a personal number first changes a decision. */
    const val UNLOCK_PERSONAL_CAP = "coach.personal_volume_cap"
    const val UNLOCK_PERSONAL_SPACING = "coach.personal_spacing"
    const val UNLOCK_PERSONAL_REPS = "coach.personal_rep_range"
    const val UNLOCK_IMBALANCE_PROJECT = "coach.imbalance_project"
    const val UNLOCK_PROJECT_PROPOSED = "coach.project_proposed"

    /** Initiative moments (E). */
    const val UNLOCK_TIER_CHANGE = "coach.tier_changed"
    const val UNLOCK_AUTONOMOUS_ACT = "coach.acted_alone"

    /** Signal-slot moments (F). */
    const val UNLOCK_HRV_TREND = "signals.hrv_trend_available"

    /** Engine moments (E-A → E-D). */
    const val UNLOCK_CARDIO_PRESCRIPTION = "engine.first_prescription"
    const val UNLOCK_INTERFERENCE = "engine.interference_deduction"
    const val UNLOCK_LIVE_HR = "engine.live_hr_session"
    const val UNLOCK_INTERVAL_PRESCRIPTION = "engine.interval_prescription"
    const val UNLOCK_BASE_TREND = "engine.base_trend"
}
