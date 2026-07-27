package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HrSeriesTest {

    private fun series(vararg bpm: Int): List<HrPoint> =
        bpm.mapIndexed { i, b -> HrPoint(timeMs = i * 1000L, bpm = b) }

    @Test
    fun `avg and max of empty series are null`() {
        assertNull(emptyList<HrPoint>().avgBpm())
        assertNull(emptyList<HrPoint>().maxBpm())
    }

    @Test
    fun `avg and max compute over the series`() {
        val s = series(100, 120, 140)
        assertEquals(120, s.avgBpm())
        assertEquals(140, s.maxBpm())
    }

    @Test
    fun `downsample passes small series through untouched`() {
        val s = series(100, 110, 120)
        assertEquals(s, downsampleHr(s, maxPoints = 120))
    }

    @Test
    fun `downsample caps at maxPoints and keeps time order`() {
        val s = (0 until 1000).map { HrPoint(timeMs = it * 1000L, bpm = 100 + (it % 40)) }
        val out = downsampleHr(s, maxPoints = 120)
        assertEquals(120, out.size)
        assertTrue(out.zipWithNext().all { (a, b) -> a.timeMs <= b.timeMs })
        // Bucket means stay inside the source's bpm range.
        assertTrue(out.all { it.bpm in 100..139 })
    }

    @Test
    fun `downsample with nonpositive cap passes through`() {
        val s = series(100, 110)
        assertEquals(s, downsampleHr(s, maxPoints = 0))
    }

    @Test
    fun `watch workout duration is span floored to minutes, never negative`() {
        val w = WatchWorkout("id", startMs = 0, endMs = 32 * 60_000L + 30_000L, exerciseType = 0, title = null, distanceKm = null, kcal = null)
        assertEquals(32, w.durationMin)
        val bad = w.copy(endMs = -1)
        assertEquals(0, bad.durationMin)
    }
}
