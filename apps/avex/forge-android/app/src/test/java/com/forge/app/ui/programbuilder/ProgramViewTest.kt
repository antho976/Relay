package com.forge.app.ui.programbuilder

import com.forge.app.data.db.entities.OverlaySource
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate

/** The Program redesign's readings: the title line, the coach's tag, a day's date, a lift's load. */
class ProgramViewTest {

    private fun day(name: String, type: String) = BuilderDay("u$name", "k$name", name, type, "#E85D4A", emptyList())

    @Test
    fun metaNamesTheSplitThenTheBlock() {
        val days = listOf(day("Push A", "push"), day("Pull A", "pull"), day("Legs A", "legs"), day("Push B", "push"))
        assertEquals("Push · Pull · Legs", programMeta(days, null))
        assertEquals("Push · Pull · Legs · week 3 of 8, Accumulate", programMeta(days, BlockReading(3, 8, "Accumulate")))
        assertEquals("", programMeta(emptyList(), null))
    }

    private fun coach(sets: Int = 0, reps: String? = null, source: String = OverlaySource.COACH) =
        ProgramCustomization(dayKey = "d", exerciseId = "x", setsOverride = sets, repRangeOverride = reps, source = source)

    @Test
    fun onlyTheCoachsChangeIsTagged() {
        assertEquals("↑ 1 rep", coachChangeTag(3, "8-10", coach(reps = "9-11")))
        assertEquals("↓ 1 set", coachChangeTag(4, "5", coach(sets = 3)))
        assertEquals("↑ 2 sets", coachChangeTag(3, "5", coach(sets = 5)))
        assertNull(coachChangeTag(3, "8-10", coach(reps = "9-11", source = OverlaySource.USER)))
        assertNull(coachChangeTag(3, "8-10", null))
        assertNull(coachChangeTag(3, "8-10", coach(sets = 3, reps = "8-10")))
    }

    @Test
    fun aDayReadsItsWeekdayAndDate() {
        val wednesday = LocalDate.of(2026, 10, 21)
        assertEquals("Mon · 19", weekdayDateLabel(setOf(0), wednesday))
        assertEquals("Sat · 24", weekdayDateLabel(setOf(5), wednesday))
        assertEquals("Wed · 21", weekdayDateLabel(setOf(5, 2), wednesday))
        assertEquals("Any day", weekdayDateLabel(emptySet(), wednesday))
    }

    @Test
    fun loadsReadInTheUsersUnit() {
        assertEquals("195 lb", loadLabel(PlannedLoad(195.0, false), WeightUnit.LB))
        assertEquals("+25 lb", loadLabel(PlannedLoad(25.0, true), WeightUnit.LB))
        assertEquals("BW", loadLabel(PlannedLoad(null, true), WeightUnit.LB))
        assertNull(loadLabel(PlannedLoad(null, false), WeightUnit.LB))
        assertNull(loadLabel(null, WeightUnit.LB))
    }

    @Test
    fun setsTimesRepsUsesARangeDash() {
        assertEquals("3 × 8–10", setsTimesReps(3, "8-10"))
        assertEquals("4 × 5", setsTimesReps(4, "5"))
    }
}
