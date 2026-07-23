package com.forge.app.domain.health

/** One heart-rate sample flattened out of Health Connect's series records (W5). */
data class HrPoint(val timeMs: Long, val bpm: Int)

/**
 * A watch-recorded Health Connect exercise session, summarised for Avex (W5): the HR-graph match on
 * a cardio detail, and the "recorded with your watch — import?" suggestions. Distance/calories are
 * null when that read isn't granted or the session simply has none.
 */
data class WatchWorkout(
    val recordId: String,
    val startMs: Long,
    val endMs: Long,
    /** Health Connect exercise-type int ([androidx.health.connect.client.records.ExerciseSessionRecord]). */
    val exerciseType: Int,
    val title: String?,
    val distanceKm: Double?,
    val kcal: Double?
) {
    val durationMin: Int get() = ((endMs - startMs) / 60_000L).toInt().coerceAtLeast(0)
}

/** Average bpm of a series, or null when empty. */
fun List<HrPoint>.avgBpm(): Int? = if (isEmpty()) null else (sumOf { it.bpm } / size)

/** Peak bpm of a series, or null when empty. */
fun List<HrPoint>.maxBpm(): Int? = maxOfOrNull { it.bpm }

/**
 * Bucket-average an HR series down to at most [maxPoints] (chart density control). Each bucket
 * averages its samples' bpm and keeps its mid time, so a 90-minute session renders as a smooth
 * line instead of thousands of points. A series already at/under the cap passes through untouched.
 */
fun downsampleHr(points: List<HrPoint>, maxPoints: Int = 120): List<HrPoint> {
    if (maxPoints <= 0 || points.size <= maxPoints) return points
    val bucketSize = points.size.toDouble() / maxPoints
    return (0 until maxPoints).map { i ->
        val from = (i * bucketSize).toInt()
        val to = (((i + 1) * bucketSize).toInt()).coerceAtMost(points.size).coerceAtLeast(from + 1)
        val bucket = points.subList(from, to)
        HrPoint(
            timeMs = bucket[bucket.size / 2].timeMs,
            bpm = bucket.sumOf { it.bpm } / bucket.size
        )
    }
}
