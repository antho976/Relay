package com.forge.app.ui.gym.freestyle

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The two load-bearing pieces of "create a custom exercise from a search that found nothing":
 * the id must be stable across creations (or a repeat move's sets stop grouping with the earlier
 * ones in history/PRs/stats), and the draft must carry the custom's own name/muscle (there is no
 * library row to re-derive them from when the log resumes).
 */
@RunWith(RobolectricTestRunner::class)
class CustomExerciseTest {

    @Test
    fun sameNameAlwaysYieldsTheSameId() {
        assertEquals(customExerciseId("Sled Push"), customExerciseId("  sled   push "))
        assertEquals("custom-sled-push", customExerciseId("Sled Push"))
    }

    @Test
    fun idIsSluggedAndNeverBlank() {
        assertEquals("custom-atlas-stone-over-bar", customExerciseId("Atlas Stone (over bar)"))
        assertEquals("custom-exercise", customExerciseId("!!!"))
        assertTrue(customExerciseId("Ski Erg").length <= "custom-".length + 40)
    }

    @Test
    fun customIdsAreDistinguishableFromLibraryAndProgramIds() {
        assertTrue(isCustomExerciseId(customExerciseId("Sled Push")))
        assertFalse(isCustomExerciseId("db-bench-press"))
        // The program editor's own custom ids use an underscore and live in a different table.
        assertFalse(isCustomExerciseId("custom_12ab"))
        assertFalse(isCustomExerciseId("ext-sled-push"))
    }

    @Test
    fun draftRoundTripKeepsCustomIdentity() {
        val draft = FreestyleDraft(
            openedAtMs = 1_000L,
            exercises = listOf(
                FreestyleDraftExercise("db-bench-press", listOf(FreestyleDraftSet("135", "8"))),
                FreestyleDraftExercise(
                    libId = customExerciseId("Sled Push"),
                    sets = listOf(FreestyleDraftSet("90", "10")),
                    name = "Sled Push",
                    muscleCode = "quads"
                )
            )
        )
        val back = FreestyleDraft.fromJson(draft.toJson())!!
        assertEquals(draft, back)
        // A library move stays as compact as before — no identity fields written for it.
        assertNull(back.exercises[0].name)
        assertNull(back.exercises[0].muscleCode)
    }

    @Test
    fun aV2DraftStillLoads() {
        // v3 only ADDED optional fields, so an in-progress log written by the previous build must
        // survive the upgrade rather than being discarded as a stale schema.
        val v2 = """{"schema":2,"openedAtMs":7,"exercises":[{"libId":"db-bench-press",""" +
            """"sets":[{"w":"135","r":"8"}]}]}"""
        val back = FreestyleDraft.fromJson(v2)!!
        assertEquals(7L, back.openedAtMs)
        assertEquals("db-bench-press", back.exercises.single().libId)
        assertNull(back.exercises.single().name)
    }

    @Test
    fun aStaleSchemaIsStillRejected() {
        assertNull(FreestyleDraft.fromJson("""{"schema":1,"openedAtMs":7,"exercises":[]}"""))
    }
}
