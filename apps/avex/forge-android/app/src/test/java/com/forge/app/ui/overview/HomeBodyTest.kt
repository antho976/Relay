package com.forge.app.ui.overview

import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.HrvSample
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneOffset

class HomeBodyTest {

    private val day = 24L * 60 * 60 * 1000
    private val today = 100 * day
    private val utc = ZoneOffset.UTC

    @Test
    fun lastNightComparesAgainstTheMedianOfEarlierNights() {
        val snap = HealthSnap(
            sleepNights = listOf(420, 430, 440, 450).mapIndexed { i, min ->
                SleepNight(endedAtMs = today - (i + 1) * day + 7 * 3_600_000, durationMin = min)
            } + SleepNight(endedAtMs = today + 7 * 3_600_000, durationMin = 395)
        )
        val sleep = homeBodyReadings(snap, today, utc).sleep!!
        assertEquals("6.6", sleep.figure)
        assertEquals("h", sleep.unit)
        assertEquals("↓ 0.8h vs usual", sleep.change)
        assertTrue(sleep.changeMoved)
        // Oldest night first, ending on last night.
        assertEquals(5, sleep.spark.size)
        assertEquals(395 / 60f, sleep.spark.last(), 0.001f)
    }

    @Test
    fun aNightSplitInTwoIsOneNight() {
        val snap = HealthSnap(
            sleepNights = listOf(
                SleepNight(endedAtMs = today + 3 * 3_600_000, durationMin = 200),
                SleepNight(endedAtMs = today + 7 * 3_600_000, durationMin = 220),
            )
        )
        assertEquals("7.0", homeBodyReadings(snap, today, utc).sleep!!.figure)
    }

    @Test
    fun noComparisonWithFewerThanThreeEarlierDays() {
        val snap = HealthSnap(
            restingHr = listOf(
                RestingHrSample(today - day, 52),
                RestingHrSample(today - 2 * day, 53),
                RestingHrSample(today + 1_000, 58)
            )
        )
        val hr = homeBodyReadings(snap, today, utc).restingHr!!
        assertEquals("58", hr.figure)
        assertEquals(" bpm", hr.unit)
        assertNull(hr.change)
    }

    @Test
    fun smallSwingsReadAsNormalAndDoNotLightTheLine() {
        val snap = HealthSnap(
            restingHr = (1..5).map { RestingHrSample(today - it * day, 52) } + RestingHrSample(today + 1, 54),
            hrv = (1..5).map { HrvSample(today - it * day, 60.0) } + HrvSample(today + 1, 57.0)
        )
        val readings = homeBodyReadings(snap, today, utc)
        assertEquals("Normal for you", readings.restingHr!!.change)
        assertFalse(readings.restingHr!!.changeMoved)
        assertEquals("Normal for you", readings.hrv!!.change)
    }

    @Test
    fun staleReadingsAreNotShownAsToday() {
        val snap = HealthSnap(
            sleepNights = listOf(SleepNight(endedAtMs = today - 3 * day, durationMin = 400)),
            restingHr = listOf(RestingHrSample(today - 3 * day, 52))
        )
        val readings = homeBodyReadings(snap, today, utc)
        assertNull(readings.sleep)
        assertNull(readings.restingHr)
        assertTrue(readings.isEmpty)
    }

    @Test
    fun stepsShowAnHonestZeroOnceConnected() {
        val snap = HealthSnap(
            dailySteps = (1..4).map { DailySteps(today - it * day, 8_000 + it * 100) } + DailySteps(today, 0)
        )
        val steps = homeBodyReadings(snap, today, utc).steps!!
        assertEquals("0", steps.figure)
        assertEquals("Typical 8.3k", steps.change)
    }

    @Test
    fun minutesFormat() {
        assertEquals("45m", formatMinutes(45))
        assertEquals("7h", formatMinutes(420))
        assertEquals("7h 14m", formatMinutes(434))
    }
}
