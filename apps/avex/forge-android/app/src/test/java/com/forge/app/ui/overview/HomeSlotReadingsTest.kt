package com.forge.app.ui.overview

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.Session
import com.forge.app.data.repo.CoachHomeSummary
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneOffset

class HomeSlotReadingsTest {

    private val day = 24L * 60 * 60 * 1000
    private val today = 100 * day
    private val utc = ZoneOffset.UTC

    private fun session(daysAgo: Int) = Session(id = 7, dayKey = "legs-a", startedAt = today - daysAgo * day + 18 * 3_600_000)

    private val lines = listOf(
        StatsRepository.SessionExerciseLine("Back Squat", 275.0, 5, 4),
        StatsRepository.SessionExerciseLine("Romanian Deadlift", 225.0, 8, 3),
    )

    @Test
    fun lastSessionReadsItsTopSetAndHowManyLiftsWentUp() {
        val deltas = listOf(
            StatsRepository.LiftDelta("Back Squat", 4f),
            StatsRepository.LiftDelta("Romanian Deadlift", 2f),
            StatsRepository.LiftDelta("Leg Press", 0.2f),
            StatsRepository.LiftDelta("Walking Lunge", null),
            StatsRepository.LiftDelta("Calf Raise", -2f),
        )
        val r = lastSessionSlot(session(1), lines, deltas, WeightUnit.LB, today, utc)
        assertEquals("Yesterday", r.label)
        assertEquals("275", r.figure)
        assertEquals("×5 squat", r.unit)
        assertTrue(r.change!!.endsWith("· 2/4 up"))
        assertTrue(r.changeMoved)
        // A lift inside the matched band is a stub (0), a first-time lift is null.
        assertEquals(listOf(4f, 2f, 0f, null, -2f), r.bars)
    }

    @Test
    fun theLabelSaysWhenTheSessionWas() {
        assertEquals("Today", lastSessionSlot(session(0), lines, emptyList(), WeightUnit.LB, today, utc).label)
        assertEquals("Last session", lastSessionSlot(session(9), lines, emptyList(), WeightUnit.LB, today, utc).label)
    }

    @Test
    fun aLongLiftNameDropsOutOfTheFigure() {
        val r = lastSessionSlot(
            session(1),
            listOf(StatsRepository.SessionExerciseLine("Cable Tricep Pushdown", 54.6, 9, 3)),
            emptyList(), WeightUnit.LB, today, utc
        )
        assertEquals("×9", r.unit)
    }

    @Test
    fun compactNamesDropTheEquipment() {
        assertEquals("Tricep Pushdown", compactLiftName("Cable Tricep Pushdown"))
        assertEquals("Calf Raise", compactLiftName("Calf Raise (machine)"))
        assertEquals("Curl", compactLiftName("EZ-Bar Curl"))
        assertEquals("Barbell", compactLiftName("Barbell"))
        assertEquals("squat", shortLiftName("Barbell Back Squat"))
    }

    @Test
    fun bodyWeightComparesAgainstAboutAMonthAgo() {
        val entries = listOf(
            BodyweightEntry(dateKey = "2026-10-21", weightLb = 182.0, recordedAt = 0),
            BodyweightEntry(dateKey = "2026-10-10", weightLb = 183.0, recordedAt = 0),
            BodyweightEntry(dateKey = "2026-09-22", weightLb = 183.2, recordedAt = 0),
            BodyweightEntry(dateKey = "2026-08-01", weightLb = 190.0, recordedAt = 0),
        )
        val r = bodyWeightSlot(entries, WeightUnit.LB)!!
        assertEquals("182", r.figure)
        assertEquals(" lb", r.unit)
        assertEquals("↓ 1.2 lb in 30d", r.change)
        assertEquals(182f, r.spark.last(), 0.001f)
    }

    @Test
    fun bodyWeightWithoutAnOldEnoughWeighInHasNoChange() {
        val r = bodyWeightSlot(listOf(BodyweightEntry(dateKey = "2026-10-21", weightLb = 182.0, recordedAt = 0)), WeightUnit.LB)!!
        assertNull(r.change)
        assertFalse(r.changeMoved)
    }

    private fun decision(status: String, name: String, type: String = "volume_up", summary: String = "Add a set to $name (3 → 4)") =
        CoachDecision(weekId = "2026-W43", type = type, targetKey = name.lowercase(), targetName = name, summary = summary, reason = "", status = status)

    @Test
    fun coachLineLeadsWithTheOpenCallThenWhatIsOnWatch() {
        val line = coachLine(CoachHomeSummary(open = listOf(decision("proposed", "Bench Press")), watching = listOf(decision("applied", "Back Squat"))))
        assertEquals("Bench Press · 3 → 4 sets · Back Squat on watch", line)
        assertEquals("Holding steady", coachLine(CoachHomeSummary(emptyList(), emptyList())))
        assertEquals(
            "2 on watch",
            coachLine(CoachHomeSummary(emptyList(), listOf(decision("applied", "A"), decision("applied", "B"))))
        )
    }
}
