package com.forge.app.ui.common

import com.forge.app.program.ExerciseLibrary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The shared exercise-picker filter: hides owner-only stations + excluded ids, and matches on
 *  name or muscle. Reused by onboarding "make my own", the Program Editor, and the freestyle logger. */
class ExerciseLibraryPickerTest {

    @Test
    fun blankQueryReturnsEveryNonCuratedMovement() {
        val result = filterLibrary("", emptySet())
        assertEquals(ExerciseLibrary.all.count { !it.curatedOnly }, result.size)
        assertTrue("no owner-only plate-count station may surface", result.none { it.curatedOnly })
    }

    @Test
    fun matchesByNameCaseInsensitive() {
        val result = filterLibrary("BENCH", emptySet())
        assertTrue(result.any { it.id == "db-bench-press" })
        assertTrue(result.all { it.name.lowercase().contains("bench") || it.muscle.displayName.lowercase().contains("bench") })
    }

    @Test
    fun matchesByMuscle() {
        val result = filterLibrary("chest", emptySet())
        assertTrue(result.isNotEmpty())
        assertTrue(result.all {
            it.name.lowercase().contains("chest") || it.muscle.displayName.lowercase().contains("chest")
        })
    }

    @Test
    fun excludesAlreadyPresentIds() {
        val all = filterLibrary("", emptySet())
        val drop = all.first().id
        val result = filterLibrary("", setOf(drop))
        assertFalse(result.any { it.id == drop })
        assertEquals(all.size - 1, result.size)
    }
}
