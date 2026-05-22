package com.forge.app.domain.pr

import com.forge.app.data.db.entities.LoggedSet

/**
 * Personal-record detection. A set counts as a PR when its `weight_lb` is strictly
 * greater than every previously logged set for the same exercise that was performed
 * at the same-or-higher rep count.
 *
 * Rationale: "more weight at this rep range" is the standard strength-training
 * definition of a PR. Comparing only against same-or-higher rep sets keeps low-rep
 * heavy work from being trivially "beaten" by high-rep light work (and vice versa).
 *
 * Sets without a numeric weight (e.g. "BW", parser returned null) never count as PRs
 * — bodyweight progression is tracked by reps, which we don't surface as a PR in 3c.
 */
object PrDetector {

    /**
     * @param history Every previously logged set for the same exercise, across all sessions.
     *                Does NOT include the set being checked.
     * @param newWeightLb The proposed set's parsed weight in lb. Null = bodyweight; never a PR.
     * @param newReps The proposed set's rep count. Must be > 0.
     */
    fun isPr(history: List<LoggedSet>, newWeightLb: Double?, newReps: Int): Boolean {
        if (newWeightLb == null || newReps <= 0) return false
        val competingMax = history
            .filter { it.weightLb != null && it.reps >= newReps }
            .maxOfOrNull { it.weightLb!! }
            ?: return true // No prior set at >= this rep count — any weight is a PR
        return newWeightLb > competingMax
    }
}
