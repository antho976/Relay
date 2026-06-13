package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.MuscleGroup

/** What program (re)generation learns from the coach's record (auto-coach: refresh ties in). */
data class GenBias(
    /** Net learned weekly-set adjustment per muscle, clamped to ±[CoachGenBias.VOLUME_CLAMP]. */
    val volumeBias: Map<MuscleGroup, Int> = emptyMap(),
    /**
     * Learned rep-range override per exercise (targetKey → reps string), latest applied/folded
     * rep_shift wins. Generation applies it to any slot that re-picks that exercise, so a coach
     * rep_shift survives a refresh the way volume does (seam fix, finding 1 — rep_shift previously
     * had no survival channel and was silently discarded on every regenerate).
     */
    val repBias: Map<String, String> = emptyMap(),
    /** Rotations the watcher judged "ok" (or already folded into the baseline) — boosted like likes. */
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
 * (the Cluster-A reconcile, plus [ExerciseCustomizationDao.clearCoachSwaps] for swaps) — without
 * this, every "refresh trainings" would silently undo months of coach learning. At regenerate the
 * cleared deltas are also marked `folded` (a terminal status that keeps feeding this bias but is no
 * longer per-change undoable), so the bias is the single carrier and undo can't replay stale state.
 *
 * Pure function of the coach decision history, recomputed from the SAME rows on every generate —
 * folding is idempotent, never compounding. Counts both `applied` (live overlay) and `folded`
 * (already baked) rows, since a row is exactly one of those once accepted:
 *  - volumeBias: net per muscle of non-failed volume_up/volume_down (reverted/skipped/failed rows
 *    drop out), clamped to ±[VOLUME_CLAMP] — the same bound as the planner's drift cap (hardening 11).
 *  - repBias: latest non-failed rep_shift per exercise.
 *  - prefer: swap replacements judged "ok", OR already folded into the baseline (folding stops the
 *    watcher, so a folded swap can't reach "ok" — it earned its slot by being accepted + absorbed).
 *  - avoid: swap replacements that failed — the generator steers around them softly (it may still
 *    pick one if the muscle has nothing else; dislike remains the hard ban).
 */
object CoachGenBias {

    const val VOLUME_CLAMP = 2

    private fun CoachDecision.isActive() = status == "applied" || status == "folded"

    fun from(decisions: List<CoachDecision>): GenBias {
        if (decisions.isEmpty()) return GenBias.NEUTRAL

        val volume = decisions
            .filter { it.isActive() && it.type.startsWith("volume") && it.outcome != "failed" }
            .mapNotNull { d ->
                ExerciseLibrary.byId(d.targetKey)?.muscle
                    ?.let { it to if (d.type == "volume_up") 1 else -1 }
            }
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, deltas) -> deltas.sum().coerceIn(-VOLUME_CLAMP, VOLUME_CLAMP) }
            .filterValues { it != 0 }

        // Latest non-failed rep_shift per exercise wins (associate keeps the last entry; ascending id
        // order makes "last" the most recent).
        val repBias = decisions
            .filter { it.isActive() && it.type == "rep_shift" && it.outcome != "failed" && it.payload != null }
            .sortedBy { it.id }
            .associate { it.targetKey to it.payload!! }

        val swaps = decisions.filter { it.type == "swap" && it.payload != null }
        val prefer = swaps
            .filter { it.status == "folded" || (it.status == "applied" && it.outcome == "ok") }
            .mapNotNull { it.payload }.toSet()
        val avoid = swaps.filter { it.outcome == "failed" }
            .mapNotNull { it.payload }.toSet() - prefer

        return GenBias(volumeBias = volume, repBias = repBias, prefer = prefer, avoid = avoid)
    }
}
