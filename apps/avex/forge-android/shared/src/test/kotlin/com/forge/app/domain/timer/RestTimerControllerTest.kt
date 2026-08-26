package com.forge.app.domain.timer

import com.forge.app.core.time.Clock
import com.forge.app.core.time.ElapsedClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Rest-timer coverage (#41 / #16). Remaining time is derived from an [ElapsedClock] end instant
 * rather than by decrementing, so the math is verified by advancing fake time and reading the value
 * pause() freezes — no real delays, no flakiness. The tick coroutine only refreshes the displayed
 * value; we cancel its scope at the end of each test.
 */
class RestTimerControllerTest {

    /**
     * Fake elapsed time. `now` is kept as a settable property so the arithmetic in these tests reads
     * the way it always did — but note it is now MONOTONIC elapsed time, not the wall clock. That
     * distinction is the point: a wall-clock correction moves a value the timer no longer reads.
     */
    private class FakeClock(var now: Long = 0L) : ElapsedClock {
        override fun elapsedMs(): Long = now
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
    fun pauseReflectsElapsedTime() {
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

    /**
     * A wall-clock correction — NTP on a stale-clock boot, or the user setting the date — moves a
     * clock the timer does not read. Both directions are simply invisible now.
     *
     * The forward direction is the one that was broken: it was indistinguishable from time actually
     * passing, so it was consumed straight out of the remaining rest. A phone four minutes slow that
     * associated with gym Wi-Fi 20 seconds into a 2:30 rest buzzed "rest over" on the spot.
     */
    @Test
    fun wallClockCorrectionDoesNotAffectTheCountdown() {
        val scope = newScope()
        val wallClock = object : Clock { override fun nowMs(): Long = 0L }
        val elapsed = FakeClock(0)
        val c = RestTimerController(scope, elapsed)
        c.start(150)
        elapsed.now += 20_000            // 20 real seconds of rest
        // The wall clock leaps four minutes forward. Elapsed time did not move.
        assertEquals(0L, wallClock.nowMs())
        c.pause()
        assertEquals(130, c.state.value!!.secondsRemaining)
        scope.cancel()
    }

    @Test
    fun countdownStillReachesZeroOnRealElapsedTime() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)
        c.start(150)
        c.pause()
        c.resume()
        clock.now += 151_000 // 151s of real elapsed time
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
    // ── restore(): the process-death path ───────────────────────────────────────────────────────
    //
    // restore() had NO test at all, which mutation testing made visible: relaxing its guard from
    // `<= 0` to `< 0` left the whole suite green. That guard is what stops a stale "0 seconds left"
    // record — written the moment a timer expired, just before the process was killed — from
    // rebuilding itself as a live timer on next launch and immediately firing the rest-done buzz.

    @Test
    fun restoreRebuildsARunningTimerFromWhatWasLeft() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)

        c.restore(totalSeconds = 150, remainingSeconds = 40, paused = false)

        val s = c.state.value!!
        assertEquals(150, s.totalSeconds)
        assertEquals(40, s.secondsRemaining)
        assertFalse(s.isPaused)
        scope.cancel()
    }

    @Test
    fun restoreKeepsAPausedTimerFrozen() {
        val scope = newScope()
        val clock = FakeClock(0)
        val c = RestTimerController(scope, clock)

        c.restore(totalSeconds = 150, remainingSeconds = 40, paused = true)
        clock.now += 30_000

        assertEquals("a paused restore must not drain", 40, c.state.value!!.secondsRemaining)
        assertTrue(c.state.value!!.isPaused)
        scope.cancel()
    }

    @Test
    fun restoreRefusesATimerWithNothingLeftToRun() {
        // The boundary the mutation slipped past. Zero is not "a timer with no time left" — it is
        // an expired record, and restoring it would show a live 0:00 countdown and buzz on the
        // first tick, for a rest that finished before the app was killed.
        val scope = newScope()
        val c = RestTimerController(scope, FakeClock(0))

        c.restore(totalSeconds = 150, remainingSeconds = 0, paused = false)
        assertNull("zero remaining must not restore", c.state.value)

        c.restore(totalSeconds = 0, remainingSeconds = 40, paused = false)
        assertNull("zero total must not restore either", c.state.value)

        c.restore(totalSeconds = -5, remainingSeconds = -5, paused = false)
        assertNull("and neither must a negative record", c.state.value)
        scope.cancel()
    }

    @Test
    fun aRestoredRemainderNeverExceedsItsTotal() {
        // Guards the progress ring: remaining > total reads as past-full.
        val scope = newScope()
        val c = RestTimerController(scope, FakeClock(0))

        c.restore(totalSeconds = 60, remainingSeconds = 90, paused = true)

        val s = c.state.value!!
        assertTrue("total must absorb the larger remainder", s.totalSeconds >= s.secondsRemaining)
        scope.cancel()
    }

}
