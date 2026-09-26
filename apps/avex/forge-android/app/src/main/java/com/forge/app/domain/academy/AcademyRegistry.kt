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
 * Since 2026-09-26 the Academy ships 12 lessons in three chapters, cut down from 35 pieces. The
 * retired ids live on in [aliases], because the ledger and the coach's reasons still carry them.
 */
object AcademyRegistry {

    /** Every lesson that currently ships, in page order: Training, then Your coach, then Cardio. */
    val lessons: List<Lesson> =
        AcademyTraining.ordered + AcademyCoachLessons.ordered + AcademyCardio.ordered

    /**
     * Retired ids and the lesson that absorbed each one (2026-09-26, 35 pieces cut to 12).
     *
     * Coach reasons, notices and the append-only ledger still carry the old ids, and the ledger can
     * never be rewritten. So an old id is resolved here instead: opening it opens the lesson that
     * replaced it, and an old unlock or read counts toward that lesson.
     */
    val aliases: Map<String, String> = mapOf(
        "fundamentals.what_a_program_is" to AcademyTraining.gettingStronger.id,
        "fundamentals.progressive_overload" to AcademyTraining.gettingStronger.id,
        "fundamentals.sets_reps_rpe" to AcademyTraining.effort.id,
        "programming.sweet_spot_reps" to AcademyTraining.effort.id,
        "library.proximity_to_failure" to AcademyTraining.effort.id,
        "programming.your_volume_landmarks" to AcademyTraining.volume.id,
        "programming.imbalances" to AcademyTraining.volume.id,
        "library.how_much_volume" to AcademyTraining.volume.id,
        "fundamentals.form_vs_load" to AcademyTraining.form.id,
        "fundamentals.warmups" to AcademyTraining.form.id,
        "fundamentals.rest_and_recovery" to AcademyTraining.recovery.id,
        "programming.your_recovery_curve" to AcademyTraining.recovery.id,
        "library.sleep_and_training" to AcademyTraining.recovery.id,
        "fundamentals.soreness_vs_injury" to AcademyTraining.soreness.id,
        "coach.strength_on_a_cut" to AcademyTraining.protein.id,
        "library.protein_intake" to AcademyTraining.protein.id,
        "fundamentals.how_the_coach_works" to AcademyCoachLessons.howItDecides.id,
        "fundamentals.log_honestly" to AcademyCoachLessons.howItDecides.id,
        "coach.trust_tiers" to AcademyCoachLessons.howItDecides.id,
        "coach.taking_decisions_back" to AcademyCoachLessons.howItDecides.id,
        "fundamentals.what_readiness_means" to AcademyCoachLessons.readiness.id,
        "coach.readiness_built_from" to AcademyCoachLessons.readiness.id,
        "signals.stress_hrv" to AcademyCoachLessons.readiness.id,
        "programming.what_a_block_is" to AcademyCoachLessons.blocks.id,
        "programming.four_phases" to AcademyCoachLessons.blocks.id,
        "programming.deloads_are_earned" to AcademyCoachLessons.blocks.id,
        "programming.reading_your_block_card" to AcademyCoachLessons.blocks.id,
        "coach.why_goals_fight" to AcademyCoachLessons.blocks.id,
        "coach.what_a_project_is" to AcademyCoachLessons.blocks.id,
        "engine.why_aerobic_base" to AcademyCardio.zone2.id,
        "engine.what_zone2_is" to AcademyCardio.zone2.id,
        "engine.reading_hr" to AcademyCardio.zone2.id,
        "engine.base_without_a_lab" to AcademyCardio.zone2.id,
        "engine.intervals" to AcademyCardio.intervals.id,
        "engine.interference" to AcademyCardio.intervals.id
    )

    /** The id a lesson ships under today, for a current or a retired id. */
    fun canonical(id: String): String = aliases[id] ?: id

    /**
     * The cold-start curriculum, in reading order (B3). Below the coach's data gates these lessons
     * ARE the Today Directive: the card degrades from personalised to principled, never to silence.
     */
    val coldStartTrack: List<Lesson> = AcademyTraining.ordered

    /**
     * The next cold-start lesson for a reader — the one the directive should carry today. Null once
     * the track is finished, at which point the coach has enough data to speak for itself.
     */
    fun nextColdStartLesson(events: List<com.forge.app.data.db.entities.LessonEvent>): Lesson? {
        val opened = events.filter { it.kind == LessonEventKind.OPENED.code }
            .map { canonical(it.lessonId) }
            .toSet()
        return coldStartTrack.firstOrNull { it.id !in opened }
    }

    fun lesson(id: String): Lesson? = canonical(id).let { c -> lessons.firstOrNull { it.id == c } }

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
        val byLesson = events.groupBy { canonical(it.lessonId) }
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
        canonical(lessonId).let { id -> stateFrom(events).firstOrNull { it.lesson.id == id } }

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
    fun unlockKeyFor(lessonId: String): String? = when (canonical(lessonId)) {
        AcademyTraining.gettingStronger.id -> UNLOCK_COLD_START
        AcademyTraining.effort.id -> UNLOCK_FIRST_SET
        AcademyTraining.volume.id -> UNLOCK_PERSONAL_CAP
        AcademyTraining.form.id -> UNLOCK_WARMUP_SHOWN
        AcademyTraining.recovery.id -> UNLOCK_REST_TIMER
        AcademyTraining.soreness.id -> UNLOCK_SORENESS_FLAG
        AcademyTraining.protein.id -> UNLOCK_CUT_STALL_SUPPRESSED
        AcademyCoachLessons.howItDecides.id -> UNLOCK_WEEK_BRIEF
        AcademyCoachLessons.readiness.id -> UNLOCK_READINESS_SHOWN
        AcademyCoachLessons.blocks.id -> UNLOCK_BLOCK_STARTED
        AcademyCardio.zone2.id -> UNLOCK_CARDIO_PRESCRIPTION
        AcademyCardio.intervals.id -> UNLOCK_INTERVAL_PRESCRIPTION
        else -> null
    }

    /** Fired the first time the plateau ladder holds its fire because the athlete is cutting. */
    const val UNLOCK_CUT_STALL_SUPPRESSED = "coach.cut_stall_suppressed"

    /** App-usage moments — these write no coach row, which is why the ledger exists. */
    const val UNLOCK_COLD_START = "app.first_open"
    const val UNLOCK_FIRST_SET = "app.first_set_logged"
    const val UNLOCK_REST_TIMER = "app.rest_timer_used"
    const val UNLOCK_SORENESS_FLAG = "app.soreness_flagged"
    const val UNLOCK_WARMUP_SHOWN = "app.warmup_shown"
    const val UNLOCK_WEEK_BRIEF = "coach.first_week_brief"
    const val UNLOCK_READINESS_SHOWN = "coach.readiness_shown"

    /** Block moments (C). */
    const val UNLOCK_BLOCK_STARTED = "coach.block_started"

    /** Learning-loop moments (D) — each fires when a personal number first changes a decision. */
    const val UNLOCK_PERSONAL_CAP = "coach.personal_volume_cap"

    /** Engine moments (E-A → E-D). */
    const val UNLOCK_CARDIO_PRESCRIPTION = "engine.first_prescription"
    const val UNLOCK_INTERVAL_PRESCRIPTION = "engine.interval_prescription"
}
