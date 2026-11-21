package com.forge.app.ui.gym.train.components

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** What the weight field opens on for the next set: last session's numbers, led by today's. */
class NextSetSeedTest {

    private fun set(lb: Double?, assisted: Boolean = false) = LoggedSet(
        loggedExerciseId = 1L, setIndex = 0, weightText = lb?.toString() ?: "BW",
        weightLb = lb, reps = 8, completedAt = 0L, isAssisted = assisted
    )

    @Test
    fun theFirstSetOpensOnLastSessionsFirstSet() {
        assertEquals(100.0, nextSetSeedLb(null, set(100.0), null)!!, 0.0)
    }

    @Test
    fun aJumpMadeTodayCarriesIntoTheNextSet() {
        // Last time: 100, 100. Today's set one went to 102.5, so set two opens on 102.5, not 100.
        assertEquals(102.5, nextSetSeedLb(set(102.5), set(100.0), set(100.0))!!, 0.0)
    }

    @Test
    fun aPyramidKeepsItsShapeFromTodaysStart() {
        // Last time: 100 then 110. Today's set one was 105, so set two opens on 115.
        assertEquals(115.0, nextSetSeedLb(set(105.0), set(110.0), set(100.0))!!, 0.0)
    }

    @Test
    fun moreSetsThanLastTimeRepeatTodaysWeight() {
        assertEquals(102.5, nextSetSeedLb(set(102.5), null, set(100.0))!!, 0.0)
    }

    @Test
    fun anAssistedOrBodyweightSetTodayDoesNotLead() {
        assertEquals(100.0, nextSetSeedLb(set(150.0, assisted = true), set(100.0), set(100.0))!!, 0.0)
        assertEquals(100.0, nextSetSeedLb(set(null), set(100.0), set(100.0))!!, 0.0)
        assertNull(nextSetSeedLb(null, set(null), null))
    }

    @Test
    fun easingBackTheFirstSetOpensOnTheCoachesTarget() {
        // Last time (before the break) 100; the coach eases to 85, so set one opens on 85.
        assertEquals(85.0, nextSetSeedLb(null, set(100.0), null, rampLb = 85.0)!!, 0.0)
        // ...and set two follows today's set one, as always.
        assertEquals(85.0, nextSetSeedLb(set(85.0), set(100.0), set(100.0), rampLb = 85.0)!!, 0.0)
    }

    @Test
    fun theRampNeverRaisesALighterPriorSet() {
        // A pyramid's lighter first set stays where it was.
        assertEquals(60.0, nextSetSeedLb(null, set(60.0), null, rampLb = 85.0)!!, 0.0)
        // With no prior set at all, the target is the seed.
        assertEquals(85.0, nextSetSeedLb(null, null, null, rampLb = 85.0)!!, 0.0)
    }
}
