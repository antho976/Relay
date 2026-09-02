package com.forge.app.ui.gym.train

import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.forgeHapticConstant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The rest timer's haptic cues, played once per threshold by the one owner that honours Feedback
 * strength. Before this, the screen and the bubble each requested both cues (two buzzes per moment)
 * and the bubble's copy skipped the setting entirely, so Off still buzzed.
 */
class RestTimerHapticCuesTest {

    /** Feed a full countdown and collect every cue, the way DayScreen's effect does. */
    private fun playRest(cues: RestTimerHapticCues, from: Int, strength: String): List<Int> {
        val played = mutableListOf<Int>()
        fun step(seconds: Int?, finished: Boolean) {
            cues.advance(seconds, finished).forEach { cue ->
                forgeHapticConstant(cue, strength)?.let { played += it }
            }
        }
        for (s in from downTo 1) step(s, finished = false)
        step(0, finished = true)
        return played
    }

    @Test
    fun aFullRestPlaysExactlyOneCuePerThreshold() {
        val cues = RestTimerHapticCues(secondsRemaining = null, finished = false)
        val events = mutableListOf<ForgeHapticType>()
        for (s in 30 downTo 1) events += cues.advance(s, finished = false)
        events += cues.advance(0, finished = true)
        assertEquals(listOf(ForgeHapticType.COUNTDOWN_TICK, ForgeHapticType.PR_OR_FINISH), events)
    }

    @Test
    fun offPlaysNothingAtEitherThreshold() {
        val cues = RestTimerHapticCues(secondsRemaining = null, finished = false)
        assertTrue(playRest(cues, from = 30, strength = "off").isEmpty())
    }

    @Test
    fun everyOtherStrengthPlaysOncePerThreshold() {
        for (strength in listOf("light", "medium", "strong")) {
            val cues = RestTimerHapticCues(secondsRemaining = null, finished = false)
            assertEquals("strength $strength", 2, playRest(cues, from = 30, strength = strength).size)
        }
    }

    @Test
    fun offResolvesToNoConstantForEveryCue() {
        ForgeHapticType.entries.forEach { assertNull(forgeHapticConstant(it, "off")) }
    }

    /** A tracker seeded mid-rest (rotation) or on a finished timer replays nothing already felt. */
    @Test
    fun seedingAtTheThresholdDoesNotReplayIt() {
        val atTen = RestTimerHapticCues(secondsRemaining = 10, finished = false)
        assertTrue(atTen.advance(10, finished = false).isEmpty())
        assertTrue(atTen.advance(9, finished = false).isEmpty())

        val finished = RestTimerHapticCues(secondsRemaining = 0, finished = true)
        assertTrue(finished.advance(0, finished = true).isEmpty())
    }

    @Test
    fun aRestThatStartsInsideTheWindowNeverWarns() {
        val cues = RestTimerHapticCues(secondsRemaining = null, finished = false)
        val events = mutableListOf<ForgeHapticType>()
        for (s in 8 downTo 1) events += cues.advance(s, finished = false)
        assertTrue(events.isEmpty())
    }

    @Test
    fun aSkippedTickStillCountsAsOneCrossing() {
        val cues = RestTimerHapticCues(secondsRemaining = 12, finished = false)
        assertEquals(listOf(ForgeHapticType.COUNTDOWN_TICK), cues.advance(9, finished = false))
        assertTrue("no second warning lower in the window", cues.advance(5, finished = false).isEmpty())
    }

    @Test
    fun addingTimeAndComingBackDownWarnsAgain() {
        val cues = RestTimerHapticCues(secondsRemaining = 11, finished = false)
        assertEquals(1, cues.advance(10, finished = false).size)
        assertTrue(cues.advance(40, finished = false).isEmpty()) // +30s
        for (s in 39 downTo 11) assertTrue(cues.advance(s, finished = false).isEmpty())
        assertEquals("a fresh crossing", listOf(ForgeHapticType.COUNTDOWN_TICK), cues.advance(10, finished = false))
    }

    @Test
    fun aNewRestAfterAFinishedOneCompletesAgain() {
        val cues = RestTimerHapticCues(secondsRemaining = 0, finished = true)
        assertTrue(cues.advance(null, finished = false).isEmpty())   // rest cleared
        assertTrue(cues.advance(60, finished = false).isEmpty())     // next rest starts
        assertEquals(listOf(ForgeHapticType.PR_OR_FINISH), cues.advance(0, finished = true))
    }
}
