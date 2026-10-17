package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale

/** The in-progress notice says when the open workout began, relative to today. */
class SessionStartedLineTest {

    private val zone = ZoneId.of("America/Toronto")
    private val locale = Locale.US
    private val today = LocalDate.of(2026, 10, 17) // a Saturday
    private val dayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()

    private fun at(date: LocalDate, h: Int, m: Int) = date.atTime(h, m).atZone(zone).toInstant().toEpochMilli()
    private fun line(startedAtMs: Long, use24h: Boolean = false) =
        NotificationFeed.sessionStartedLine(startedAtMs, dayStartMs, use24h, zone, locale)

    @Test
    fun `a session from today gives just the time`() {
        assertEquals("Started at 8:41 AM.", line(at(today, 8, 41)))
    }

    @Test
    fun `last night's session says yesterday`() {
        assertEquals("Started yesterday at 11:20 PM.", line(at(today.minusDays(1), 23, 20)))
    }

    @Test
    fun `an older session names its weekday`() {
        assertEquals("Started Thu at 7:05 AM.", line(at(today.minusDays(2), 7, 5)))
    }

    @Test
    fun `the time follows the app's clock setting, not the locale`() {
        assertEquals("Started yesterday at 23:20.", line(at(today.minusDays(1), 23, 20), use24h = true))
    }
}
