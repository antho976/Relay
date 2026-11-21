package com.forge.app.program

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The custom-exercise registry: the JSON blob it persists as, and the resolvers it feeds. A saved
 * custom move used to lose its picked muscle the moment the logger closed, because the logged row
 * has no muscle column; the registry is where that muscle now lives, and [Program.exercise] is
 * where every muscle aggregation reads it from.
 *
 * Robolectric because the blob round-trips through `org.json`, which the unit-test stub jar leaves
 * unimplemented.
 */
@RunWith(RobolectricTestRunner::class)
// A PLAIN Application, not Avex's own — the same collision CoachRepositoryApplyTest documents.
// Robolectric instantiates the manifest's `ForgeApp` for every test method, and its `onCreate`
// launches `settingsRepository.customExercises.collect { CustomExerciseRegistry.setAll(it) }` on a
// background scope. DataStore is empty here, so that collector calls `setAll(emptyList())` at some
// unpredictable point INSIDE the test body — wiping the very process-global registry these tests
// register into. It landed between a `put` and the read below on CI run 286 and not on 285, at the
// same source: `expected:<GLUTES> but was:<null>`, a harness race reported as a registry defect.
@Config(application = android.app.Application::class)
class CustomExerciseRegistryTest {

    private val sled = CustomExerciseDef("custom-sled-push", "Sled Push", "quads")

    @Before
    fun reset() = CustomExerciseRegistry.clear()

    @After
    fun tearDown() = CustomExerciseRegistry.clear()

    @Test
    fun jsonRoundTripKeepsEveryDefinition() {
        val defs = listOf(sled, CustomExerciseDef("custom-exercise-1a2b3c4d", "!!!", "core"))
        assertEquals(defs, CustomExerciseDef.listFromJson(CustomExerciseDef.listToJson(defs)))
    }

    @Test
    fun corruptOrStaleBlobsReadAsEmpty() {
        assertTrue(CustomExerciseDef.listFromJson(null).isEmpty())
        assertTrue(CustomExerciseDef.listFromJson("").isEmpty())
        assertTrue(CustomExerciseDef.listFromJson("not json").isEmpty())
        assertTrue(CustomExerciseDef.listFromJson("""{"schema":99,"exercises":[]}""").isEmpty())
        // An entry missing any of its three fields is skipped, not turned into a half-identity.
        val partial = """{"schema":1,"exercises":[{"id":"custom-x","name":"X"},{"id":"custom-y","name":"Y","muscle":"back"}]}"""
        assertEquals(listOf(CustomExerciseDef("custom-y", "Y", "back")), CustomExerciseDef.listFromJson(partial))
    }

    @Test
    fun programResolvesARegisteredCustomMoveWithItsPickedMuscle() {
        assertNull("unknown until registered", Program.exercise(sled.id))
        CustomExerciseRegistry.put(sled)
        val plan = Program.exercise(sled.id)!!
        assertEquals("Sled Push", plan.name)
        assertEquals(MuscleGroup.QUADS, plan.muscle)
        assertEquals("Sled Push", Program.exerciseDisplayName(sled.id))
    }

    @Test
    fun theRegistryNeverShadowsALibraryOrProgramId() {
        val lib = ExerciseLibrary.all.first { !it.curatedOnly }
        CustomExerciseRegistry.put(CustomExerciseDef(lib.id, "Impostor", "core"))
        assertEquals(lib.name, Program.exercise(lib.id)!!.name)
        assertEquals(lib.muscle, Program.exercise(lib.id)!!.muscle)
    }

    @Test
    fun setAllReplacesAndPutOverwritesByIdAndAnUnparseableMuscleYieldsNoPlan() {
        CustomExerciseRegistry.setAll(listOf(sled))
        CustomExerciseRegistry.put(sled.copy(muscleCode = "glutes"))
        assertEquals(MuscleGroup.GLUTES, CustomExerciseRegistry.muscle(sled.id))
        CustomExerciseRegistry.setAll(listOf(CustomExerciseDef("custom-odd", "Odd", "not-a-muscle")))
        assertNull("replaced wholesale", CustomExerciseRegistry.get(sled.id))
        assertEquals("Odd", CustomExerciseRegistry.name("custom-odd"))
        assertNull("a muscle code that no longer parses cannot be folded onto a muscle", Program.exercise("custom-odd"))
    }
}
