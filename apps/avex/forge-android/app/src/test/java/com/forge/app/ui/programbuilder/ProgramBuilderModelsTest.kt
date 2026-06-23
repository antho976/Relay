package com.forge.app.ui.programbuilder

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure mapping + reorder used by the routine builder. */
class ProgramBuilderModelsTest {

    private fun ex(id: String, sets: Int = 3, reps: String = "8-12") =
        BuilderExercise(uid = "u-$id", libId = id, name = id, muscle = "Chest", sets = sets, reps = reps)

    @Test
    fun toEntities_mapsPositionsKeysAndSlots() {
        val days = listOf(
            BuilderDay(
                uid = "ud1", key = "day-a", name = "Push", archetype = "push", accentHex = "#E85D4A",
                exercises = listOf(ex("db-bench-press", 4, "6-10"), ex("pec-deck"))
            ),
            BuilderDay(
                uid = "ud2", key = "day-b", name = "Legs", archetype = "legs", accentHex = "#D4A017",
                exercises = listOf(ex("goblet-squat"))
            )
        )
        val (dayRows, slotRows) = days.toEntities()

        assertEquals(2, dayRows.size)
        assertEquals(0, dayRows[0].position)
        assertEquals(1, dayRows[1].position)
        assertEquals("day-a", dayRows[0].id)
        assertEquals("Push", dayRows[0].name)
        assertEquals("PUSH", dayRows[0].word)        // derived from archetype
        assertEquals("push", dayRows[0].archetype)
        assertEquals("LEGS", dayRows[1].word)

        val dayASlots = slotRows.filter { it.dayId == "day-a" }
        assertEquals(2, dayASlots.size)
        assertEquals("day-a-0", dayASlots[0].id)
        assertEquals(0, dayASlots[0].position)
        assertEquals("db-bench-press", dayASlots[0].exerciseLibId)
        assertEquals(4, dayASlots[0].sets)
        assertEquals("6-10", dayASlots[0].reps)
        assertEquals(1, slotRows.count { it.dayId == "day-b" })
    }

    @Test
    fun toEntities_blankNameFallsBackToDayN() {
        val rows = listOf(
            BuilderDay(uid = "u", key = "day-x", name = "  ", archetype = "fb", accentHex = "#000", exercises = emptyList())
        ).toEntities().first
        assertEquals("Day 1", rows[0].name)
    }

    @Test
    fun moved_reordersWithinBounds() {
        val list = listOf("a", "b", "c")
        assertEquals(listOf("b", "a", "c"), list.moved(0, 1))
        assertEquals(listOf("b", "c", "a"), list.moved(0, 2))
    }

    @Test
    fun moved_outOfRangeReturnsSame() {
        val list = listOf("a", "b")
        assertSame(list, list.moved(0, 5))
        assertSame(list, list.moved(1, 1))
    }

    @Test
    fun emptyBuilderProducesEmptyProgram() {
        val (days, slots) = emptyList<BuilderDay>().toEntities()
        assertTrue(days.isEmpty())
        assertTrue(slots.isEmpty())
    }
}
