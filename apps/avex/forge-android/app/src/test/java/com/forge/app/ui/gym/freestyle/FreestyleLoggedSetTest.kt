package com.forge.app.ui.gym.freestyle

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The one "does this set count" predicate behind the freestyle logger's session total, its Save
 * gate and each card's `N SETS` line. The card used to count positive reps only, so a valid
 * 45-second plank hold enabled Save and raised the session total while its own card said 0 SETS:
 * a timed hold legitimately stores zero reps.
 */
class FreestyleLoggedSetTest {

    @Test
    fun aTimedHoldCountsByItsHoldTimeNotItsReps() {
        assertTrue(isLoggedFreestyleSet(timed = true, reps = "", hold = "0:45"))
        assertTrue(isLoggedFreestyleSet(timed = true, reps = "0", hold = "45"))
        assertFalse(isLoggedFreestyleSet(timed = true, reps = "8", hold = ""))
        assertFalse(isLoggedFreestyleSet(timed = true, reps = "", hold = "0:00"))
    }

    @Test
    fun aRepSetCountsByPositiveRepsOnly() {
        assertTrue(isLoggedFreestyleSet(timed = false, reps = "8", hold = ""))
        assertFalse(isLoggedFreestyleSet(timed = false, reps = "0", hold = ""))
        assertFalse(isLoggedFreestyleSet(timed = false, reps = "", hold = "0:45"))
        assertFalse(isLoggedFreestyleSet(timed = false, reps = "x", hold = ""))
    }
}
