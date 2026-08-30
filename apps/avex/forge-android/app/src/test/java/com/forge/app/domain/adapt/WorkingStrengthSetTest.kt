package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The working-strength contract, which [LoggedSet.durationSeconds]'s own documentation has always
 * stated and which only the SQL layer enforced:
 *
 *   > When set, `reps` is not a meaningful count and this set is excluded from every weight×reps
 *   > aggregate (volume, e1RM, PR) so it can't pollute strength stats.
 *
 * The Kotlin side filtered weight and assistance and nothing else, so a 45 lb × 90-second weighted
 * plank arrived at Epley as forty-five pounds for ninety reps — an estimated 180 lb single, higher
 * than any real set of the movement, feeding progression, plateau detection, the deload score and
 * the coach's goals for as long as it stayed in the window.
 */
class WorkingStrengthSetTest {

    private fun set(
        weightLb: Double? = 100.0,
        reps: Int = 5,
        assisted: Boolean = false,
        durationSeconds: Int? = null
    ) = LoggedSet(
        loggedExerciseId = 1L, setIndex = 0, weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb, reps = reps, completedAt = 0L,
        isAssisted = assisted, durationSeconds = durationSeconds
    )

    @Test
    fun `a plain weighted set is a working set`() {
        assertTrue(set().isWorkingStrengthSet())
    }

    @Test
    fun `bodyweight and assisted sets are not`() {
        assertFalse(set(weightLb = null).isWorkingStrengthSet())
        assertFalse(set(assisted = true).isWorkingStrengthSet())
    }

    @Test
    fun `a timed hold is not, however it is weighted`() {
        assertFalse(set(weightLb = 45.0, reps = 90, durationSeconds = 90).isWorkingStrengthSet())
    }

    @Test
    fun `a weighted plank does not become the best e1RM`() {
        val realWork = set(weightLb = 135.0, reps = 5)          // Epley: 157.5
        val plank = set(weightLb = 45.0, reps = 90, durationSeconds = 90) // Epley would say 180
        val best = bestWorkingE1rm(listOf(realWork, plank))!!
        assertEquals(E1rm.epley(135.0, 5), best, 0.001)
    }

    @Test
    fun `a session of nothing but holds has no working e1RM at all`() {
        val holds = listOf(
            set(weightLb = 45.0, reps = 60, durationSeconds = 60),
            set(weightLb = 45.0, reps = 90, durationSeconds = 90)
        )
        assertEquals(null, bestWorkingE1rm(holds))
    }

    @Test
    fun `workingStrengthSets keeps order and drops only what it must`() {
        val a = set(weightLb = 100.0, reps = 5)
        val hold = set(weightLb = 45.0, reps = 90, durationSeconds = 90)
        val b = set(weightLb = 105.0, reps = 5)
        assertEquals(listOf(a, b), listOf(a, hold, b).workingStrengthSets())
    }

    @Test
    fun `isRepSet is the weaker claim and keeps bodyweight work`() {
        // Consumers that read `reps` without caring about load — the deload rep drop-off, the
        // profile's rep histogram — must not lose push-ups, which have no weight and ten real reps.
        assertTrue(set(weightLb = null, reps = 10).isRepSet())
        assertFalse(set(assisted = true).isRepSet())
        assertFalse(set(weightLb = 45.0, reps = 90, durationSeconds = 90).isRepSet())
    }
}
