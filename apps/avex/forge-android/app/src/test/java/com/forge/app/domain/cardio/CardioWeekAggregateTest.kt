package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId

class CardioWeekAggregateTest {

    private val zone = ZoneId.of("UTC")
    // Mon 2026-06-22 is the start of ISO week 26.
    private val weekStart = LocalDate.of(2026, 6, 22)
    private val weekStartMs = weekStart.atStartOfDay(zone).toInstant().toEpochMilli()

    private fun ms(date: LocalDate, hour: Int = 12): Long =
        date.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli()

    private fun entry(date: LocalDate, type: String = CardioType.RUN.code, dur: Int = 30, dist: Double? = null) =
        CardioEntry(date = ms(date), type = type, durationMin = dur, distanceKm = dist)

    @Test fun `empty week is all zeros`() {
        val agg = cardioWeekAggregate(emptyList(), weekStartMs, zone)
        assertEquals(0, agg.days)
        assertEquals(0, agg.sessions)
        assertEquals(0, agg.minutes)
        assertEquals(listOf(0, 0, 0, 0, 0, 0, 0), agg.perDayMinutes)
    }

    @Test fun `per-day minutes land in the right Mon-Sun slots`() {
        val entries = listOf(
            entry(weekStart, dur = 40),                       // Monday → index 0
            entry(weekStart.plusDays(2), dur = 25),           // Wednesday → index 2
            entry(weekStart.with(DayOfWeek.SUNDAY), dur = 15)  // Sunday → index 6
        )
        val agg = cardioWeekAggregate(entries, weekStartMs, zone)
        assertEquals(listOf(40, 0, 25, 0, 0, 0, 15), agg.perDayMinutes)
        assertEquals(3, agg.sessions)
        assertEquals(3, agg.days)
        assertEquals(80, agg.minutes)
    }

    @Test fun `two sessions same day count one day but two sessions`() {
        val entries = listOf(entry(weekStart, dur = 30), entry(weekStart, dur = 20))
        val agg = cardioWeekAggregate(entries, weekStartMs, zone)
        assertEquals(1, agg.days)
        assertEquals(2, agg.sessions)
        assertEquals(50, agg.minutes)
        assertEquals(50, agg.perDayMinutes[0])
    }

    @Test fun `entries outside the week are ignored`() {
        val entries = listOf(
            entry(weekStart.minusDays(1)),       // previous Sunday
            entry(weekStart.plusDays(7)),        // next Monday
            entry(weekStart.plusDays(1), dur = 33)
        )
        val agg = cardioWeekAggregate(entries, weekStartMs, zone)
        assertEquals(1, agg.sessions)
        assertEquals(33, agg.minutes)
    }

    @Test fun `rest entries excluded - distance and type breakdown sum correctly`() {
        val entries = listOf(
            entry(weekStart, type = CardioType.RUN.code, dur = 30, dist = 5.0),
            entry(weekStart.plusDays(1), type = CardioType.RUN.code, dur = 20, dist = 3.0),
            entry(weekStart.plusDays(2), type = CardioType.WALK.code, dur = 40, dist = 4.0),
            entry(weekStart.plusDays(3), type = CardioType.REST.code, dur = 0)
        )
        val agg = cardioWeekAggregate(entries, weekStartMs, zone)
        assertEquals(3, agg.sessions)
        assertEquals(12.0, agg.distanceKm, 1e-9)
        // Run (50 min) should sort ahead of Walk (40 min).
        assertEquals(CardioType.RUN, agg.minutesByType[0].first)
        assertEquals(50, agg.minutesByType[0].second)
        assertEquals(CardioType.WALK, agg.minutesByType[1].first)
        assertEquals(40, agg.minutesByType[1].second)
    }
}
