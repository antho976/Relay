package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.MuscleGroup

/** What program (re)generation learns from the coach's record (auto-coach: refresh ties in). */
data class GenBias(
    /** Net learned weekly-set adjustment per muscle, clamped to ±[CoachGenBias.VOLUME_CLAMP]. */
    val volumeBias: Map<MuscleGroup, Int> = emptyMap(),
    /** Rotations the watcher judged "ok" — boosted in selection like liked exercises. */
    val prefer: Set<String> = emptySet(),
    /** Rotations that failed (skipped after the change / user-reverted) — softly avoided. */
    val avoid: Set<String> = emptySet()
) {
    companion object {
        val NEUTRAL = GenBias()
    }
}

/**
 * Folds the coach's learned adjustments back into the generation BASELINE, so a re-roll or
 * regenerate doesn't forget them. Background: applied coach changes live as overlays
 * (rep/sets overrides, persistent swaps) which a wholesale regeneration deliberately clears
 * (the Cluster-A reconcile) — without this, every "refresh trainings" would silently undo
 * months of coach learning.
 *
 * Pure function of the coach decision history, recomputed from the SAME rows on every
 * generate — folding is idempotent, never compounding:
 *  - volumeBias: net of still-APPLIED volume_up/volume_down decisions per muscle (reverted and
 *    skipped rows drop out on their own), clamped to ±[VOLUME_CLAMP] — the same bound as the
 *    planner's drift cap (hardening 11).
 *  - prefer: swap replacements whose outcome the watcher judged "ok" — they earned their slot.
 *  - avoid: swap replacements that failed — the generator steers around them softly (it may
 *    still pick one if the muscle has nothing else; dislike remains the hard ban).
 */
object CoachGenBias {

    const val VOLUME_CLAMP = 2

    fun from(decisions: List<CoachDecision>): GenBias {
        if (decisions.isEmpty()) return GenBias.NEUTRAL

        val volume = decisions
            .filter { it.status == "applied" && it.type.startsWith("volume") }
            .mapNotNull { d ->
                ExerciseLibrary.byId(d.targetKey)?.muscle
                    ?.let { it to if (d.type == "volume_up") 1 else -1 }
            }
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, deltas) -> deltas.sum().coerceIn(-VOLUME_CLAMP, VOLUME_CLAMP) }
            .filterValues { it != 0 }

        val swaps = decisions.filter { it.type == "swap" && it.payload != null }
        val prefer = swaps.filter { it.status == "applied" && it.outcome == "ok" }
            .mapNotNull { it.payload }.toSet()
        val avoid = swaps.filter { it.outcome == "failed" }
            .mapNotNull { it.payload }.toSet() - prefer

        return GenBias(volumeBias = volume, prefer = prefer, avoid = avoid)
    }
}
