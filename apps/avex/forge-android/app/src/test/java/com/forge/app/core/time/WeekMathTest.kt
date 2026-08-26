package com.forge.app.core.time

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.ZoneOffset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The app's "this week" and "this month" boundaries.
 *
 * Four lines of production code with thirteen callers — the coach's volume gate, the goal
 * portfolio, Stats, the cardio log, the backup window, the deload window — and until this file no
 * test at all. That gap is not theoretical: two suites in this repo were pinned to an epoch-day
 * "now" that happens to land on a MONDAY AT 00:00, so their "this week" was zero seconds long and
 * every fixture row fell into the previous week. Both read 0 while asserting they proved what the
 * week contained.
 *
 * Everything here is written as calendar dates rather than epoch arithmetic, because every bug this
 * code can have is a calendar bug: a Sunday counted into next week, a DST week assumed to be
 * 168 hours, a deload window that governs two days.
 */
class WeekMathTest {

    private val utc: ZoneId = ZoneOffset.UTC

    /** Europe/London: on UTC in winter, an hour ahead in summer — so the DST cases are real. */
    private val london: ZoneId = ZoneId.of("Europe/London")

    private fun at(date: String, time: String = "12:00", zone: ZoneId = utc): Long =
        LocalDateTime.parse("${date}T$time").atZone(zone).toInstant().toEpochMilli()

    private fun startOfDay(date: String, zone: ZoneId = utc): Long = at(date, "00:00", zone)

    // ── mondayStartMs ──────────────────────────────────────────────────────────────────────────

    @Test
    fun midWeekResolvesToThatWeeksMonday() {
        // 2026-08-27 is a Thursday; its ISO week starts Monday 2026-08-24.
        assertEquals(startOfDay("2026-08-24"), mondayStartMs(at("2026-08-27"), utc))
    }

    @Test
    fun sundayBelongsToTheWeekThatStartedSixDaysEarlier() {
        // The off-by-one every "start of week" implementation gets wrong at least once: ISO weeks
        // are Monday-anchored, so Sunday is the LAST day of its week, not the first day of the next.
        assertEquals(startOfDay("2026-08-24"), mondayStartMs(at("2026-08-30", "23:59"), utc))
    }

    @Test
    fun mondayResolvesToItsOwnMidnight() {
        assertEquals(startOfDay("2026-08-24"), mondayStartMs(at("2026-08-24", "09:15"), utc))
    }

    @Test
    fun aMondayMidnightIsAlreadyTheWeekStart() {
        // The exact instant the two coach suites were pinned to. It is the week START, which makes
        // "so far this week" empty — correct, and worth stating so nobody "fixes" it later.
        val mondayMidnight = startOfDay("2026-08-24")
        assertEquals(mondayMidnight, mondayStartMs(mondayMidnight, utc))
    }

    @Test
    fun theResultIsAlwaysLocalMidnightOnAMonday() {
        // Swept across a full year rather than spot-checked: this is the invariant every caller
        // relies on, and the cheapest way to catch a zone or DST edge nobody thought to name.
        var date = LocalDate.parse("2026-01-01")
        val end = LocalDate.parse("2027-01-01")
        while (date < end) {
            for (zone in listOf(utc, london, ZoneId.of("America/New_York"), ZoneId.of("Australia/Sydney"))) {
                val start = mondayStartMs(at(date.toString(), "13:45", zone), zone)
                val asLocal = java.time.Instant.ofEpochMilli(start).atZone(zone)
                assertEquals("$date in $zone", DayOfWeek.MONDAY, asLocal.dayOfWeek)
                assertEquals("$date in $zone", 0, asLocal.hour)
                assertEquals("$date in $zone", 0, asLocal.minute)
                assertTrue("$date in $zone: week start must not be in the future", start <= at(date.toString(), "13:45", zone))
            }
            date = date.plusDays(1)
        }
    }

    @Test
    fun theBoundaryIsLocalMidnight_notUtcMidnight() {
        // Sydney is +10/+11, so its Monday starts BEFORE UTC's. A "this week" computed in UTC for a
        // Sydney user silently drops their whole Monday — the day a weekly target is most often hit.
        val sydney = ZoneId.of("Australia/Sydney")
        val mondayMorningSydney = at("2026-08-24", "07:00", sydney)
        assertEquals(startOfDay("2026-08-24", sydney), mondayStartMs(mondayMorningSydney, sydney))
        // Read in UTC, that same instant is still SUNDAY, so it belongs to the previous week.
        assertEquals(startOfDay("2026-08-17", utc), mondayStartMs(mondayMorningSydney, utc))
    }

    @Test
    fun aSpringForwardWeekIsOnly167HoursLong() {
        // British Summer Time starts on Sunday 2026-03-29. Any code that adds 7 * 24 h to a week
        // start instead of asking the calendar lands an hour inside the NEXT week here.
        val thisWeek = mondayStartMs(at("2026-03-25", "12:00", london), london)
        val nextWeek = mondayStartMs(at("2026-04-01", "12:00", london), london)
        assertEquals(startOfDay("2026-03-23", london), thisWeek)
        assertEquals(167L, (nextWeek - thisWeek) / 3_600_000L)
    }

    @Test
    fun anAutumnFallBackWeekIs169HoursLong() {
        // The same failure in the other direction: clocks go back on Sunday 2026-10-25, so a fixed
        // 7 * 24 h stride ends an hour SHORT and loses the last hour of Sunday.
        val thisWeek = mondayStartMs(at("2026-10-21", "12:00", london), london)
        val nextWeek = mondayStartMs(at("2026-10-28", "12:00", london), london)
        assertEquals(169L, (nextWeek - thisWeek) / 3_600_000L)
    }

    // ── monthStartMs ───────────────────────────────────────────────────────────────────────────

    @Test
    fun monthStartIsMidnightOnTheFirst() {
        assertEquals(startOfDay("2026-08-01"), monthStartMs(at("2026-08-27"), utc))
        assertEquals(startOfDay("2026-08-01"), monthStartMs(at("2026-08-01", "00:00"), utc))
        assertEquals(startOfDay("2026-08-01"), monthStartMs(at("2026-08-31", "23:59"), utc))
    }

    @Test
    fun monthStartHandlesFebruaryInALeapYear() {
        assertEquals(startOfDay("2024-02-01"), monthStartMs(at("2024-02-29"), utc))
    }

    @Test
    fun monthStartIsAlsoLocal_notUtc() {
        val sydney = ZoneId.of("Australia/Sydney")
        val firstMorningSydney = at("2026-08-01", "07:00", sydney)
        assertEquals(startOfDay("2026-08-01", sydney), monthStartMs(firstMorningSydney, sydney))
        // Still July in UTC at that instant.
        assertEquals(startOfDay("2026-07-01", utc), monthStartMs(firstMorningSydney, utc))
    }

    // ── The deload window ──────────────────────────────────────────────────────────────────────

    @Test
    fun aDeloadWindowStartsOnItsOwnWeeksMonday() {
        assertEquals(
            startOfDay("2026-08-24"),
            deloadWeekStartMs(at("2026-08-27", "19:00"), utc)
        )
    }

    @Test
    fun aDeloadAppliedMondayToThursdayGovernsThatWeekOnly() {
        // Applied Mon..Thu, at least four days of the week remain, so one week is enough.
        for (day in listOf("2026-08-24", "2026-08-25", "2026-08-26", "2026-08-27")) {
            assertEquals(
                "applied $day",
                startOfDay("2026-08-31"),
                deloadWeekEndMs(at(day, "19:00"), utc)
            )
        }
    }

    @Test
    fun aDeloadAppliedFridayToSundayRunsIntoTheFollowingWeek() {
        // Fri/Sat/Sun leave three days or fewer — a "deload week" that short is not a deload, so the
        // window extends. Note the boundary is FRIDAY, not Thursday as the KDoc used to say.
        for (day in listOf("2026-08-28", "2026-08-29", "2026-08-30")) {
            assertEquals(
                "applied $day",
                startOfDay("2026-09-07"),
                deloadWeekEndMs(at(day, "19:00"), utc)
            )
        }
    }

    @Test
    fun aDeloadWindowAlwaysGovernsAtLeastFourDaysAndEndsOnAMonday() {
        // The property the two branches above exist to produce, checked over a year so a change to
        // the >= 4 threshold cannot pass by updating one hand-written expectation.
        var date = LocalDate.parse("2026-01-01")
        val end = LocalDate.parse("2027-01-01")
        while (date < end) {
            val applied = at(date.toString(), "19:00", london)
            val windowEnd = deloadWeekEndMs(applied, london)
            val endLocal = java.time.Instant.ofEpochMilli(windowEnd).atZone(london)
            assertEquals("$date", DayOfWeek.MONDAY, endLocal.dayOfWeek)
            assertEquals("$date", 0, endLocal.hour)

            val remainingDays = (windowEnd - applied) / 86_400_000L
            assertTrue(
                "$date: a deload window must govern at least four days, got $remainingDays",
                remainingDays >= 3   // 3 whole days + the rest of the applied day = the fourth
            )
            assertTrue("$date: the window must contain the moment it was applied", windowEnd > applied)
            assertTrue(
                "$date: the window starts at its own week's Monday",
                deloadWeekStartMs(applied, london) <= applied
            )
            date = date.plusDays(1)
        }
    }

    @Test
    fun theDeloadWindowIsHalfOpen() {
        // end is EXCLUSIVE: the Monday that ends one deload window is day one of the next week, and
        // a session logged at exactly that instant must not be marked as a deload session.
        val applied = at("2026-08-24", "19:00", utc)
        val end = deloadWeekEndMs(applied, utc)
        assertEquals(startOfDay("2026-08-31"), end)
        assertEquals(startOfDay("2026-08-31"), mondayStartMs(end, utc))
    }
}
