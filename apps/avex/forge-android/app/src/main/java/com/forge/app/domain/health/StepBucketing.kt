package com.forge.app.domain.health

import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.cardio.HourlySteps
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.TreeMap

/** One raw step total Health Connect recorded over a short interval — its start instant + the count. */
data class StepSample(val startMs: Long, val count: Long)

/**
 * One Health Connect AGGREGATE bucket (M-01): the provider's own de-duplicated step total for the
 * window starting at [startMs]. [steps] is null when the window had no data at all, which is how
 * `StepsRecord.COUNT_TOTAL` reports an empty bucket.
 *
 * Aggregation is what makes the count honest. Summing raw rows counted a phone and a watch that
 * each logged the same 1,000 steps as 2,000; Health Connect's aggregate applies its source
 * priority so overlapping providers count once. Slicing the aggregate by hour or by local day
 * also lets the provider split a row that spans a boundary, where start-hour attribution put
 * all of 10:30 to 11:30 in the 10 o'clock bucket and a 23:55 to 00:05 row on the earlier day.
 */
data class StepBucket(val startMs: Long, val steps: Long?)

/**
 * Per-hour totals for the cardio graph from hour-sliced aggregate [buckets], in [zone]'s local
 * time. Each bucket is attributed to the local hour it STARTS in, which for hour-aligned buckets
 * is simply the hour it covers; empty (null) buckets are dropped, so only hours with steps are
 * returned, exactly as [bucketStepsByHour] did for raw samples.
 */
fun hourlyStepsFromBuckets(buckets: List<StepBucket>, zone: ZoneId): List<HourlySteps> =
    bucketStepsByHour(buckets.map { StepSample(startMs = it.startMs, count = it.steps ?: 0L) }, zone)

/**
 * Per-day totals from day-sliced aggregate [buckets], keyed on the local calendar day in [zone]
 * and sorted oldest first. A bucket with no data (null) contributes nothing, so a day the
 * provider had nothing for is absent rather than reported as zero, matching the raw-row read's
 * "no records, no entry" shape that the Home movement line and the coach already handle. Two
 * buckets landing on one day (a slicer that wasn't day-aligned) are summed.
 */
fun dailyStepsFromBuckets(buckets: List<StepBucket>, zone: ZoneId): List<DailySteps> {
    if (buckets.isEmpty()) return emptyList()
    val perDay = TreeMap<LocalDate, Long>()
    for (b in buckets) {
        val steps = b.steps ?: continue
        val day = Instant.ofEpochMilli(b.startMs).atZone(zone).toLocalDate()
        perDay[day] = (perDay[day] ?: 0L) + steps.coerceAtLeast(0L)
    }
    return perDay.map { (day, steps) ->
        DailySteps(
            dayStartMs = day.atStartOfDay(zone).toInstant().toEpochMilli(),
            steps = steps.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        )
    }
}

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
