package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The split reshaped around skipped, light and focused muscles ([SplitTemplates.forDays]). */
class BodyPartsSplitTest {

    private fun muscles(days: List<DayArchetype>) = days.flatMap { it.targets }.map { it.muscle }.toSet()
    private fun slotsOf(days: List<DayArchetype>, m: MuscleGroup) = days.sumOf { d -> d.targets.count { it.muscle == m } }
    private val noLegs = BodyParts(skip = BodyParts.LEGS)
    private val noUpper = BodyParts(skip = BodyParts.UPPER)

    @Test
    fun `nothing set is exactly the ordinary split`() {
        for (d in 1..7) assertEquals(SplitTemplates.forDays(d), SplitTemplates.forDays(d, BodyParts.NONE))
    }

    @Test
    fun `no legs leaves no leg slot on any day count`() {
        for (d in 1..7) {
            val week = SplitTemplates.forDays(d, noLegs)
            assertTrue("days=$d", muscles(week).none { it in BodyParts.LEGS })
            assertTrue("days=$d", week.isNotEmpty())
        }
    }

    @Test
    fun `no legs on four days is push and pull twice, not two half-empty lower days`() {
        val week = SplitTemplates.forDays(4, noLegs)
        assertEquals(listOf("push-a", "pull-a", "push-b", "pull-b"), week.map { it.key })
        assertTrue(week.all { it.targets.size >= 5 })
    }

    @Test
    fun `no legs on three days is push, pull and upper`() {
        assertEquals(listOf("push", "pull", "upper"), SplitTemplates.forDays(3, noLegs).map { it.key })
    }

    @Test
    fun `no upper body is leg days only, four a week at most`() {
        for (d in 1..7) {
            val week = SplitTemplates.forDays(d, noUpper)
            assertTrue("days=$d", muscles(week).none { it in BodyParts.UPPER })
            assertTrue("days=$d", week.size <= 4)
            assertEquals("days=$d", minOf(d, 4), week.size)
        }
    }

    @Test
    fun `day keys stay unique in every reshaped week`() {
        for (parts in listOf(noLegs, noUpper)) for (d in 1..7) {
            val keys = SplitTemplates.forDays(d, parts).map { it.key }
            assertEquals(keys.size, keys.toSet().size)
        }
    }

    @Test
    fun `one small muscle skipped drops its slots and moves nothing else`() {
        val normal = SplitTemplates.forDays(4)
        val week = SplitTemplates.forDays(4, BodyParts(skip = setOf(MuscleGroup.CALVES)))
        assertEquals(normal.map { it.key }, week.map { it.key })
        assertEquals(0, slotsOf(week, MuscleGroup.CALVES))
        normal.zip(week).forEach { (a, b) ->
            assertEquals(a.targets.filter { it.muscle != MuscleGroup.CALVES }, b.targets)
        }
    }

    @Test
    fun `a day left with one slot is dropped`() {
        // The seven-day split's last day is core and calves; without calves it is one slot.
        val week = SplitTemplates.forDays(7, BodyParts(skip = setOf(MuscleGroup.CALVES)))
        assertFalse(week.any { it.key == "core-calves" })
        assertEquals(6, week.size)
    }

    @Test
    fun `everything but the core out is one short core day`() {
        val week = SplitTemplates.forDays(5, BodyParts(skip = BodyParts.LEGS + BodyParts.UPPER))
        assertEquals(1, week.size)
        assertEquals(setOf(MuscleGroup.CORE), muscles(week))
    }

    @Test
    fun `a light muscle keeps one slot a week`() {
        val week = SplitTemplates.forDays(6, BodyParts(light = setOf(MuscleGroup.QUADS)))
        assertEquals(1, slotsOf(week, MuscleGroup.QUADS))
        // Nothing else lost a slot.
        assertEquals(slotsOf(SplitTemplates.forDays(6), MuscleGroup.CHEST), slotsOf(week, MuscleGroup.CHEST))
    }

    @Test
    fun `a focused muscle leads the days that train it`() {
        val week = SplitTemplates.forDays(4, BodyParts(focus = setOf(MuscleGroup.BICEPS)))
        week.filter { d -> d.targets.any { it.muscle == MuscleGroup.BICEPS } }
            .forEach { assertEquals(it.key, MuscleGroup.BICEPS, it.targets.first().muscle) }
    }

    @Test
    fun `a focused muscle is trained more often, up to three times, only on days of its region`() {
        // Five days: calves sit on legs and lower. Focus adds no calf slot to push, pull or upper.
        val normal = SplitTemplates.forDays(5)
        val week = SplitTemplates.forDays(5, BodyParts(focus = setOf(MuscleGroup.CALVES)))
        val daysWith = { w: List<DayArchetype> -> w.filter { d -> d.targets.any { it.muscle == MuscleGroup.CALVES } }.map { it.key } }
        assertEquals(daysWith(normal), daysWith(week))
        // Four days upper/lower: biceps is already on both upper days and gains nothing on lower days.
        val arms = SplitTemplates.forDays(4, BodyParts(focus = setOf(MuscleGroup.BICEPS)))
        assertEquals(listOf("upper-a", "upper-b"), arms.filter { d -> d.targets.any { it.muscle == MuscleGroup.BICEPS } }.map { it.key })
        // Six days: rear delts are on the two pull days; the push days are upper-body days and one takes a slot.
        val rear = SplitTemplates.forDays(6, BodyParts(focus = setOf(MuscleGroup.REAR_DELTS)))
        assertEquals(3, rear.count { d -> d.targets.any { it.muscle == MuscleGroup.REAR_DELTS } })
    }

    @Test
    fun `a muscle that is skipped is not also focused`() {
        val week = SplitTemplates.forDays(4, BodyParts(skip = setOf(MuscleGroup.CALVES), focus = setOf(MuscleGroup.CALVES)))
        assertEquals(0, slotsOf(week, MuscleGroup.CALVES))
    }

    // ── Volume ────────────────────────────────────────────────────────────────

    private fun weekly(days: List<DayArchetype>, sets: List<List<Int>>, m: MuscleGroup): Int =
        days.indices.sumOf { di -> days[di].targets.indices.filter { days[di].targets[it].muscle == m }.sumOf { sets[di][it] } }

    @Test
    fun `a light muscle runs two sets a week`() {
        val parts = BodyParts(light = setOf(MuscleGroup.QUADS))
        val days = SplitTemplates.forDays(4, parts)
        val sets = VolumeModel.allocate(days, light = parts.light)
        assertEquals(VolumeModel.LIGHT_SETS, weekly(days, sets, MuscleGroup.QUADS))
    }

    @Test
    fun `a focus gains weekly sets and the rest of the week gives some back`() {
        val parts = BodyParts(focus = setOf(MuscleGroup.BACK))
        val normalDays = SplitTemplates.forDays(4)
        val normal = VolumeModel.allocate(normalDays)
        val days = SplitTemplates.forDays(4, parts)
        val sets = VolumeModel.allocate(days, focus = parts.focus, specialize = parts.focus)
        assertTrue(weekly(days, sets, MuscleGroup.BACK) > weekly(normalDays, normal, MuscleGroup.BACK))
        // Chest's hypertrophy accessory gave up a set; its heavy compounds did not.
        assertTrue(weekly(days, sets, MuscleGroup.CHEST) < weekly(normalDays, normal, MuscleGroup.CHEST))
        days.forEachIndexed { di, d ->
            d.targets.forEachIndexed { si, slot ->
                if (slot.muscle == MuscleGroup.CHEST && slot.scheme == RepScheme.STRENGTH) assertEquals(3, sets[di][si])
            }
        }
    }

    @Test
    fun `the generator honours a skip through every day count and equipment set`() {
        val params = GenerationParams(daysPerWeek = 4, skipMuscles = BodyParts.LEGS)
        val week = ProgramGenerator.generate(params, Equipment.entries.toSet(), emptySet(), emptySet(), seed = 7L)
        val picked = week.flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId) }
        assertTrue(picked.isNotEmpty())
        assertTrue(picked.none { it.muscle in BodyParts.LEGS })
        assertEquals(listOf("push-a", "pull-a", "push-b", "pull-b"), week.map { it.key })
    }
}
