package com.forge.app.ui.overview

import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.HrvSample
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HomeBodyTest {

    private val day = 24L * 60 * 60 * 1000
    private val today = 100 * day

    @Test
    fun lastNightComparesAgainstTheMedianOfEarlierNights() {
        val snap = HealthSnap(
            sleepNights = listOf(420, 430, 440, 450).mapIndexed { i, min ->
                SleepNight(endedAtMs = today - (i + 1) * day + 7 * 3_600_000, durationMin = min)
            } + SleepNight(endedAtMs = today + 7 * 3_600_000, durationMin = 395)
        )
        val sleep = homeBodyReadings(snap, today).sleep!!
        assertEquals("6h 35m", sleep.value)
        assertEquals("45m under usual", sleep.compare)
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
        val hr = homeBodyReadings(snap, today).restingHr!!
        assertEquals("58 bpm", hr.value)
        assertNull(hr.compare)
    }

    @Test
    fun smallSwingsReadAsNormal() {
        val snap = HealthSnap(
            restingHr = (1..5).map { RestingHrSample(today - it * day, 52) } + RestingHrSample(today + 1, 54),
            hrv = (1..5).map { HrvSample(today - it * day, 60.0) } + HrvSample(today + 1, 57.0)
        )
        val readings = homeBodyReadings(snap, today)
        assertEquals("Normal for you", readings.restingHr!!.compare)
        assertEquals("Normal for you", readings.hrv!!.compare)
    }

    @Test
    fun staleReadingsAreNotShownAsToday() {
        val snap = HealthSnap(
            sleepNights = listOf(SleepNight(endedAtMs = today - 3 * day, durationMin = 400)),
            restingHr = listOf(RestingHrSample(today - 3 * day, 52))
        )
        val readings = homeBodyReadings(snap, today)
        assertNull(readings.sleep)
        assertNull(readings.restingHr)
        assertTrue(readings.isEmpty)
    }

    @Test
    fun stepsShowAnHonestZeroOnceConnected() {
        val snap = HealthSnap(
            dailySteps = (1..4).map { DailySteps(today - it * day, 8_000 + it * 100) } + DailySteps(today, 0)
        )
        val steps = homeBodyReadings(snap, today).steps!!
        assertEquals("0", steps.value)
        assertEquals("Typical 8.3k", steps.compare)
    }

    @Test
    fun minutesFormat() {
        assertEquals("45m", formatMinutes(45))
        assertEquals("7h", formatMinutes(420))
        assertEquals("7h 14m", formatMinutes(434))
    }
}
