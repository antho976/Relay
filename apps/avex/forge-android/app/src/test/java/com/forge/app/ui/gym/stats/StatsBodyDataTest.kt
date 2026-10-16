package com.forge.app.ui.gym.stats

import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class StatsBodyDataTest {

    private val day = 24L * 60 * 60 * 1000
    private val today = 400 * day
    private val dayStart: (Long) -> Long = { it - it % day }

    @Test
    fun seriesEndsTodayWithGapsForMissingDays() {
        val snap = HealthSnap(
            restingHr = listOf(
                RestingHrSample(today - 3 * day + 1_000, 50),
                RestingHrSample(today - 2 * day + 1_000, 52),
                RestingHrSample(today - day + 1_000, 54),
                RestingHrSample(today + 1_000, 58)
            )
        )
        val hr = statsBodySeries(snap, today, dayStart, stepsGranted = false).restingHr!!
        assertEquals(BODY_DAYS, hr.days.size)
        assertEquals(58.0, hr.latest!!, 0.0)
        assertNull(hr.days[BODY_DAYS - 5])
        assertEquals(52.0, hr.usual!!, 0.0)
    }

    @Test
    fun aNightBelongsToTheMorningItEnded() {
        val snap = HealthSnap(sleepNights = listOf(SleepNight(endedAtMs = today + 7 * 3_600_000, durationMin = 400)))
        val sleep = statsBodySeries(snap, today, dayStart, stepsGranted = false).sleepMin!!
        assertEquals(400.0, sleep.latest!!, 0.0)
    }

    @Test
    fun readingsOutsideTheWindowAreDropped() {
        val snap = HealthSnap(restingHr = listOf(RestingHrSample(today - 30 * day, 50)))
        val hr = statsBodySeries(snap, today, dayStart, stepsGranted = false).restingHr!!
        assertEquals(false, hr.hasData)
    }

    @Test
    fun stepsNeedTheirOwnGrant() {
        val snap = HealthSnap(dailySteps = listOf(DailySteps(today, 5_000)))
        assertNull(statsBodySeries(snap, today, dayStart, stepsGranted = false).steps)
        assertEquals(5_000.0, statsBodySeries(snap, today, dayStart, stepsGranted = true).steps!!.latest!!, 0.0)
    }
}
