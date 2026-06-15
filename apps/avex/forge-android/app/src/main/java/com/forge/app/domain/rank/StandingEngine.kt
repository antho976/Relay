package com.forge.app.domain.rank

import com.forge.app.domain.units.formatVolumeCompact

/**
 * Estimated standing "vs typical lifters". The app is offline / single-user, so there are no real
 * other athletes — each metric is mapped through a documented population model and reported as a
 * "Top X%" band. "Top 12%" means "better than ~88% of typical lifters", an estimate, never a live
 * comparison. Pure + deterministic.
 */
data class StandingMetric(
    val key: String,
    val label: String,
    val valueText: String,
    val topPercent: Int  // 2..99, lower = better
)

/** Inputs computed over the last 90 days (the comparison window). */
data class StandingSnapshot(
    val sessionsPerWeek: Double,
    val streakWeeks: Int,
    val weeklyVolumeLb: Double
)

object StandingEngine {

    // (metricValue, topPercent) anchors, ascending by value; piecewise-linear between them.
    // Anchors encode a plausible "typical lifter" distribution — documented assumptions, not data.
    private val CONSISTENCY = listOf(
        0.0 to 95, 1.0 to 70, 2.0 to 50, 3.0 to 30, 4.0 to 15, 5.0 to 7, 6.0 to 3
    )
    private val STREAK = listOf(
        0.0 to 90, 1.0 to 60, 4.0 to 35, 8.0 to 18, 12.0 to 9, 20.0 to 4, 40.0 to 2
    )
    private val VOLUME = listOf(
        0.0 to 95, 5_000.0 to 70, 12_000.0 to 50, 25_000.0 to 28, 45_000.0 to 12, 70_000.0 to 6, 110_000.0 to 2
    )

    fun standings(s: StandingSnapshot, useKg: Boolean): List<StandingMetric> = listOf(
        StandingMetric("consistency", "Consistency", "${fmt1(s.sessionsPerWeek)}×/wk", pct(CONSISTENCY, s.sessionsPerWeek)),
        StandingMetric("streak", "Streak length", "${s.streakWeeks} wk", pct(STREAK, s.streakWeeks.toDouble())),
        StandingMetric("volume", "Weekly volume", formatVolumeCompact(s.weeklyVolumeLb, useKg), pct(VOLUME, s.weeklyVolumeLb))
    )

    /** Piecewise-linear interpolation between anchors, clamped to [2, 99]. */
    private fun pct(anchors: List<Pair<Double, Int>>, v: Double): Int {
        if (v <= anchors.first().first) return anchors.first().second
        if (v >= anchors.last().first) return anchors.last().second
        for (i in 0 until anchors.size - 1) {
            val (x0, y0) = anchors[i]
            val (x1, y1) = anchors[i + 1]
            if (v in x0..x1) {
                val t = (v - x0) / (x1 - x0)
                return (y0 + t * (y1 - y0)).toInt().coerceIn(2, 99)
            }
        }
        return anchors.last().second
    }

    private fun fmt1(d: Double): String = if (d % 1.0 == 0.0) d.toInt().toString() else "%.1f".format(d)
}
