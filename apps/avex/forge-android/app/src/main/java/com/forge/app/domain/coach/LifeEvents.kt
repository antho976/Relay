package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.entities.InjuryRestriction
import com.forge.app.data.db.entities.Session
import com.forge.app.program.MuscleGroup

/**
 * Illness, injury and time away (Coach v3 B1) — the most common adjustment a real coach makes, and
 * the one v2 had no story for at all.
 *
 * Without this, a flu week reads as unexplained stalls and rising fatigue, and a fortnight abroad
 * reads as a plateau on every lift. The coach then "corrects" a problem that was never training.
 *
 * Pure and deterministic like every other advisor. Three mechanisms:
 *  - **Sick**: one flag, one source of truth. The check-in owns it, and the legacy sick REST-day
 *    reason feeds the same flag rather than a parallel deduction (plan M6).
 *  - **Layoff**: a real gap in training, from raw session dates. Declared vacations already pause
 *    the weekly pass; this handles the *return*, and the gaps nobody declared.
 *  - **Injury restriction**: a muscle or movement that's off the table until cleared, distinct
 *    from acute soreness and routed around rather than silently avoided.
 */
object LifeEvents {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** A break this long stops being "life happened this week" and becomes detraining. */
    const val LAYOFF_MIN_DAYS = 14

    /** How long the eased return lasts once training restarts. */
    const val RAMP_DAYS = 7

    /** How much the first sessions back hold off — enough to matter, not enough to insult. */
    const val RAMP_SCALE = 0.9

    /** A sick flag speaks for this long; illness doesn't end the moment you stop logging it. */
    const val SICK_WINDOW_DAYS = 3

    /** Soreness reported today gates that muscle for this long. */
    const val SORENESS_WINDOW_HOURS = 36

    /** A gap in training, and where the athlete is in it. */
    data class Layoff(
        val days: Int,
        /** Still away: no session since the gap started. */
        val away: Boolean,
        /** Back inside the ramp window — loads ease, and the first session is FIRST_BACK. */
        val returning: Boolean,
        /** When training resumed, if it has. */
        val returnedAtMs: Long?,
        /** Start of the gap (the last session before it), for verdict suppression. */
        val gapStartMs: Long
    )

    /** Everything the coach needs to know about the athlete's life today. */
    data class State(
        val sick: Boolean,
        val layoff: Layoff?,
        val soreMuscles: Set<MuscleGroup>,
        val restrictedMuscles: Set<MuscleGroup>,
        val restrictedExerciseIds: Set<String>
    ) {
        /** True when the coach should hold back rather than push: illness or a fresh return. */
        val easeOff: Boolean get() = sick || layoff?.returning == true || layoff?.away == true

        /** Scale to apply to prescribed loads, 1.0 when nothing applies. */
        val loadScale: Double get() = if (layoff?.returning == true) RAMP_SCALE else 1.0

        /** A muscle is off-limits when it's injured; sore muscles are gated, not banned. */
        fun isRestricted(muscle: MuscleGroup): Boolean = muscle in restrictedMuscles

        fun isRestricted(exerciseId: String): Boolean = exerciseId in restrictedExerciseIds

        companion object {
            val NONE = State(
                sick = false, layoff = null, soreMuscles = emptySet(),
                restrictedMuscles = emptySet(), restrictedExerciseIds = emptySet()
            )
        }
    }

    /**
     * Read the athlete's current life state.
     *
     * @param sessions finished sessions, any order.
     * @param checkins recent check-ins, any order.
     * @param cardio recent cardio rows — the legacy sick rest-day reason feeds the same flag.
     * @param restrictions injury restrictions (active ones are used).
     */
    fun assess(
        sessions: List<Session>,
        checkins: List<CheckinEntry>,
        cardio: List<CardioEntry>,
        restrictions: List<InjuryRestriction>,
        nowMs: Long
    ): State {
        val sickFromCheckin = checkins
            .filter { it.sick && nowMs - it.recordedAt <= SICK_WINDOW_DAYS * DAY_MS }
            .isNotEmpty()
        // The rest-day "sick" reason predates the flag and users still reach for it — same signal,
        // one flag, so illness can never be counted twice (M6).
        val sickFromCardio = cardio
            .any { it.restReason == "sick" && nowMs - it.date <= SICK_WINDOW_DAYS * DAY_MS }

        val sore = checkins
            .filter { nowMs - it.recordedAt <= SORENESS_WINDOW_HOURS * 60L * 60 * 1000 }
            .flatMap { entry -> entry.soreMuscles.split(",").mapNotNull { code -> muscle(code.trim()) } }
            .toSet()

        val active = restrictions.filter { it.isActive }

        return State(
            sick = sickFromCheckin || sickFromCardio,
            layoff = layoff(sessions, nowMs),
            soreMuscles = sore,
            restrictedMuscles = active
                .filter { it.scope == InjuryRestriction.SCOPE_MUSCLE }
                .mapNotNull { muscle(it.targetKey) }
                .toSet(),
            restrictedExerciseIds = active
                .filter { it.scope == InjuryRestriction.SCOPE_EXERCISE }
                .map { it.targetKey }
                .toSet()
        )
    }

    /**
     * The current layoff, or null when training has been continuous.
     *
     * Two shapes: still away (nothing logged for ≥ [LAYOFF_MIN_DAYS]), or back inside the ramp
     * window after such a gap. Deliberately measured in RAW days — a declared holiday is still
     * time your body spent not training, even though the weekly pass paused for it.
     */
    fun layoff(sessions: List<Session>, nowMs: Long): Layoff? {
        val finished = sessions.filter { it.finishedAt != null && !it.isUntracked }
            .sortedBy { it.startedAt }
        if (finished.isEmpty()) return null

        val last = finished.last()
        val daysSinceLast = ((nowMs - last.startedAt) / DAY_MS).toInt()
        if (daysSinceLast >= LAYOFF_MIN_DAYS) {
            return Layoff(
                days = daysSinceLast, away = true, returning = false,
                returnedAtMs = null, gapStartMs = last.startedAt
            )
        }

        // Not away now — but did training restart recently after a real gap?
        for (i in finished.indices.reversed()) {
            if (i == 0) break
            val session = finished[i]
            val previous = finished[i - 1]
            val gapDays = ((session.startedAt - previous.startedAt) / DAY_MS).toInt()
            if (gapDays >= LAYOFF_MIN_DAYS) {
                val withinRamp = nowMs - session.startedAt <= RAMP_DAYS * DAY_MS
                return Layoff(
                    days = gapDays,
                    away = false,
                    returning = withinRamp,
                    returnedAtMs = session.startedAt,
                    gapStartMs = previous.startedAt
                )
            }
            // Stop scanning once we're older than one ramp window plus the gap we'd care about.
            if (nowMs - session.startedAt > (RAMP_DAYS + LAYOFF_MIN_DAYS) * DAY_MS) break
        }
        return null
    }

    /**
     * Should a coach decision's outcome be judged, given what the athlete's life did to its window?
     *
     * A suggestion applied before a three-week break can't be judged on the sessions that never
     * happened — that's not a failed suggestion, it's an absent athlete. Same for a decision whose
     * window sat inside an illness. Suppressed decisions are the "not followed" verdict's natural
     * home (plan M2): recorded, watched, but never counted against the coach.
     */
    fun suppressesVerdict(appliedAtMs: Long, windowEndMs: Long, state: State): Boolean {
        if (state.sick) return true
        val layoff = state.layoff ?: return false
        // The window overlaps the gap when the decision predates the gap's end and the window
        // outlives its start.
        val gapEnd = layoff.returnedAtMs ?: Long.MAX_VALUE
        return appliedAtMs <= gapEnd && windowEndMs >= layoff.gapStartMs
    }

    /** The coach's own words for the current state, or null when life is uneventful. */
    fun explain(state: State): String? = when {
        state.sick -> "You flagged being unwell, so nothing is being pushed until that passes."
        state.layoff?.away == true ->
            "It's been ${state.layoff.days} days. When you're back, the first week eases in."
        state.layoff?.returning == true ->
            "First week back after ${state.layoff.days} days off, so loads hold about 10% under."
        state.restrictedMuscles.isNotEmpty() || state.restrictedExerciseIds.isNotEmpty() ->
            "Working around what you flagged as injured until you clear it."
        else -> null
    }

    private fun muscle(code: String): MuscleGroup? =
        MuscleGroup.entries.firstOrNull { it.code == code }
}
