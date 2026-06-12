package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for the generalized library (2026-06-11) — equipment beyond the owner's MWM-989 home gym.
 * Ordinary (non-curated) pools must exclude the owner-only plate-count stations, the new equipment
 * must be fully usable (every chip has movements; the commercial/home-barbell presets produce
 * complete plans), and only the owner's preset stays curated. The owner's frozen preset itself is
 * covered by [FrozenPresetTest].
 */
class GeneralizedLibraryTest {

    private fun gearOf(presetId: String): Set<Equipment> =
        equipmentPresets.first { it.id == presetId }.equipment
            .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()

    private val commercial = gearOf("commercial")
    private val homeBarbell = gearOf("home-barbell")

    @Test
    fun ordinaryPoolsNeverIncludeCuratedStations() {
        // No plate-count (owner-only) movement may appear in an equipment-filtered pool, even with
        // every piece of equipment selected.
        val pool = ExerciseLibrary.availablePool(commercial, null)
        assertTrue("commercial pool leaked a curated movement: ${pool.filter { it.curatedOnly }.map { it.id }}",
            pool.none { it.curatedOnly })
        assertTrue("commercial pool leaked a PLATES movement",
            pool.none { it.unit == ExerciseUnit.PLATES })
    }

    @Test
    fun everyEquipmentChipHasAtLeastOneMovement() {
        // An equipment value with no exercise would render a chip that generates a broken empty day.
        Equipment.entries.forEach { equip ->
            assertTrue("$equip has no exercise that uses it",
                ExerciseLibrary.all.any { equip in it.equipment })
        }
    }

    @Test
    fun commercialGymCoversEveryMuscleWithNoCuratedMovements() {
        (3..6).forEach { d ->
            listOf(1L, 7L, 13L).forEach { seed ->
                val picks = ProgramGenerator
                    .generate(GenerationParams(d), commercial, emptySet(), emptySet(), seed = seed)
                    .flatMap { it.exercises }
                val muscles = picks.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
                assertEquals("$d-day commercial plan (seed $seed) should cover every muscle",
                    MuscleGroup.entries.toSet(), muscles)
                assertTrue("$d-day commercial plan (seed $seed) used a curated/plate-count movement",
                    picks.all { ExerciseLibrary.byId(it.libId)?.curatedOnly == false })
            }
        }
    }

    @Test
    fun homeBarbellPlanUsesBarbellMovementsAndCoversEveryMuscle() {
        val defs = ProgramGenerator
            .generate(GenerationParams(4), homeBarbell, emptySet(), emptySet(), seed = 3L)
            .flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId) }
        assertEquals(MuscleGroup.entries.toSet(), defs.map { it.muscle }.toSet())
        assertTrue("a barbell home-gym plan should use at least one barbell lift",
            defs.any { Equipment.BARBELL in it.equipment })
    }

    @Test
    fun bodyweightOnlyCoversEveryMuscle() {
        // The skip-onboarding default generates a bodyweight program — it must never starve a muscle.
        (3..6).forEach { d ->
            val muscles = ProgramGenerator.generate(
                GenerationParams(d), setOf(Equipment.BODYWEIGHT_ONLY), emptySet(), emptySet(), seed = 9L
            ).flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
            assertEquals("$d-day bodyweight-only plan should cover every muscle",
                MuscleGroup.entries.toSet(), muscles)
        }
    }

    @Test
    fun newPresetsExistAndOnlyDeveloperIsCurated() {
        val ids = equipmentPresets.map { it.id }.toSet()
        assertTrue(ids.containsAll(
            setOf("developer", "commercial", "home-barbell", "db-bench", "dumbbells", "bodyweight")))
        assertEquals("only the developer preset is frozen/curated",
            listOf("developer"), equipmentPresets.filter { it.frozenIds != null }.map { it.id })
    }
}
