package com.forge.app.ui.common

/**
 * RPE rendered without a trailing ".0" — "8", "8.5". Single source for every surface that shows
 * an RPE (the set row, stats effort breakdown, session detail), so the rule can't drift.
 */
internal fun rpeLabel(rpe: Double): String =
    if (rpe % 1.0 == 0.0) "${rpe.toInt()}" else "%.1f".format(rpe)

/**
 * Reps-in-reserve implied by an RPE (RIR ≈ 10 − RPE), rendered without a trailing ".0".
 * RPE and RIR are the same effort axis seen from opposite ends — RPE 10 = 0 RIR, RPE 8 = 2 RIR.
 */
internal fun rirLabel(rpe: Double): String {
    val rir = (10.0 - rpe).coerceAtLeast(0.0)
    return if (rir % 1.0 == 0.0) "${rir.toInt()}" else "%.1f".format(rir)
}
