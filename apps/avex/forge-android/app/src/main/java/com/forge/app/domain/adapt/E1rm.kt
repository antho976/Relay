package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet

/**
 * Estimated one-rep max via the Epley formula: `w × (1 + reps/30)`.
 *
 * Used as the progression/plateau yardstick because it collapses weight × reps into one
 * comparable number — "did 45×10 this week beat 47.5×8 last week?" has a single answer.
 * Epley over alternatives (Brzycki etc.) because it's the simplest, it's monotonic in both
 * inputs, and we only ever compare e1RMs against each other — absolute accuracy doesn't
 * matter, ordering does.
 */
object E1rm {

    fun epley(weightLb: Double, reps: Int): Double =
        if (reps <= 1) weightLb else weightLb * (1 + reps / 30.0)

    /**
     * Inverse of [epley]: the weight liftable for [reps] reps given an estimated 1RM — used to draw the
     * fitted strength curve from an e1RM. reps ≤ 1 returns the e1RM itself (the projected single),
     * mirroring [epley]'s reps ≤ 1 branch so the two never diverge.
     */
    fun epleyInverse(e1rmLb: Double, reps: Int): Double =
        if (reps <= 1) e1rmLb else e1rmLb / (1 + reps / 30.0)
}

// ── Shared "best working e1RM" helpers ─────────────────────────────────────────
// One definition of "the heaviest estimated 1RM across the weighted, non-assisted (working) sets",
// reused by ProgressionAdvisor, AutoCoachPlanner, OutcomeWatcher and the InsightEngine so a formula
// change lives in one place. Returns null when no working set qualifies.

fun bestWorkingE1rm(sets: List<LoggedSet>): Double? =
    sets.filter { it.weightLb != null && !it.isAssisted }
        .maxOfOrNull { E1rm.epley(it.weightLb!!, it.reps) }

/** Best working e1RM in a single bout. */
fun ExerciseBout.bestE1rm(): Double? = bestWorkingE1rm(sets)

/** Best working e1RM across a set of bouts (caller filters skipped if needed). */
fun List<ExerciseBout>.bestE1rm(): Double? = bestWorkingE1rm(flatMap { it.sets })
