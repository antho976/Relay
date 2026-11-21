package com.forge.app.ui.recap

import com.forge.app.data.db.entities.Session
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneOffset
import java.util.Locale

class RecapMathTest {

    private fun at(d: LocalDate) = d.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() + 3_600_000L * 17

    private fun session(id: Long, d: LocalDate, vol: Double, prs: Int = 0, type: String = "normal") =
        Session(id = id, dayKey = "push_a", startedAt = at(d), finishedAt = at(d) + 3_000_000, totalVolumeLb = vol, prCount = prs, setCount = 10, sessionType = type)

    @Test
    fun changesAgainstThePreviousPeriod() {
        assertEquals("↑ 2 on Sep" to true, countChange(12, 10, "Sep"))
        assertEquals("Same as Sep" to false, countChange(3, 3, "Sep"))
        assertEquals("↑ 6%", percentChange(106.0, 100.0))
        assertNull(percentChange(5.0, 0.0))
    }

    @Test
    fun volumeSplitsIntoFigureAndSuffix() {
        assertEquals("248" to "k", volumeParts(248_000.0, WeightUnit.LB))
        assertEquals("2.1" to "M", volumeParts(2_100_000.0, WeightUnit.LB))
        assertEquals("950" to "", volumeParts(950.0, WeightUnit.LB))
    }

    @Test
    fun weeksFoldTheMonthsTailIntoTheFourth() {
        assertEquals(0, weekBucket(1))
        assertEquals(2, weekBucket(21))
        assertEquals(3, weekBucket(31))
    }

    @Test
    fun biggestJumpBeatsTheBestBeforeThePeriod() {
        val from = at(LocalDate.of(2026, 10, 1))
        val to = at(LocalDate.of(2026, 11, 1))
        val sets = listOf(
            RecapSet("bench", null, from - 1000, 195.0, 5),
            RecapSet("bench", null, from + 1000, 205.0, 5),
            RecapSet("row", null, from + 2000, 150.0, 8),
        )
        val (set, gain, _) = biggestJump(sets, from, to)!!
        assertEquals("bench", set.exerciseId)
        assertEquals(205.0 * (1 + 5 / 30.0) - 195.0 * (1 + 5 / 30.0), gain, 1e-6)
        // The row has no earlier best, so on its own it is not a jump, unless measured from the period's start.
        assertNull(biggestJump(sets.drop(2), from, to))
    }

    @Test
    fun monthRecapNamesTheWeekStillGoingAndTheDeload() {
        val today = LocalDate.of(2026, 10, 21)
        val oct = listOf(
            session(1, LocalDate.of(2026, 10, 2), 15_000.0),
            session(2, LocalDate.of(2026, 10, 16), 9_000.0, type = "deload"),
            session(3, LocalDate.of(2026, 10, 19), 16_000.0, prs = 2),
        )
        val sep = listOf(session(4, LocalDate.of(2026, 9, 3), 14_000.0))
        val m = buildMonthRecap(YearMonth.of(2026, 10), oct, sep, emptyList(), "Row" to 2, today, WeightUnit.LB, Locale.US, ZoneOffset.UTC, { id, _ -> id }, { "Push A" })
        assertEquals("21 of 31 days", m.daysLine)
        assertEquals(2, m.currentWeek)
        assertEquals("Week 3 was a deload · Week 3 is still going", m.weekCaption)
        assertEquals("↑ 2 on Sep", m.figures[0].change)
        assertTrue(m.highlights.any { it.label == "Best day" && it.sub == "Push A · 2 PRs" })
    }

    @Test
    fun yearRecapLightsTheBestMonth() {
        val sessions = listOf(1, 1, 5, 5, 5, 10).mapIndexed { i, m -> session(i.toLong(), LocalDate.of(2026, m, 3 + i), 10_000.0) }
        val y = buildYearRecap(2026, sessions, emptyList(), null, 3, LocalDate.of(2026, 10, 21), WeightUnit.LB, Locale.US, ZoneOffset.UTC) { id, _ -> id }
        assertEquals(4, y.bestMonth)
        assertEquals(9, y.currentMonth)
        assertEquals("Best: May", y.bestLine)
        assertEquals("October is still going", y.caption)
    }
}
