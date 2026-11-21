package com.forge.app.ui.profile

import com.forge.app.data.repo.ProgressPhoto
import java.time.ZoneId
import kotlin.math.abs

// ── Same weight, different body ──────────────────────────────────────────────
// (Drawn as cards in the compare strip; see [compareSuggestions].)

/**
 * Two shots taken at the SAME bodyweight but far enough apart to show a different physique — the
 * "scale held, the body didn't" comparison. [before]/[after] are ordered oldest→newest and
 * [avgWeightLb] is their shared weight (the two differ by at most [SAME_WEIGHT_TOL_LB]).
 */
internal data class SameWeightPair(
    val before: ProgressPhoto,
    val after: ProgressPhoto,
    val avgWeightLb: Double,
    val daysApart: Long
)

// The scale reads essentially the same within this band (about a normal day-to-day fluctuation).
private const val SAME_WEIGHT_TOL_LB = 2.0
// Two shots must sit this far apart for a body change to be worth showing (a plateau, not a week).
private const val MIN_DAYS_APART = 30L
private const val MAX_SAME_WEIGHT_PAIRS = 3

/**
 * Auto-detect "same weight, different body" pairs among [photos]: two shots of the SAME pose (so the
 * angle compares fairly) whose snapshotted bodyweight is within [SAME_WEIGHT_TOL_LB] yet taken at
 * least [MIN_DAYS_APART] apart. Ranked longest-span first (the most dramatic hold), then closest
 * weight, and kept to distinct photos so each surfaced card is its own comparison. Photos with no
 * weight snapshot can't make the claim, so they're skipped; [exclude] drops the one pair the progress
 * band already shows (it carries "SAME WT" itself when its ends match), so the section never echoes it.
 */
internal fun sameWeightPairs(
    photos: List<ProgressPhoto>,
    zone: ZoneId,
    exclude: Set<String> = emptySet()
): List<SameWeightPair> {
    val weighed = photos.filter { it.weightLb != null }
    if (weighed.size < 2) return emptyList()

    // Three greedy selections preserve the original ranking without allocating every pair.
    // Calendar conversion happens once per photo instead of once per candidate.
    val dates = weighed.map { java.time.Instant.ofEpochMilli(it.takenAtMs).atZone(zone).toLocalDate().toEpochDay() }
    val used = HashSet<String>()
    val picked = ArrayList<SameWeightPair>(MAX_SAME_WEIGHT_PAIRS)
    repeat(MAX_SAME_WEIGHT_PAIRS) {
        var bestI = -1
        var bestJ = -1
        var bestDays = -1L
        var bestGap = Double.POSITIVE_INFINITY
        for (i in weighed.indices) {
            val a = weighed[i]
            if (a.fileName in used) continue
            for (j in i + 1 until weighed.size) {
                val b = weighed[j]
                if (b.fileName in used || a.pose != b.pose) continue
                if (exclude.size == 2 && a.fileName in exclude && b.fileName in exclude) continue
                val gap = abs(a.weightLb!! - b.weightLb!!)
                if (gap > SAME_WEIGHT_TOL_LB || !gap.isFinite()) continue
                val days = abs(dates[i] - dates[j])
                if (days < MIN_DAYS_APART) continue
                if (days > bestDays || (days == bestDays && gap < bestGap)) {
                    bestI = i; bestJ = j; bestDays = days; bestGap = gap
                }
            }
        }
        if (bestI < 0) return picked
        val a = weighed[bestI]
        val b = weighed[bestJ]
        val (before, after) = if (a.takenAtMs <= b.takenAtMs) a to b else b to a
        picked += SameWeightPair(before, after, (a.weightLb!! + b.weightLb!!) / 2.0, bestDays)
        used += a.fileName
        used += b.fileName
    }
    return picked
}
