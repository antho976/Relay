package com.forge.app.ui.programbuilder

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The plan editor's "Day of the week" picks, as they become the weekly schedule. */
// Robolectric: the round-trips go through org.json, which the plain JVM only has as android.jar
// stubs that throw, so these tests could never pass outside it.
@RunWith(RobolectricTestRunner::class)
class ProgramBuilderWeekdayTest {

    private fun day(key: String, weekdays: Set<Int> = emptySet()) = BuilderDay(
        uid = "u-$key", key = key, name = key, archetype = "fb", accentHex = "#E85D4A",
        exercises = emptyList(), weekdays = weekdays
    )

    @Test
    fun noPicksMeansInSequence() {
        assertNull(listOf(day("a"), day("b")).toSchedule())
    }

    @Test
    fun picksBecomeTheSchedule() {
        val s = listOf(day("a", setOf(0, 3)), day("b", setOf(1)), day("c")).toSchedule()
        assertEquals(listOf("a", "b", "", "a", "", "", ""), s)
    }

    @Test
    fun theLabelReadsInCalendarOrder() {
        assertEquals("Mon · Thu", day("a", setOf(3, 0)).weekdayLabel())
        assertNull(day("a").weekdayLabel())
    }

    @Test
    fun weekdaysSurviveTheDraft() {
        val draft = ProgramBuilderDraft(listOf(day("a", setOf(2, 5)), day("b")), dirty = true, openDayUid = null, dialog = DayDialog.None)
        val back = ProgramBuilderDraft.fromJson(draft.toJson())!!
        assertEquals(setOf(2, 5), back.days[0].weekdays)
        assertEquals(emptySet<Int>(), back.days[1].weekdays)
    }
}
