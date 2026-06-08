package com.forge.app.domain.timer

import com.forge.app.core.time.Clock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wall-clock rest-timer coverage (#41 / #16). Remaining time is derived from a [Clock] end
 * instant rather than by decrementing, so the math is verified by advancing a fake clock and
 * reading the value pause() freezes — no real delays, no flakiness. The tick coroutine only
 * refreshes the displayed value; we cancel its scope at the end of each test.
 */
class RestTimerControllerTest {

    private class FakeClock(var now: Long = 0L) : Clock {
        override fun nowMs(): Long = now
    }

    /** Unconfined scope: start()/resume() launch a ticker that suspends at delay(1000); we never await it. */
    private fun newScope() = CoroutineScope(Dispatchers.Unconfined)

    @Test
    fun startSetsFullRemainingAndRunning() {
        val scope = newScope()
        val c = RestTimerController(scope, FakeClock(0))
        c.start(150)
        val s = c.state.value!!
        assertEquals(150, s.totalSeconds)
        assertEquals(150, s.secondsRemaining)
        assertFalse(s.isPaused)
        scope.cancel()
    }

    @Test
    fun pauseReflectsWallClockElapsed() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        clock.now = 30_000
        c.pause()
        assertEquals(120, c.state.value!!.secondsRemaining)
        assertTrue(c.state.value!!.isPaused)
        scope.cancel()
    }

    @Test
    fun pausedTimerDoesNotCountDownWhileClockMoves() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        clock.now = 30_000
        c.pause() // 120 left, frozen
        clock.now = 90_000 // time passes while paused
        assertEquals(120, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun resumeContinuesFromFrozenValue() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        clock.now = 30_000
        c.pause() // 120
        clock.now = 90_000
        c.resume() // end instant rebuilt from 120s remaining
        clock.now = 100_000 // 10s into the resumed run
        c.pause()
        assertEquals(110, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun addSecondsWhileRunningExtendsRemaining() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        clock.now = 30_000 // 120 left
        c.addSeconds(30) // → 150 left
        c.pause()
        assertEquals(150, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun addSecondsWhilePausedResumesAndAdds() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(60)
        clock.now = 20_000 // 40 left
        c.pause()
        c.addSeconds(30) // 70, and un-paused
        assertFalse(c.state.value!!.isPaused)
        c.pause()
        assertEquals(70, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun resetRestoresTotalAndPauses() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        clock.now = 50_000
        c.reset()
        assertEquals(150, c.state.value!!.secondsRemaining)
        assertTrue(c.state.value!!.isPaused)
        scope.cancel()
    }

    @Test
    fun stopClearsState() {
        val scope = newScope()
        val c = RestTimerController(scope, FakeClock(0))
        c.start(150)
        c.stop()
        assertNull(c.state.value)
        scope.cancel()
    }

    @Test
    fun remainingNeverGoesNegative() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(60)
        clock.now = 90_000 // past the end
        c.pause()
        assertEquals(0, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun controlsAreNoOpsWhenNoTimerRunning() {
        val scope = newScope()
        val c = RestTimerController(scope, FakeClock(0))
        c.pause(); c.resume(); c.addSeconds(30); c.reset()
        assertNull(c.state.value)
        scope.cancel()
    }
}
