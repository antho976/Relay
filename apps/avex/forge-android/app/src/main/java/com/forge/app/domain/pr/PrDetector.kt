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
 * — bodyweight progression is tracked by reps, which we don't surface as a PR in 3c. Nor does a
 * zero weight, which the parser reads as a literal 0.0 rather than as "no weight".
 */
object PrDetector {

    /**
     * @param history Every previously logged set for the same exercise, across all sessions.
     *                Does NOT include the set being checked.
     * @param newWeightLb The proposed set's parsed weight in lb. Null = bodyweight; never a PR.
     * @param newReps The proposed set's rep count. Must be > 0.
     */
    fun isPr(history: List<LoggedSet>, newWeightLb: Double?, newReps: Int): Boolean {
        // A typed "0" parses to 0.0, not null (WeightParser accepts it as a literal weight), so it
        // slipped past the null guard and — with no prior set at this rep count — returned true
        // unconditionally. A brand-new exercise logged 0 x 5 in freestyle or from the watch got a
        // gold star and a lifetime PR count; with history, 0 x 20 "beat" 100 x 5. Zero load is not
        // a record at any rep count.
        if (newWeightLb == null || newWeightLb <= 0.0 || newReps <= 0) return false
        val competingMax = history
            // Assisted sets (bands/spotter) are excluded from all-time PR comparison (LoggedSet.isAssisted).
            .filter { it.weightLb != null && !it.isAssisted && it.reps >= newReps }
            .maxOfOrNull { it.weightLb!! }
            ?: return true // No prior set at >= this rep count — any weight is a PR
        return newWeightLb > competingMax
    }
}
