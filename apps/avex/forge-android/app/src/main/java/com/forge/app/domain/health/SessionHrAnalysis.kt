package com.forge.app.domain.health

/**
 * Post-session heart-rate analysis (W3) — pure, so the graph's inputs and the HRR math are unit-
 * testable without Room. Consumes the watch's [HrPoint] trace, the session's set timestamps and
 * its rest windows; produces everything the detail page's HEART RATE section renders.
 */
data class SessionHrView(
    /** Downsampled trace, time-ordered — the chart's line. */
    val points: List<HrPoint>,
    val avgBpm: Int,
    val maxBpm: Int,
    /** completedAt of every logged set — the chart's on-line markers. */
    val setMarkersMs: List<Long>,
    /** Exercise start boundaries (first set's span start per exercise) — the chart's hairlines. */
    val exerciseBoundariesMs: List<Long>,
    /** Per-exercise average bpm, in session order — "what was my HR during squats". */
    val perExercise: List<ExerciseHr>,
    /** Mean HR drop over the first 60 s of rest, across rests long enough to measure; null when
     *  no rest window had usable samples. Positive = bpm recovered. */
    val avgHrr60: Int?
) {
    data class ExerciseHr(val name: String, val avgBpm: Int)
}

/** One set as the analysis needs it: when it was logged and which exercise it belonged to. */
data class HrSetRef(val completedAtMs: Long, val exerciseName: String)

/** One completed rest: when it ended (next set / expiry) and how long it really lasted. */
data class HrRestRef(val endedAtMs: Long, val realizedSeconds: Int)

fun buildSessionHrView(
    samples: List<HrPoint>,
    sets: List<HrSetRef>,
    rests: List<HrRestRef>,
    maxChartPoints: Int = 160
): SessionHrView? {
    if (samples.size < MIN_SAMPLES) return null
    val ordered = samples.sortedBy { it.timeMs }
    val bySets = sets.sortedBy { it.completedAtMs }

    // Per-exercise spans: an exercise owns the time from just after the previous exercise's last
    // set to its own last set (the first exercise starts at the session's first sample).
    val perExercise = mutableListOf<SessionHrView.ExerciseHr>()
    val boundaries = mutableListOf<Long>()
    var spanStart = ordered.first().timeMs
    var i = 0
    while (i < bySets.size) {
        val name = bySets[i].exerciseName
        var j = i
        while (j + 1 < bySets.size && bySets[j + 1].exerciseName == name) j++
        val spanEnd = bySets[j].completedAtMs
        val inSpan = ordered.filter { it.timeMs in spanStart..spanEnd }
        if (inSpan.isNotEmpty()) {
            perExercise += SessionHrView.ExerciseHr(name = name, avgBpm = inSpan.map { it.bpm }.average().toInt())
            if (i > 0) boundaries += spanStart
        }
        spanStart = spanEnd + 1
        i = j + 1
    }

    // HRR60: for each rest ≥60 s, the drop from the rest's start bpm to bpm one minute in.
    val drops = rests.filter { it.realizedSeconds >= HRR_WINDOW_SEC }.mapNotNull { rest ->
        val restStartMs = rest.endedAtMs - rest.realizedSeconds * 1000L
        val startBpm = nearestBpm(ordered, restStartMs) ?: return@mapNotNull null
        val laterBpm = nearestBpm(ordered, restStartMs + HRR_WINDOW_SEC * 1000L) ?: return@mapNotNull null
        startBpm - laterBpm
    }

    return SessionHrView(
        points = downsampleHr(ordered, maxChartPoints),
        avgBpm = ordered.map { it.bpm }.average().toInt(),
        maxBpm = ordered.maxOf { it.bpm },
        setMarkersMs = bySets.map { it.completedAtMs },
        exerciseBoundariesMs = boundaries,
        perExercise = perExercise,
        avgHrr60 = drops.takeIf { it.isNotEmpty() }?.average()?.toInt()
    )
}

/** The sample nearest [atMs], or null when none lies within the tolerance (a BT gap ≠ a reading). */
private fun nearestBpm(ordered: List<HrPoint>, atMs: Long): Int? =
    ordered.minByOrNull { kotlin.math.abs(it.timeMs - atMs) }
        ?.takeIf { kotlin.math.abs(it.timeMs - atMs) <= NEAREST_TOLERANCE_MS }
        ?.bpm

private const val MIN_SAMPLES = 10
private const val HRR_WINDOW_SEC = 60
private const val NEAREST_TOLERANCE_MS = 20_000L
