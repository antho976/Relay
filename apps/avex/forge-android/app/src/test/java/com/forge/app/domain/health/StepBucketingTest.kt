package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneId

class StepBucketingTest {

    private val utc = ZoneId.of("UTC")
    private fun atHour(h: Int, minute: Int = 0) = (h * 3_600_000L) + (minute * 60_000L)

    @Test
    fun emptyInputYieldsNoBuckets() {
        assertTrue(bucketStepsByHour(emptyList(), utc).isEmpty())
    }

    @Test
    fun sumsMultipleSamplesInTheSameHour() {
        val out = bucketStepsByHour(
            listOf(
                StepSample(startMs = atHour(6), count = 100),
                StepSample(startMs = atHour(6, 1), count = 50),
                StepSample(startMs = atHour(6, 59), count = 25)
            ),
            utc
        )
        assertEquals(1, out.size)
        assertEquals(6, out[0].hour)
        assertEquals(175, out[0].steps)
    }

    @Test
    fun returnsHoursSortedAscendingWithGapsOmitted() {
        val out = bucketStepsByHour(
            listOf(
                StepSample(startMs = atHour(9), count = 300),
                StepSample(startMs = atHour(6), count = 100)
            ),
            utc
        )
        assertEquals(listOf(6, 9), out.map { it.hour })
        assertEquals(listOf(100, 300), out.map { it.steps })
    }

    @Test
    fun dropsZeroAndNegativeCounts() {
        val out = bucketStepsByHour(
            listOf(
                StepSample(startMs = atHour(7), count = 0),
                StepSample(startMs = atHour(8), count = -50),
                StepSample(startMs = atHour(8), count = 80)
            ),
            utc
        )
        assertEquals(1, out.size)
        assertEquals(8, out[0].hour)
        assertEquals(80, out[0].steps)
    }

    @Test
    fun attributesToTheLocalHourOfTheZone() {
        // Epoch 0 is 1970-01-01T00:00Z → hour 0 in UTC, but 19:00 the previous day in New York (UTC-5).
        assertEquals(0, bucketStepsByHour(listOf(StepSample(0L, 10)), utc).single().hour)
        val ny = bucketStepsByHour(listOf(StepSample(0L, 10)), ZoneId.of("America/New_York")).single()
        assertEquals(19, ny.hour)
    }

    // ── Aggregate buckets (M-01): the provider's de-duplicated totals, sliced by hour / local day ──

    @Test
    fun hourlyBucketsMapToTheirLocalHourAndDropEmptyOnes() {
        // Hour-aligned buckets from an aggregateGroupByDuration over a local day: an empty bucket
        // (null) is absent from the graph, exactly as an hour with no raw samples was.
        val out = hourlyStepsFromBuckets(
            listOf(
                StepBucket(startMs = atHour(6), steps = 120L),
                StepBucket(startMs = atHour(7), steps = null),
                StepBucket(startMs = atHour(8), steps = 0L),
                StepBucket(startMs = atHour(9), steps = 300L)
            ),
            utc
        )
        assertEquals(listOf(6, 9), out.map { it.hour })
        assertEquals(listOf(120, 300), out.map { it.steps })
    }

    @Test
    fun hourlyBucketsUseTheZoneNotUtc() {
        val ny = hourlyStepsFromBuckets(listOf(StepBucket(0L, 10L)), ZoneId.of("America/New_York")).single()
        assertEquals(19, ny.hour)
    }

    @Test
    fun dailyBucketsKeyOnTheLocalDayOldestFirst() {
        val day = 24 * 3_600_000L
        val out = dailyStepsFromBuckets(
            listOf(
                StepBucket(startMs = 2 * day, steps = 5_000L),
                StepBucket(startMs = 0L, steps = 8_000L),
                StepBucket(startMs = day, steps = null)
            ),
            utc
        )
        // Day 1 had no data: absent, not zero, so a typical-day median never counts a silent day.
        assertEquals(listOf(0L, 2 * day), out.map { it.dayStartMs })
        assertEquals(listOf(8_000, 5_000), out.map { it.steps })
    }

    @Test
    fun dailyBucketsSumOntoOneLocalDayWhenNotDayAligned() {
        // Two buckets inside the same local day (a slicer that started mid-day) roll up together, and
        // a negative total from a corrupt provider can't subtract from the day.
        val out = dailyStepsFromBuckets(
            listOf(
                StepBucket(startMs = atHour(3), steps = 1_000L),
                StepBucket(startMs = atHour(15), steps = 2_500L),
                StepBucket(startMs = atHour(20), steps = -50L)
            ),
            utc
        )
        assertEquals(1, out.size)
        assertEquals(0L, out[0].dayStartMs)
        assertEquals(3_500, out[0].steps)
    }

    @Test
    fun dailyBucketsAttributeToTheLocalCalendarDay() {
        // 01:00Z on 1970-01-02 is still 1970-01-01 (20:00) in New York: the day start moves with the zone.
        val ny = ZoneId.of("America/New_York")
        val out = dailyStepsFromBuckets(listOf(StepBucket(startMs = 25 * 3_600_000L, steps = 42L)), ny).single()
        val expectedStart = java.time.LocalDate.of(1970, 1, 1).atStartOfDay(ny).toInstant().toEpochMilli()
        assertEquals(expectedStart, out.dayStartMs)
        assertEquals(42, out.steps)
    }

    @Test
    fun emptyBucketsYieldNothing() {
        assertTrue(hourlyStepsFromBuckets(emptyList(), utc).isEmpty())
        assertTrue(dailyStepsFromBuckets(emptyList(), utc).isEmpty())
        assertTrue(dailyStepsFromBuckets(listOf(StepBucket(0L, null)), utc).isEmpty())
    }
}
