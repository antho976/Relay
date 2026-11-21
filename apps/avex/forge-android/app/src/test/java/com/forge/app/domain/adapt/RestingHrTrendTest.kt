package com.forge.app.domain.adapt

import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RestingHrTrendTest {

    private val t = AdaptThresholds()
    private val dayMs = 24L * 60 * 60 * 1000
    private val now = 100_000_000_000L
    private val windowStart = now - t.deloadWindowDays * dayMs
    private val priorStart = windowStart - t.deloadPriorBaselineDays * dayMs

    /** [count] samples evenly placed inside [from, to), all at [bpm]. */
    private fun samples(from: Long, to: Long, bpm: Int, count: Int): List<RestingHrSample> {
        val step = (to - from) / (count + 1)
        return (1..count).map { RestingHrSample(timeMs = from + step * it, bpm = bpm) }
    }

    @Test fun `spike when window HR is above baseline`() {
        val baseline = samples(priorStart, windowStart, bpm = 55, count = t.deloadMinRestingHrSamples)
        val window = samples(windowStart, now, bpm = 55 + t.deloadRestingHrDeltaBpm + 2, count = t.deloadMinRestingHrSamples)
        val r = RestingHrTrend.spike(baseline + window, now, t)
        assertNotNull(r)
        assertTrue(r!!.deltaBpm >= t.deloadRestingHrDeltaBpm)
        assertTrue(r.windowBpm > r.baselineBpm)
    }

    @Test fun `no spike when HR matches baseline`() {
        val baseline = samples(priorStart, windowStart, bpm = 55, count = t.deloadMinRestingHrSamples)
        val window = samples(windowStart, now, bpm = 55, count = t.deloadMinRestingHrSamples)
        assertNull(RestingHrTrend.spike(baseline + window, now, t))
    }

    @Test fun `no spike when a delta below the threshold`() {
        val baseline = samples(priorStart, windowStart, bpm = 55, count = t.deloadMinRestingHrSamples)
        val window = samples(windowStart, now, bpm = 55 + t.deloadRestingHrDeltaBpm - 1, count = t.deloadMinRestingHrSamples)
        assertNull(RestingHrTrend.spike(baseline + window, now, t))
    }

    @Test fun `no spike below the sample gate`() {
        val baseline = samples(priorStart, windowStart, bpm = 55, count = t.deloadMinRestingHrSamples - 1)
        val window = samples(windowStart, now, bpm = 70, count = t.deloadMinRestingHrSamples - 1)
        assertNull(RestingHrTrend.spike(baseline + window, now, t))
    }

    @Test fun `no spike with no samples`() {
        assertNull(RestingHrTrend.spike(emptyList(), now, t))
    }
}
