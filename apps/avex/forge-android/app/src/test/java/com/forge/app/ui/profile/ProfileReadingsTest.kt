package com.forge.app.ui.profile

import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneOffset
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfileReadingsTest {
    private val day = 86_400_000L
    private val zone = ZoneOffset.UTC
    private fun ms(d: LocalDate) = d.atStartOfDay(zone).toInstant().toEpochMilli()

    @Test
    fun compactFiguresReadAtAGlance() {
        assertEquals("2.9M", compactFigure(2_912_000.0))
        assertEquals("248k", compactFigure(248_400.0))
        assertEquals("12.5k", compactFigure(12_480.0))
        assertEquals("4,830", compactFigure(4830.0))
        assertEquals("64", compactFigure(64.0))
        assertEquals("0", compactFigure(0.0))
    }

    @Test
    fun liftedAddsUpTheLast30Days() {
        val now = 100 * day
        val r = liftedReading(listOf(1000.0, 3000.0, 6000.0), listOf(10 * day, 60 * day, 90 * day), now, WeightUnit.LB)
        assertEquals(6000.0, r.total, 1e-9)
        // The window opened at day 70, after the 3,000 total.
        assertEquals(3000.0, r.last30, 1e-9)
    }

    @Test
    fun theLifetimeCurveNeedsTwoWorkouts() {
        assertTrue(lifetimeCurve(listOf(10.0), listOf(0L), day, zone, Locale.US).points.isEmpty())
        val c = lifetimeCurve(listOf(10.0, 20.0), listOf(0L, 50 * day), 60 * day, zone, Locale.US)
        assertEquals(1f, c.points.last().x, 1e-6f)
        assertEquals(20f, c.points.last().y, 1e-6f)
    }

    private val plan: (LocalDate) -> Boolean? = { d -> d.dayOfWeek == DayOfWeek.THURSDAY || d.dayOfWeek == DayOfWeek.SUNDAY }

    @Test
    fun theMonthGridMarksEachDay() {
        val today = LocalDate.of(2026, 10, 21)
        val activity = mapOf(
            LocalDate.of(2026, 10, 2).toEpochDay() to 1,
            LocalDate.of(2026, 10, 12).toEpochDay() to 1,
        )
        val weeks = monthGrid(YearMonth.of(2026, 10), today, activity, setOf(LocalDate.of(2026, 10, 12).toEpochDay()), plan) { false }
        val byDay = weeks.flatten().filterNotNull().associateBy { it.date.dayOfMonth }
        // October 2026 opens on a Thursday: three empty cells before it.
        assertEquals(listOf(null, null, null), weeks.first().take(3))
        assertEquals(DayMark.REST, byDay[1]!!.mark)
        assertEquals(DayMark.TRAINED, byDay[2]!!.mark)
        assertEquals(DayMark.MISSED, byDay[5]!!.mark)
        assertEquals(DayMark.DELOAD, byDay[12]!!.mark)
        assertEquals(DayMark.TODAY, byDay[21]!!.mark)
        assertEquals(DayMark.FUTURE, byDay[22]!!.mark)
        assertTrue(weeks.all { it.size == 7 })
    }

    @Test
    fun aHolidayIsRestNotAMiss() {
        val today = LocalDate.of(2026, 10, 21)
        val weeks = monthGrid(YearMonth.of(2026, 10), today, emptyMap(), emptySet(), plan) { it.dayOfMonth in 5..9 }
        assertEquals(DayMark.REST, weeks.flatten().filterNotNull().first { it.date.dayOfMonth == 6 }.mark)
    }

    @Test
    fun weeksOnPlanCountsBackUntilAMissedWeek() {
        val today = LocalDate.of(2026, 10, 21) // a Wednesday
        val trained = generateSequence(LocalDate.of(2026, 9, 14)) { it.plusDays(1) }
            .takeWhile { it.isBefore(today) }
            .filter { plan(it) == false }
            .associate { it.toEpochDay() to 1 }
        // Five full weeks (Sep 14 to Oct 18) and this week on plan so far.
        assertEquals(6, weeksOnPlan(today, trained, plan, { false }))
        // Miss a day in the week of Oct 5 and the run starts after it: last week and this one.
        val missed = trained - LocalDate.of(2026, 10, 6).toEpochDay()
        assertEquals(2, weeksOnPlan(today, missed, plan, { false }))
    }

    @Test
    fun theConsistencyLineCountsPlannedDays() {
        val today = LocalDate.of(2026, 10, 8)
        val activity = listOf(2, 3, 5, 6, 7).associate { LocalDate.of(2026, 10, it).toEpochDay() to 1 }
        assertEquals(
            "5 of 5 planned days so far · 2 weeks on plan",
            consistencyLine(YearMonth.of(2026, 10), today, activity, plan, { false }, 2)
        )
        assertEquals(
            "5 days trained so far",
            consistencyLine(YearMonth.of(2026, 10), today, activity, { null }, { false }, 0)
        )
        assertEquals(
            "0 of 22 planned days",
            consistencyLine(YearMonth.of(2026, 9), today, activity, plan, { false }, 2)
        )
    }

    @Test
    fun weightReadsAsItsSevenDayAverageAgainstAMonthBefore() {
        val entries = (0..40).map { i ->
            BodyweightEntry(id = i.toLong(), dateKey = LocalDate.of(2026, 9, 1).plusDays(i.toLong()).toString(), weightLb = 190.0 - i * 0.1, recordedAt = i * day)
        }
        val r = weightReading(entries, goalLb = 180.0, unit = WeightUnit.LB)
        // The last seven weigh-ins: 186.6 … 186.0, averaging 186.3, against 189.3 a month before.
        assertEquals("186.3", r.figure)
        assertEquals("↓ 3.0 lb in 30 days · 6.3 to goal", r.change)
        assertTrue(r.moved)
    }

    @Test
    fun emptyReadingsShowZero() {
        val w = weightReading(emptyList(), null, WeightUnit.KG)
        assertEquals("0", w.figure)
        assertEquals(" kg", w.unit)
        assertTrue(w.empty)
        assertEquals("0", bodyFatReading(emptyList()).figure)
    }

    @Test
    fun bodyFatMovesInPoints() {
        val e = listOf(17.4, 17.0, 16.8).mapIndexed { i, p -> BodyFatEntry(id = i.toLong(), dateKey = "2026-10-0${i + 1}", percent = p, recordedAt = i * 10 * day) }
        val r = bodyFatReading(e)
        assertEquals("16.8", r.figure)
        assertEquals("↓ 0.6 pts", r.change)
    }

    @Test
    fun thenAndNowPrefersTheFrontPose() {
        val photos = listOf(
            ProgressPhoto("a", ms(LocalDate.of(2025, 3, 12)), pose = "FRONT", weightLb = 191.2),
            ProgressPhoto("b", ms(LocalDate.of(2025, 3, 12)), pose = "SIDE"),
            ProgressPhoto("c", ms(LocalDate.of(2026, 1, 2)), pose = "SIDE"),
            ProgressPhoto("d", ms(LocalDate.of(2026, 10, 19)), pose = "FRONT", weightLb = 182.4),
        )
        val p = thenAndNow(photos, zone)!!
        assertEquals("a", p.then.fileName)
        assertEquals("d", p.now.fileName)
        assertEquals(PhotoPose.FRONT, p.pose)
        assertEquals("19 months apart", p.apart)
        assertEquals(-8.8, p.weightDeltaLb!!, 1e-9)
        assertNull(thenAndNow(photos.take(1), zone))
    }

    @Test
    fun theGalleryRunsByMonthFrontSideBack() {
        val d = LocalDate.of(2026, 10, 3)
        val photos = listOf(
            ProgressPhoto("back", ms(d), pose = "BACK"),
            ProgressPhoto("front-old", ms(d.minusDays(1)), pose = "FRONT"),
            ProgressPhoto("front", ms(d.plusDays(1)), pose = "FRONT"),
            ProgressPhoto("loose", ms(d.plusDays(2))),
            ProgressPhoto("sept", ms(d.minusMonths(1))),
        )
        val months = galleryByMonth(photos, zone)
        assertEquals(listOf(YearMonth.of(2026, 10), YearMonth.of(2026, 9)), months.map { it.month })
        assertEquals(listOf("front", "back", "loose"), months.first().tiles.map { it.fileName })
        assertEquals(4, months.first().count)
    }
}
