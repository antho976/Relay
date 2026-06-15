package com.forge.app.domain.rank

import com.forge.app.domain.units.formatVolumeCompact

/**
 * Pure XP computation. Every input is derivable from existing finished-session data, so XP carries
 * no new persistence (recomputed on profile open, like the trophy-points sum) — no DB migration.
 *
 * The weights below are the answer to "how do I gain XP", surfaced verbatim in the in-app
 * "How XP works" sheet via the returned [XpBreakdown.sources].
 */
data class XpSnapshot(
    val finishedSessions: Int,
    val totalSets: Int,
    val totalPrs: Int,
    val totalVolumeLb: Double,
    val activeWeeks: Int,   // distinct ISO weeks with ≥1 finished session
    val trophyPoints: Int   // Σ tier.points of unlocked trophies
)

/** One contributor to the XP total — drives the "how XP works" breakdown. */
data class XpSource(val label: String, val detail: String, val xp: Long)

data class XpBreakdown(val sources: List<XpSource>, val total: Long)

object XpEngine {
    const val WORKOUT_XP = 100L
    const val SET_XP = 4L
    const val PR_XP = 50L
    const val VOLUME_XP_PER_1000LB = 5L
    const val ACTIVE_WEEK_XP = 40L

    fun compute(s: XpSnapshot): XpBreakdown {
        val workouts = s.finishedSessions * WORKOUT_XP
        val sets = s.totalSets * SET_XP
        val prs = s.totalPrs * PR_XP
        val volume = (s.totalVolumeLb / 1000.0 * VOLUME_XP_PER_1000LB).toLong()
        val weeks = s.activeWeeks * ACTIVE_WEEK_XP
        val trophies = s.trophyPoints.toLong()

        val sources = listOf(
            XpSource("Workouts", "${s.finishedSessions} × $WORKOUT_XP", workouts),
            XpSource("Sets logged", "${s.totalSets} × $SET_XP", sets),
            XpSource("PRs", "${s.totalPrs} × $PR_XP", prs),
            // XP is computed per 1000 lb, so the breakdown shows the lb basis + rate for all users —
            // a kg figure with "× 5 (per 1000 lb)" wouldn't multiply out. Internal scoring detail.
            XpSource("Volume", "${formatVolumeCompact(s.totalVolumeLb, useKg = false)} × $VOLUME_XP_PER_1000LB", volume),
            XpSource("Consistency", "${s.activeWeeks} wk × $ACTIVE_WEEK_XP", weeks),
            XpSource("Trophies", "tier points", trophies)
        ).filter { it.xp > 0 }

        return XpBreakdown(sources, sources.sumOf { it.xp })
    }
}
