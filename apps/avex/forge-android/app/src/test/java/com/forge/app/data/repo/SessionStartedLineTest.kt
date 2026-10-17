package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale

/** The in-progress notice says when the open workout began, relative to today. */
class SessionStartedLineTest {

    private val zone = ZoneId.of("America/Toronto")
    private val locale = Locale.US
    private val today = LocalDate.of(2026, 10, 17) // a Saturday
    private val dayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()

    private fun at(date: LocalDate, h: Int, m: Int) = date.atTime(h, m).atZone(zone).toInstant().toEpochMilli()
    private fun time(h: Int, m: Int) =
        LocalTime.of(h, m).format(DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT).withLocale(locale))

    private fun line(startedAtMs: Long) =
        NotificationFeed.sessionStartedLine(startedAtMs, dayStartMs, zone, locale)

    @Test
    fun `a session from today gives just the time`() {
        assertEquals("Started at ${time(8, 41)}.", line(at(today, 8, 41)))
    }

    @Test
    fun `last night's session says yesterday`() {
        assertEquals("Started yesterday at ${time(23, 20)}.", line(at(today.minusDays(1), 23, 20)))
    }

    @Test
    fun `an older session names its weekday`() {
        assertEquals("Started Thu at ${time(7, 5)}.", line(at(today.minusDays(2), 7, 5)))
    }
}
