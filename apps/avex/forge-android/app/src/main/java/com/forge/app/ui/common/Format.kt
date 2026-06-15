package com.forge.app.ui.common

/**
 * RPE rendered without a trailing ".0" — "8", "8.5". Single source for every surface that shows
 * an RPE (the set row, stats effort breakdown, session detail), so the rule can't drift.
 */
internal fun rpeLabel(rpe: Double): String =
    if (rpe % 1.0 == 0.0) "${rpe.toInt()}" else "%.1f".format(rpe)
