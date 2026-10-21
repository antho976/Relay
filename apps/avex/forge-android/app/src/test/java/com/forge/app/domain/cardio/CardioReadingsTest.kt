package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneOffset

class CardioReadingsTest {

    private val zone = ZoneOffset.UTC
    private val today = LocalDate.of(2026, 10, 21) // a Wednesday

    private fun ms(d: LocalDate, hour: Int = 12) = d.atTime(hour, 0).toInstant(ZoneOffset.UTC).toEpochMilli()
    private var nextId = 1L
    private fun e(d: LocalDate, type: String = "run", min: Int = 30, km: Double? = 5.0) =
        CardioEntry(nextId++, ms(d), type, min, km)

    @Test
    fun weekStreakCountsBackFromThisWeekOrLast() {
        val monday = LocalDate.of(2026, 10, 19)
        val entries = listOf(e(monday), e(monday.minusWeeks(1)), e(monday.minusWeeks(2)), e(monday.minusWeeks(4)))
        assertEquals(3, cardioWeekStreak(entries, zone, today))
        // Nothing yet this week still keeps last week's run going.
        assertEquals(2, cardioWeekStreak(entries.drop(1), zone, today))
        // A rest day is not a session.
        assertEquals(0, cardioWeekStreak(listOf(e(monday, type = "rest")), zone, today))
    }

    @Test
    fun lastWeekToDateStopsAtTheSameWeekday() {
        val lastMon = LocalDate.of(2026, 10, 12)
        val entries = listOf(e(lastMon, km = 3.0), e(lastMon.plusDays(2), km = 2.0), e(lastMon.plusDays(3), km = 9.0))
        assertEquals(5.0, lastWeekToDateKm(entries, zone, today), 1e-9)
    }

    @Test
    fun recentTypesAreDistinctNewestFirst() {
        val entries = listOf(e(today, "walk"), e(today.minusDays(1), "run"), e(today.minusDays(2), "walk"), e(today.minusDays(3), "rest"), e(today.minusDays(4), "cycle"))
        assertEquals(listOf("walk", "run", "cycle"), recentCardioTypes(entries))
    }

    @Test
    fun usualStepsNeedsThreeDaysAndSumsToTheHour() {
        fun day(vararg s: Pair<Int, Int>) = CardioWearableDay(s.map { HourlySteps(it.first, it.second) })
        val days = listOf(day(8 to 1000, 14 to 500), day(8 to 2000), day(9 to 3000, 20 to 100))
        assertEquals(2000, usualStepsByHour(days, 13))
        assertNull(usualStepsByHour(days.take(2), 13))
    }

    @Test
    fun standingsRankWithinTheActivity() {
        val mine = e(today, km = 6.0, min = 30)
        val others = listOf(e(today.minusDays(3), km = 8.0, min = 48), e(today.minusDays(5), km = 4.0, min = 28), e(today.minusDays(6), "walk", km = 9.0))
        val dist = distanceStanding(mine, others + mine)!!
        assertEquals(2, dist.rank)
        assertEquals(3, dist.values.size)
        val pace = paceStanding(mine, others + mine)!!
        assertEquals(1, pace.rank) // 5:00/km beats 6:00 and 7:00
    }

    @Test
    fun longestInWeeksNamesHowLongItStood() {
        val mine = e(today, km = 7.0)
        val beat = e(today.minusWeeks(3), km = 7.5)
        val shorter = e(today.minusWeeks(1), km = 5.0)
        assertEquals(3, longestInWeeks(mine, listOf(beat, shorter, mine), zone))
        assertEquals(Int.MAX_VALUE, longestInWeeks(mine, listOf(shorter, mine), zone))
        assertNull(longestInWeeks(mine, listOf(e(today.minusDays(3), km = 8.0), mine), zone))
    }

    @Test
    fun bestsPickTheFastestTheLongestAndTheBiggestWeek() {
        val monday = LocalDate.of(2026, 10, 12)
        val a = e(monday, km = 5.0, min = 25)
        val b = e(monday.plusDays(2), km = 6.0, min = 40)
        val c = e(monday.minusWeeks(2), km = 10.0, min = 70)
        val bests = cardioBests(listOf(a, b, c), "run", zone)!!
        assertEquals(a.id, bests.fastest?.id)
        assertEquals(c.id, bests.longest?.id)
        assertEquals(11.0, bests.bestWeekKm, 1e-9)
        assertEquals(ms(monday, 0), bests.bestWeekStartMs)
    }

    @Test
    fun paceWindowFallsBackToAllTimeWhenTheWindowIsThin() {
        val series = CardioPaceSeries("run", listOf(
            CardioPacePoint(ms(today.minusWeeks(30)), 30, 5.0),
            CardioPacePoint(ms(today.minusWeeks(20)), 28, 5.0),
            CardioPacePoint(ms(today.minusDays(2)), 27, 5.0),
        ))
        val w = cardioPaceWindow(series, ms(today, 0), useMiles = false)!!
        assertTrue(w.allTime)
        assertEquals(3, w.points.size)
        assertEquals(324.0, w.points.last().second, 1e-9)
    }

    @Test
    fun ridesReadAsSpeed() {
        assertTrue(readsAsSpeed("cycle"))
        assertEquals(20.0, speedPerHour(30, 10.0, useMiles = false)!!, 1e-9)
    }
}
