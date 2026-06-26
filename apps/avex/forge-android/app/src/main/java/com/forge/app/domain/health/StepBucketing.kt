package com.forge.app.domain.health

import com.forge.app.domain.cardio.HourlySteps
import java.time.Instant
import java.time.ZoneId

/** One raw step total Health Connect recorded over a short interval — its start instant + the count. */
data class StepSample(val startMs: Long, val count: Long)

/**
 * Bucket raw Health Connect step samples into per-hour totals in [zone]'s local time.
 *
 * Each sample is attributed to the wall-clock hour of its START. Samsung Health / a Galaxy Watch
 * write steps in short, sub-hour intervals, so start-hour bucketing reproduces the day's shape
 * without the complexity of splitting one record across the hours it spans. Counts are summed per
 * hour; only hours that actually have steps are returned (the cardio graph fills the gaps with zero).
 *
 * Pure and Android-free (no Compose, no HealthConnectClient) so it's unit-testable and the same
 * output flows straight into [com.forge.app.domain.cardio.CardioWearableDay].
 */
fun bucketStepsByHour(samples: List<StepSample>, zone: ZoneId): List<HourlySteps> {
    if (samples.isEmpty()) return emptyList()
    val perHour = HashMap<Int, Long>()
    for (s in samples) {
        if (s.count <= 0L) continue
        val hour = Instant.ofEpochMilli(s.startMs).atZone(zone).hour
        perHour[hour] = (perHour[hour] ?: 0L) + s.count
    }
    return perHour.entries
        .sortedBy { it.key }
        // A single hour can't realistically overflow Int, but coerce so a corrupt multi-day count can't.
        .map { HourlySteps(hour = it.key, steps = it.value.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()) }
}
