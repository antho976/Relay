package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for the frequency-aware volume model (program-unlock Phase 4). */
class VolumeModelTest {

    private fun weeklySets(days: List<DayArchetype>, muscle: MuscleGroup): Int {
        val sets = VolumeModel.allocate(days)
        var total = 0
        days.forEachIndexed { di, day ->
            day.targets.forEachIndexed { si, slot -> if (slot.muscle == muscle) total += sets[di][si] }
        }
        return total
    }

    @Test
    fun volumeScalesWithFrequency() {
        // Same template family, more days → the muscle is trained more often → more weekly volume.
        val threeDay = weeklySets(SplitTemplates.forDays(3), MuscleGroup.CHEST)
        val sixDay = weeklySets(SplitTemplates.forDays(6), MuscleGroup.CHEST)
        assertTrue("6-day chest ($sixDay) should exceed 3-day chest ($threeDay)", sixDay > threeDay)
    }

    @Test
    fun everySlotIsWithinSetBounds() {
        (1..7).forEach { d ->
            VolumeModel.allocate(SplitTemplates.forDays(d)).flatten().forEach { sets ->
                assertTrue("$sets out of bounds", sets in VolumeModel.MIN_SETS..VolumeModel.MAX_SETS)
            }
        }
    }

    private fun weeklySets(days: List<DayArchetype>, muscle: MuscleGroup, focus: Set<MuscleGroup>): Int {
        val sets = VolumeModel.allocate(days, focus)
        var total = 0
        days.forEachIndexed { di, day ->
            day.targets.forEachIndexed { si, slot -> if (slot.muscle == muscle) total += sets[di][si] }
        }
        return total
    }

    @Test
    fun emphasisRaisesFocusMusclesAndLeavesOthersAlone() {
        val days = SplitTemplates.forDays(3)
        val focus = VolumeModel.emphasisFocus("arms-shoulders")
        assertTrue(
            "arms emphasis should raise bicep volume",
            weeklySets(days, MuscleGroup.BICEPS, focus) > weeklySets(days, MuscleGroup.BICEPS, emptySet())
        )
        assertEquals(
            "quads untouched by arms emphasis",
            weeklySets(days, MuscleGroup.QUADS, emptySet()),
            weeklySets(days, MuscleGroup.QUADS, focus)
        )
    }

    @Test
    fun balancedEmphasisFocusIsEmpty() {
        assertEquals(emptySet<MuscleGroup>(), VolumeModel.emphasisFocus("balanced"))
    }

    @Test
    fun emphasisStaysBoundedByTheWeeklyCap() {
        // Regression: emphasis headroom used to be slots.size × bonus, letting a prioritised muscle on a
        // high-frequency split blow well past its junk-volume cap (measured 26 back sets vs a 20 cap).
        // It must now stay within a small bounded headroom above the cap.
        val sixDay = SplitTemplates.forDays(6)
        MuscleGroup.entries.forEach { muscle ->
            val cap = VolumeModel.weeklyCap[muscle] ?: return@forEach
            val focused = weeklySets(sixDay, muscle, setOf(muscle))
            assertTrue(
                "$muscle emphasis ($focused) should stay within cap ($cap) + small headroom",
                focused <= cap + 2
            )
        }
    }

    @Test
    fun perSessionVolumeStaysReasonable() {
        // A "standard" day should be ~12–24 sets, not 30 (the clamp-everything-to-max failure mode).
        VolumeModel.allocate(SplitTemplates.forDays(3)).forEach { day ->
            val total = day.sum()
            assertTrue("session total $total out of sane range", total in 12..24)
        }
    }
}
