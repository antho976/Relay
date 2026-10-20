package com.quietsoftware.relay.ui.threads

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Avex's Show more offers what one more read brings, and stops where the PC's read does. */
class WorkoutsPagingTest {
    @Test
    fun aPageAtATimeUpToWhatThePcReadsAtOnce() {
        assertEquals(60, moreWorkouts(shown = 60, listed = 60, total = 900))
        assertEquals(15, moreWorkouts(shown = 120, listed = 120, total = 135))
        // 480 shown: the last 20 the PC reads at once, then nothing more to offer.
        assertEquals(20, moreWorkouts(shown = 480, listed = 480, total = 900))
        assertNull(moreWorkouts(shown = 500, listed = 500, total = 900))
        assertNull(moreWorkouts(shown = 60, listed = 42, total = 42))
    }
}
