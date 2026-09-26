package com.forge.app.ui.gym.freestyle

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The rules behind the set-by-set logger: what the entry slab starts from, when a set earns the
 * beat-last-time mark, and that a draft from the previous logger (which kept one blank row per
 * exercise) restores without phantom sets.
 */
@RunWith(RobolectricTestRunner::class)
class FreestyleFlowTest {

    private val bench = FsExercise("custom-bench", "Bench", MuscleGroup.CHEST, bodyweight = false, custom = true)

    private fun prior(lb: Double?, reps: Int, index: Int = 0) =
        LoggedSet(loggedExerciseId = 1, setIndex = index, weightText = lb?.toString() ?: "BW", weightLb = lb, reps = reps, completedAt = 0)

    @Test
    fun theSlabRepeatsTheSetJustLoggedWithItsTagsCleared() {
        val ex = bench.copy(sets = listOf(FsSet("100", "8", setType = "warmup", rpe = 7.0)))
        assertEquals(FsSet("100", "8"), ex.seedEntry(listOf(prior(200.0, 5)), WeightUnit.LB))
    }

    @Test
    fun aFreshExerciseStartsFromLastTimesOpeningSet() {
        val last = listOf(prior(100.0, 10, 0), prior(110.0, 8, 1))
        assertEquals(FsSet("100", "10"), bench.seedEntry(last, WeightUnit.LB))
        // Once a set is logged, it leads, not last time.
        val one = bench.copy(sets = listOf(FsSet("105", "9")))
        assertEquals(FsSet("105", "9"), one.seedEntry(last, WeightUnit.LB))
        assertEquals(FsSet(), bench.seedEntry(emptyList(), WeightUnit.LB))
    }

    @Test
    fun lastTimeIsReadInTheCurrentUnit() {
        val seeded = bench.seedEntry(listOf(prior(fromDisplayWeight(100.0, WeightUnit.KG), 5)), WeightUnit.KG)
        assertEquals("100", seeded.weight)
    }

    @Test
    fun onlyASetThatOutLiftsEveryPriorSetBeatsLastTime() {
        val best = lastTimeBestLb(listOf(prior(100.0, 10), prior(110.0, 5)))
        assertTrue(bench.beatsLastTime(FsSet("100", "11"), best, WeightUnit.LB))
        assertFalse(bench.beatsLastTime(FsSet("100", "10"), best, WeightUnit.LB))
        // No history, no mark; and a bodyweight move is never rated.
        assertFalse(bench.beatsLastTime(FsSet("300", "10"), null, WeightUnit.LB))
        assertFalse(bench.copy(bodyweight = true).beatsLastTime(FsSet("", "50"), best, WeightUnit.LB))
    }

    @Test
    fun aLegacyDraftsBlankRowsAreDroppedOnRestore() {
        val draft = FreestyleDraft(
            openedAtMs = 0,
            exercises = listOf(
                FreestyleDraftExercise(
                    libId = "custom-bench",
                    sets = listOf(FreestyleDraftSet("100", "8"), FreestyleDraftSet("", "")),
                    name = "Bench",
                    muscleCode = MuscleGroup.CHEST.code
                )
            ),
            unitLabel = WeightUnit.LB.label
        )
        val restored = draftToItems(draft, WeightUnit.LB).single()
        assertEquals(listOf(FsSet("100", "8")), restored.sets)
    }

    @Test
    fun readingsSpellOutLoadRepsAndHolds() {
        assertEquals("60 kg × 8", bench.setReading(FsSet("60", "8"), "kg"))
        assertEquals("BW × 12", bench.copy(bodyweight = true).setReading(FsSet("", "12"), "kg"))
        val hold = bench.copy(timed = true, bodyweight = true)
        assertEquals("1:30", hold.setReading(FsSet(hold = "90"), "kg"))
    }
}
