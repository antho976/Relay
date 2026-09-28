package com.forge.app.domain.coach

import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.program.MuscleGroup
import com.forge.app.program.SessionEstimate

/**
 * Mid-session re-planning (Coach v3 E) — the "what now?" eliminator.
 *
 * Three things go wrong inside a session, and all three currently cost the athlete a decision:
 * the equipment is taken, the time ran out, or something hurts. Each gets an instant answer here.
 *
 * Pure: this decides WHAT to change; the caller applies it through the existing swap and reorder
 * paths, so every change still goes through a user-confirmed write with normal undo.
 */
object SessionAdaptor {

    /** Never cut a session below this many exercises — at some point it isn't a session. */
    const val MIN_EXERCISES = 2

    /**
     * A session re-planned around a problem.
     *
     * @param keep the slots to do, in the session's own order.
     * @param drop what was cut.
     * @param reason the coach's one line about what it kept and why.
     */
    data class Triage(
        val keep: List<ProgramSlotSnap>,
        val drop: List<ProgramSlotSnap>,
        val reason: String
    )

    /**
     * One exercise still to do today, priced for [fitToTime]. [remainingSets] is what's left of the
     * target (a half-done exercise costs only its remaining sets); [started] means sets are already
     * logged against it, so it is never the one cut.
     */
    data class TimedExercise(
        val id: String,
        val remainingSets: Int,
        val restSeconds: Int,
        val compound: Boolean,
        val started: Boolean = false
    ) {
        val seconds: Int get() = SessionEstimate.exerciseSeconds(remainingSets, restSeconds)
    }

    /** What survives an "I have N minutes" fit, both lists in the session's own order. */
    data class TimeFit(val keepIds: List<String>, val dropIds: List<String>)

    /**
     * The day screen's "I have N minutes today". Priced by the same rule as the day card's "~min"
     * ([SessionEstimate.exerciseSeconds]), not a flat per-set guess, so "20 minutes" means the same
     * thing on both screens. Keeps, in order: anything already started, then compounds, then the
     * rest in the order the day lists them (a generated day lists its most important work first).
     * Whatever doesn't fit is dropped, but never below [MIN_EXERCISES] kept.
     */
    fun fitToTime(exercises: List<TimedExercise>, minutesAvailable: Int): TimeFit {
        val budget = minutesAvailable.coerceAtLeast(0) * 60
        val ranked = exercises.withIndex().sortedWith(
            compareByDescending<IndexedValue<TimedExercise>> { it.value.started }
                .thenByDescending { it.value.compound }
                .thenBy { it.index }
        ).map { it.value }
        val keep = HashSet<String>()
        var used = 0
        for (ex in ranked) {
            val mustKeep = ex.started || keep.size < MIN_EXERCISES
            if (!mustKeep && used + ex.seconds > budget) continue
            keep += ex.id
            used += ex.seconds
        }
        return TimeFit(
            keepIds = exercises.map { it.id }.filter { it in keep },
            dropIds = exercises.map { it.id }.filterNot { it in keep }
        )
    }

    /**
     * The equipment is taken. Offers the slot's existing swap candidates — the pool is already
     * equipment- and dislike-filtered at snapshot time, so anything here is something the athlete
     * can actually do right now.
     */
    fun swapCandidates(slot: ProgramSlotSnap, life: LifeEvents.State, limit: Int = 3): List<String> =
        slot.swapCandidateIds
            .filterNot { life.isRestricted(it) }
            .take(limit)

    /**
     * Something hurts. Returns what to do with the session: which slots to drop, and the line to
     * say. Deliberately NOT a weight adjustment — mid-session pain is not a load problem.
     */
    fun soreReroute(
        slots: List<ProgramSlotSnap>,
        muscle: MuscleGroup,
        life: LifeEvents.State
    ): Triage {
        val affected = slots.filter { it.muscle == muscle }
        if (affected.isEmpty()) {
            return Triage(slots, emptyList(), "Nothing left today loads your ${muscle.displayName.lowercase()}.")
        }
        val keep = slots - affected.toSet()
        val restricted = life.isRestricted(muscle)
        val reason = if (restricted) {
            "That's flagged as injured, so it's out of today's session entirely."
        } else {
            "Dropping the ${muscle.displayName.lowercase()} work for today. The rest of the session stands."
        }
        return Triage(keep, affected, reason)
    }

    /**
     * Sets already logged block a swap (the existing rule, `DaySwapHandlers`: re-keying would
     * mis-attribute them). So mid-exercise the honest answer is to finish that exercise short and
     * substitute the NEXT one, rather than relaxing a rule that protects the log.
     */
    fun canSwapCleanly(setsLogged: Int): Boolean = setsLogged == 0
}
