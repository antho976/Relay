package com.forge.app.ui.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * P-13: the lifetime-volume curve is one point per finished session with no cap, and every frame of
 * its 900 ms reveal rebuilt both paths from all of them. The reduction has to be cheap on the
 * common case and honest on the long one — a mean would flatten exactly the spikes a lifetime curve
 * exists to show.
 */
class SparklineSeriesTest {

    @Test
    fun aSeriesInsideTheCapIsReturnedUntouched() {
        val values = List(100) { it.toDouble() }
        assertSame("no allocation for the ordinary case", values, sparklineSeries(values, maxPoints = 512))
    }

    @Test
    fun aLongSeriesIsReducedToTheCap() {
        val values = List(50_000) { it.toDouble() }
        val out = sparklineSeries(values, maxPoints = 512)
        assertEquals(512, out.size)
    }

    @Test
    fun theExtremesSurviveTheReduction() {
        // One spike in the middle of a flat run. Averaging would erase it; bucketing by extremes
        // must not.
        val values = MutableList(10_000) { 1.0 }
        values[5_000] = 999.0
        values[7_000] = -999.0

        val out = sparklineSeries(values, maxPoints = 512)

        assertEquals("the peak", 999.0, out.max(), 0.0001)
        assertEquals("and the trough", -999.0, out.min(), 0.0001)
    }

    @Test
    fun theCurveStartsAndEndsWhereTheDataDoes() {
        // The end dot sits on the figure printed beside the chart, so the last point must be exact.
        val values = List(20_000) { it.toDouble() }.toMutableList()
        values[0] = -7.0
        values[values.lastIndex] = 12_345.0

        val out = sparklineSeries(values, maxPoints = 512)

        assertEquals(-7.0, out.first(), 0.0001)
        assertEquals(12_345.0, out.last(), 0.0001)
    }

    @Test
    fun aRisingSeriesStaysMonotonicSoTheShapeIsUnchanged() {
        val values = List(30_000) { it.toDouble() }
        val out = sparklineSeries(values, maxPoints = 512)
        assertTrue("a cumulative curve must not wobble", out.zipWithNext().all { (a, b) -> b >= a })
    }

    @Test
    fun anAwkwardCapOrASeriesTooShortToBucketIsLeftAlone() {
        val values = List(1_000) { it.toDouble() }
        assertSame(values, sparklineSeries(values, maxPoints = 2))
        assertSame(values, sparklineSeries(values, maxPoints = 3))
    }

    @Test
    fun everyBucketContributesAndNothingIsDroppedOffTheEnd() {
        // A series just over the cap: the reduction still covers it end to end rather than
        // stopping short, which would truncate the curve.
        val values = List(513) { it.toDouble() }
        val out = sparklineSeries(values, maxPoints = 512)
        assertEquals(512, out.size)
        assertEquals(0.0, out.first(), 0.0001)
        assertEquals(512.0, out.last(), 0.0001)
    }
}
