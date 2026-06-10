package com.forge.app.domain.adapt

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
}
