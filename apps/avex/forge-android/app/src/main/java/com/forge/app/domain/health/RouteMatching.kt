package com.forge.app.domain.health

/** One Health Connect exercise session reduced to its time window + a stable list index. */
data class SessionWindow(val index: Int, val startMs: Long, val endMs: Long)

/**
 * Pick the [SessionWindow] that best corresponds to a Avex cardio entry, or null if none is close
 * enough. Avex cardio is logged by hand (the user picks a time and a duration), so it rarely lines
 * up exactly with a watch's recorded session — we match on **time overlap** first, then fall back to
 * the nearest start within [toleranceMs] when nothing overlaps (e.g. the user logged a round time
 * after the run). Returns the winning session's [SessionWindow.index].
 *
 * Pure and dependency-free so it's unit-testable; the HealthConnect read maps records into
 * [SessionWindow]s and feeds them here.
 */
fun bestSessionMatch(
    entryStartMs: Long,
    entryDurationMin: Int,
    sessions: List<SessionWindow>,
    toleranceMs: Long = DEFAULT_MATCH_TOLERANCE_MS
): Int? {
    if (sessions.isEmpty()) return null
    // A zero/negative duration is a point in time; give it a 1-minute window so overlap maths work.
    val entryEndMs = entryStartMs + (entryDurationMin.coerceAtLeast(1) * 60_000L)

    var bestOverlap = 0L
    var bestOverlapIdx: Int? = null
    var nearestGap = Long.MAX_VALUE
    var nearestIdx: Int? = null

    for (s in sessions) {
        val overlap = minOf(entryEndMs, s.endMs) - maxOf(entryStartMs, s.startMs)
        if (overlap > bestOverlap) {
            bestOverlap = overlap
            bestOverlapIdx = s.index
        }
        val gap = kotlin.math.abs(s.startMs - entryStartMs)
        if (gap < nearestGap) {
            nearestGap = gap
            nearestIdx = s.index
        }
    }

    // Any real overlap wins outright; otherwise accept the nearest start only if it's within tolerance.
    return bestOverlapIdx ?: nearestIdx?.takeIf { nearestGap <= toleranceMs }
}

/** Default proximity window for the no-overlap fallback — 3 hours either side of the entry's start. */
const val DEFAULT_MATCH_TOLERANCE_MS = 3L * 60 * 60 * 1000
