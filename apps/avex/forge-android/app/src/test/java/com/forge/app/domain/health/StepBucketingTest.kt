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
}
