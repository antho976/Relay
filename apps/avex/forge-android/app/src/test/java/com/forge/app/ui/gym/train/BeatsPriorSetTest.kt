package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.ui.gym.train.state.beatsPriorSet
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The live "vs last session" duel: which logged sets count as beating their same-position set. */
class BeatsPriorSetTest {

    private fun set(weightLb: Double?, reps: Int) = LoggedSet(
        loggedExerciseId = 1L,
        setIndex = 0,
        weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb,
        reps = reps,
        completedAt = 0L
    )

    @Test
    fun aRepeatAtTheRoundedLoadOnScreenIsATieNotAWin() {
        // Last session stored 28.588 lb, the screen showed 28.6, and the user typed 28.6.
        assertFalse(beatsPriorSet(set(28.6, 8), set(28.587833424881211, 8)))
    }

    @Test
    fun theSameLoadWinsOnlyOnReps() {
        assertTrue(beatsPriorSet(set(28.6, 9), set(28.587833424881211, 8)))
        assertFalse(beatsPriorSet(set(28.58, 7), set(28.6, 8)))
    }

    @Test
    fun aRealJumpStillWins() {
        assertTrue(beatsPriorSet(set(102.5, 5), set(100.0, 5)))
    }

    @Test
    fun aLighterSetWinsOnVolumeAlone() {
        assertTrue(beatsPriorSet(set(90.0, 12), set(100.0, 8)))
        assertFalse(beatsPriorSet(set(90.0, 8), set(100.0, 8)))
    }
}
