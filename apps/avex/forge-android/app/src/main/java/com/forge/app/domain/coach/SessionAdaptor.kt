package com.forge.app.domain.coach

import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.program.MuscleGroup

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

    /** Minutes a working set plus its rest typically costs, for the time triage. */
    const val MINUTES_PER_SET = 3.0

    /** Never triage a session below this many exercises — at some point it isn't a session. */
    const val MIN_EXERCISES = 2

    /**
     * A session cut to fit the time available.
     *
     * @param keep the slots to do, in order, highest value first.
     * @param drop what was cut.
     * @param reason the coach's one line about what it kept and why.
     */
    data class Triage(
        val keep: List<ProgramSlotSnap>,
        val drop: List<ProgramSlotSnap>,
        val reason: String
    )

    /**
     * "I have N minutes." Keeps the highest-value work: goal-serving lifts first, then compounds,
     * then accessories — because the accessory you skip costs far less than the compound you rush.
     *
     * @param goalMuscles muscles the athlete's active goals care about; these survive triage first.
     */
    fun triage(
        slots: List<ProgramSlotSnap>,
        minutesAvailable: Int,
        goalMuscles: Set<MuscleGroup> = emptySet()
    ): Triage {
        if (slots.isEmpty()) return Triage(emptyList(), emptyList(), "Nothing planned to cut.")
        val ranked = slots.sortedWith(
            compareByDescending<ProgramSlotSnap> { it.muscle in goalMuscles }
                .thenByDescending { isCompound(it) }
                .thenByDescending { it.targetSets }
        )
        val budgetSets = (minutesAvailable / MINUTES_PER_SET).toInt().coerceAtLeast(0)

        val keep = mutableListOf<ProgramSlotSnap>()
        var used = 0
        for (slot in ranked) {
            if (used + slot.targetSets > budgetSets && keep.size >= MIN_EXERCISES) continue
            keep += slot
            used += slot.targetSets
        }
        // Keep the athlete's own ordering for what survives — a re-ordered session is a different
        // session, and the point here is to do less, not to do it differently.
        val kept = slots.filter { it in keep }
        val dropped = slots.filterNot { it in keep }
        val reason = when {
            dropped.isEmpty() -> "It all fits in $minutesAvailable minutes."
            else -> "Keeping the ${kept.size} that matter most and dropping ${dropped.size} for time."
        }
        return Triage(kept, dropped, reason)
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

    /** A compound is the work worth protecting when time runs out. */
    private fun isCompound(slot: ProgramSlotSnap): Boolean =
        slot.muscle in setOf(
            MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.QUADS,
            MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.SHOULDERS
        )
}
