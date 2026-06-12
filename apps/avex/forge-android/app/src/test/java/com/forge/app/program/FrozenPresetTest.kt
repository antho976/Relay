package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for the curated "Developer's preset" — Antho's MWM-989 pool, frozen
 * ([ExerciseLibrary.DEVELOPER_FROZEN_IDS]). The freeze must (a) reference only real movements,
 * (b) contain nothing his gear can't do, and (c) fully wall the preset off from the rest of the
 * library: generation, swaps and the available-pool all draw ONLY from the frozen ids, yet still
 * cover every muscle so plans are never starved. This is what lets the equipment generalization
 * grow the library without ever changing what the preset produces.
 */
class FrozenPresetTest {

    private val frozen = ExerciseLibrary.DEVELOPER_FROZEN_IDS

    /** His real gear: dumbbells + a FLAT bench + the MWM-989 cable + machine stations. */
    private val mwmGear = setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.CABLE, Equipment.MACHINE)

    @Test
    fun everyFrozenIdResolvesToARealMovement() {
        val missing = frozen.filter { ExerciseLibrary.byId(it) == null }
        assertTrue("frozen ids with no library entry (typos?): $missing", missing.isEmpty())
    }

    @Test
    fun frozenPoolHasNoInclineOrPullUpBarMovements() {
        // The MWM-989 bench is flat and this preset has no pull-up bar (Antho's call) — those
        // movements must never be in the frozen pool.
        val offenders = frozen.mapNotNull { ExerciseLibrary.byId(it) }.filter { def ->
            Equipment.INCLINE_BENCH in def.equipment || Equipment.PULL_UP_BAR in def.equipment
        }
        assertTrue("frozen pool leaks incline/pull-up movements: ${offenders.map { it.id }}",
            offenders.isEmpty())
    }

    @Test
    fun everyFrozenMovementIsDoableOnHisGear() {
        // Snapshot guard that survives future library growth: the freeze may DIVERGE from
        // "everything his gear can do" (new movements won't be added to it), but it must never
        // contain a movement his gear can't perform.
        val undoable = frozen.mapNotNull { ExerciseLibrary.byId(it) }
            .filterNot { ExerciseLibrary.isAvailable(it, mwmGear) }
        assertTrue("frozen pool contains movements his gear can't do: ${undoable.map { it.id }}",
            undoable.isEmpty())
    }

    @Test
    fun availablePoolWithFreezeIsExactlyTheFrozenSet() {
        val pool = ExerciseLibrary.availablePool(emptySet(), frozen).map { it.id }.toSet()
        assertEquals(frozen, pool)
    }

    @Test
    fun developerPresetIsWiredToTheFrozenPool() {
        val dev = equipmentPresets.first { it.id == "developer" }
        assertEquals(frozen, dev.frozenIds)
        assertEquals(mwmGear.map { it.name }.toSet(), dev.equipment)
        // Every other preset is plain equipment filtering — no freeze.
        assertTrue("only the developer preset is curated",
            equipmentPresets.filter { it.id != "developer" }.all { it.frozenIds == null })
    }

    @Test
    fun generationWithFreezeOnlyEverPicksFrozenMovements() {
        (3..6).forEach { d ->
            (0 until 10).forEach { s ->
                val outside = ProgramGenerator.generate(
                    GenerationParams(d, frozenIds = frozen), mwmGear, emptySet(), emptySet(), seed = s.toLong()
                ).flatMap { it.exercises }.map { it.libId }.filter { it !in frozen }
                assertTrue("$d-day seed $s picked non-frozen movements: $outside", outside.isEmpty())
            }
        }
    }

    @Test
    fun frozenGenerationStillCoversEveryMuscle() {
        // Walling the preset off must not starve it — every muscle still trained on every split.
        (3..6).forEach { d ->
            listOf(1L, 2L, 3L).forEach { seed ->
                val muscles = ProgramGenerator.generate(
                    GenerationParams(d, frozenIds = frozen), mwmGear, emptySet(), emptySet(), seed = seed
                ).flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
                assertEquals("$d-day frozen plan (seed $seed) should cover every muscle",
                    MuscleGroup.entries.toSet(), muscles)
            }
        }
    }

    @Test
    fun swapCandidatesWithFreezeStayInsideThePool() {
        MuscleGroup.entries.forEach { m ->
            val candidates = ExerciseLibrary.swapCandidates(m, mwmGear, emptySet(), frozen)
            assertTrue("swap candidates for $m leaked outside the frozen pool",
                candidates.all { it.id in frozen })
        }
    }
}
