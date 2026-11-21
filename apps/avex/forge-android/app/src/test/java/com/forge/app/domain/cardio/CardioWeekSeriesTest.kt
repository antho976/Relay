package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneId

class CardioWeekSeriesTest {

    private val zone = ZoneId.of("UTC")
    // Mon 2026-06-22 starts the "current" week in these tests; now is that Wednesday.
    private val currentMonday = LocalDate.of(2026, 6, 22)
    private val nowMs = currentMonday.plusDays(2).atTime(9, 0).atZone(zone).toInstant().toEpochMilli()

    private fun ms(date: LocalDate, hour: Int = 12): Long =
        date.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli()

    private fun entry(date: LocalDate, type: String = CardioType.RUN.code, dur: Int = 30, dist: Double? = null) =
        CardioEntry(date = ms(date), type = type, durationMin = dur, distanceKm = dist)

    @Test fun `no history still returns the full window at zero`() {
        val series = cardioWeekSeries(emptyList(), nowMs, weeks = 4, zone = zone)
        assertEquals(4, series.size)
        assertTrue(series.all { it.isEmpty })
        assertTrue(series.all { it.perDayMinutes == listOf(0, 0, 0, 0, 0, 0, 0) })
    }

    @Test fun `series is oldest to newest and ends with the current week`() {
        val series = cardioWeekSeries(emptyList(), nowMs, weeks = 3, zone = zone)
        val expected = listOf(
            currentMonday.minusWeeks(2), currentMonday.minusWeeks(1), currentMonday
        ).map { it.atStartOfDay(zone).toInstant().toEpochMilli() }
        assertEquals(expected, series.map { it.weekStartMs })
    }

    @Test fun `an untrained week stays in the series at zero`() {
        // Trained two weeks ago and this week; the week between must survive as a zero point, or the
        // load chart would read as an unbroken run of training.
        val entries = listOf(
            entry(currentMonday.minusWeeks(2), dur = 40),
            entry(currentMonday, dur = 25)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 3, zone = zone)
        assertEquals(listOf(40, 0, 25), series.map { it.minutes })
        assertTrue(series[1].isEmpty)
    }

    @Test fun `entries older than the window are ignored`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(9), dur = 90),
            entry(currentMonday, dur = 20)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 4, zone = zone)
        assertEquals(20, series.sumOf { it.minutes })
    }

    @Test fun `rest entries never count as minutes moved`() {
        val entries = listOf(
            entry(currentMonday, type = CardioType.REST.code, dur = 0),
            entry(currentMonday.plusDays(1), dur = 35)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 1, zone = zone)
        assertEquals(35, series.last().minutes)
        assertEquals(1, series.last().sessions)
        assertEquals(1, series.last().days)
    }

    @Test fun `two sessions on one day count once toward days`() {
        val entries = listOf(
            entry(currentMonday, dur = 20, dist = 3.0),
            entry(currentMonday, dur = 30, dist = 5.0)
        )
        val week = cardioWeekSeries(entries, nowMs, weeks = 1, zone = zone).last()
        assertEquals(2, week.sessions)
        assertEquals(1, week.days)
        assertEquals(50, week.minutes)
        assertEquals(8.0, week.distanceKm, 0.001)
        assertEquals(listOf(50, 0, 0, 0, 0, 0, 0), week.perDayMinutes)
    }

    @Test fun `per-day minutes land in the right Mon-Sun slots`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(1), dur = 15),                // Monday
            entry(currentMonday.minusWeeks(1).plusDays(3), dur = 45),    // Thursday
            entry(currentMonday.minusWeeks(1).plusDays(6), dur = 60)     // Sunday
        )
        val week = cardioWeekSeries(entries, nowMs, weeks = 2, zone = zone).first()
        assertEquals(listOf(15, 0, 0, 45, 0, 0, 60), week.perDayMinutes)
    }

    @Test fun `zero or negative window returns nothing`() {
        assertEquals(emptyList<CardioWeekPoint>(), cardioWeekSeries(emptyList(), nowMs, weeks = 0, zone = zone))
    }

    @Test fun `weeks on target excludes the week still in progress`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(2), dur = 200),
            entry(currentMonday.minusWeeks(1), dur = 100),
            entry(currentMonday, dur = 200)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 3, zone = zone)
        // The 200-minute current week must NOT count — only the completed 200 behind it does.
        assertEquals(1, cardioWeeksOnTarget(series, targetMin = 150))
    }

    @Test fun `weeks on target is zero without a target`() {
        val series = cardioWeekSeries(emptyList(), nowMs, weeks = 4, zone = zone)
        assertEquals(0, cardioWeeksOnTarget(series, targetMin = 0))
    }

    @Test fun `load delta needs three completed weeks`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(1), dur = 100),
            entry(currentMonday, dur = 200)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 2, zone = zone)
        assertNull(cardioLoadDeltaPct(series))
    }

    @Test fun `load delta reads this week against the median of the weeks behind it`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(3), dur = 100),
            entry(currentMonday.minusWeeks(2), dur = 100),
            entry(currentMonday.minusWeeks(1), dur = 100),
            entry(currentMonday, dur = 150)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 4, zone = zone)
        assertEquals(50, cardioLoadDeltaPct(series))
    }

    @Test fun `load delta is negative when this week is behind the baseline`() {
        val entries = listOf(
            entry(currentMonday.minusWeeks(3), dur = 200),
            entry(currentMonday.minusWeeks(2), dur = 200),
            entry(currentMonday.minusWeeks(1), dur = 200),
            entry(currentMonday, dur = 50)
        )
        val series = cardioWeekSeries(entries, nowMs, weeks = 4, zone = zone)
        assertEquals(-75, cardioLoadDeltaPct(series))
    }

    @Test fun `load delta is null when nothing was trained behind this week`() {
        val entries = listOf(entry(currentMonday, dur = 60))
        val series = cardioWeekSeries(entries, nowMs, weeks = 5, zone = zone)
        assertNull(cardioLoadDeltaPct(series))
    }
}
