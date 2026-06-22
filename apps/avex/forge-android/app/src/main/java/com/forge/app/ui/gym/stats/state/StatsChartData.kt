package com.forge.app.ui.gym.stats.state

/** One logged working set as a (reps, weight) point for the load-rep strength curve (Tier 6). */
data class RepWeightPoint(val reps: Int, val weightLb: Double)

/**
 * Per-exercise scatter of working sets plus the Epley estimated 1RM, feeding the load-rep curve:
 * every set as a dot (weight vs reps) with the fitted curve and the extrapolated 1RM point.
 */
data class StrengthCurve(
    val exerciseId: String,
    val exerciseName: String,
    val points: List<RepWeightPoint>,
    val e1rmLb: Double
)

/**
 * One day's training load — set count and tonnage. Feeds the adherence calendar heatmap (Tier 7) and
 * the Banister fitness/fatigue form curves (Tier 8). Only days with logged work are present.
 */
data class DayLoad(
    /** LocalDate.toEpochDay() — calendar day in the system zone at build time. */
    val epochDay: Long,
    val sets: Int,
    val volumeLb: Double
)
