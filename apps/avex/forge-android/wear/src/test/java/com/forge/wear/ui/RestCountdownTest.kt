package com.forge.wear.ui

import com.forge.shared.protocol.TimerStateDto
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The first tests in the :wear module.
 *
 * Until now the watch app shipped 1,700 lines with zero tests and `:wear:testDebugUnitTest` was
 * NO-SOURCE in every CI run — a green task that ran nothing, which is worse than a missing one
 * because the pipeline reports it as passing. The rest countdown is the right place to start: it is
 * the number the athlete stands watching between sets, and it is computed from a timestamp taken on
 * the phone's clock and compared against the watch's.
 */
class RestCountdownTest {

    /** 2:30 of rest, published by the phone at T=1_000_000 and ending at T=1_150_000. */
    private fun timer(
        endAtMs: Long = 1_150_000L,
        totalSeconds: Int = 150,
        paused: Boolean = false,
        pausedRemainingSeconds: Int = 0,
        publishedAtMs: Long = 1_000_000L
    ) = TimerStateDto(
        endAtMs = endAtMs,
        totalSeconds = totalSeconds,
        paused = paused,
        pausedRemainingSeconds = pausedRemainingSeconds,
        publishedAtMs = publishedAtMs
    )

    // ── The clock-skew rule ────────────────────────────────────────────────────────────────────

    @Test
    fun countsDownFromHowLongWasLeftWhenThePhonePublished() {
        // The watch received the payload at ITS 5_000 and is now at ITS 5_000: nothing has elapsed
        // locally, so the full 150 s remains — even though the watch's clock reads nothing like the
        // phone's 1_000_000.
        assertEquals(150, RestCountdown.remainingSeconds(timer(), nowMs = 5_000, receivedAtMs = 5_000))
    }

    @Test
    fun localElapsedTimeIsWhatDrainsTheCountdown() {
        assertEquals(140, RestCountdown.remainingSeconds(timer(), nowMs = 15_000, receivedAtMs = 5_000))
        assertEquals(60, RestCountdown.remainingSeconds(timer(), nowMs = 95_000, receivedAtMs = 5_000))
    }

    @Test
    fun clockSkewBetweenTheTwoDevicesDoesNotChangeTheReading() {
        // The regression publishedAtMs exists to prevent. Same phone payload, same elapsed local
        // time, but a watch clock an hour off in each direction. All three must agree — with the
        // raw-instant maths they would have read 150, 3750 and -3450.
        val hour = 3_600_000L
        val onTime = RestCountdown.remainingSeconds(timer(), nowMs = 1_000_000, receivedAtMs = 1_000_000)
        val slow = RestCountdown.remainingSeconds(timer(), nowMs = 1_000_000 - hour, receivedAtMs = 1_000_000 - hour)
        val fast = RestCountdown.remainingSeconds(timer(), nowMs = 1_000_000 + hour, receivedAtMs = 1_000_000 + hour)
        assertEquals(150, onTime)
        assertEquals(onTime, slow)
        assertEquals(onTime, fast)
    }

    @Test
    fun aPhoneTooOldToSendPublishedAtFallsBackToTheRawInstant() {
        // publishedAtMs = 0 means "this phone predates the fix". The old behaviour is exactly right
        // for it — reading endAtMs against the watch's own clock — and must keep working rather than
        // collapsing to zero.
        val legacy = timer(publishedAtMs = 0L)
        assertEquals(150, RestCountdown.remainingSeconds(legacy, nowMs = 1_000_000, receivedAtMs = 1_000_000))
        assertEquals(90, RestCountdown.remainingSeconds(legacy, nowMs = 1_060_000, receivedAtMs = 1_000_000))
    }

    // ── Rounding and floors ────────────────────────────────────────────────────────────────────

    @Test
    fun apartSecondReadsAsAWholeSecondRemaining() {
        // 200 ms left must show 0:01, not 0:00. Truncating shows 0:00 for most of the final second,
        // which on a wrist reads as a timer that has stopped working.
        val almostDone = timer(endAtMs = 1_000_200L)
        assertEquals(1, RestCountdown.remainingSeconds(almostDone, nowMs = 5_000, receivedAtMs = 5_000))
    }

    @Test
    fun anExactlyExpiredTimerReadsZero() {
        assertEquals(0, RestCountdown.remainingSeconds(timer(endAtMs = 1_000_000L), nowMs = 5_000, receivedAtMs = 5_000))
    }

    @Test
    fun anOverrunTimerFloorsAtZeroRatherThanGoingNegative() {
        // The watch keeps ticking after zero until the phone republishes. It must sit at 0:00, not
        // count upward into negative time.
        assertEquals(0, RestCountdown.remainingSeconds(timer(), nowMs = 300_000, receivedAtMs = 5_000))
    }

    // ── Paused ─────────────────────────────────────────────────────────────────────────────────

    @Test
    fun aPausedTimerHoldsTheValueFrozenAtPause() {
        // Local elapsed time is irrelevant while paused: the phone froze the remainder into the
        // payload, and reading the clocks instead would drain a timer that is not running.
        val paused = timer(paused = true, pausedRemainingSeconds = 42)
        assertEquals(42, RestCountdown.remainingSeconds(paused, nowMs = 5_000, receivedAtMs = 5_000))
        assertEquals(42, RestCountdown.remainingSeconds(paused, nowMs = 999_000, receivedAtMs = 5_000))
    }

    @Test
    fun aNegativePausedRemainderIsFlooredRatherThanRendered() {
        // The one input that can produce "-1:-05" on screen.
        val broken = timer(paused = true, pausedRemainingSeconds = -65)
        assertEquals(0, RestCountdown.remainingSeconds(broken, nowMs = 5_000, receivedAtMs = 5_000))
    }

    // ── The ring ───────────────────────────────────────────────────────────────────────────────

    @Test
    fun theRingIsFullAtTheStartAndEmptyAtZero() {
        assertEquals(1f, RestCountdown.ringProgress(150, 150), 0.0001f)
        assertEquals(0.5f, RestCountdown.ringProgress(75, 150), 0.0001f)
        assertEquals(0f, RestCountdown.ringProgress(0, 150), 0.0001f)
    }

    @Test
    fun anUnknownTotalDrawsAnEmptyRingRatherThanNaN() {
        // totalSeconds <= 0 means the phone has not said how long this rest is. Dividing would give
        // NaN or an infinity, and a NaN sweep angle paints nothing while logging nothing.
        assertEquals(0f, RestCountdown.ringProgress(30, 0), 0.0001f)
        assertEquals(0f, RestCountdown.ringProgress(30, -1), 0.0001f)
        assertTrue(RestCountdown.ringProgress(30, 0).isFinite())
    }

    @Test
    fun tappingPlus30BeforeThePhoneRepublishesCannotOverfillTheRing() {
        // +30 raises the remaining seconds immediately; totalSeconds only catches up on the phone's
        // next publish, so for a few hundred milliseconds the ratio genuinely exceeds 1.
        assertEquals(1f, RestCountdown.ringProgress(180, 150), 0.0001f)
    }

    // ── Formatting ─────────────────────────────────────────────────────────────────────────────

    @Test
    fun formatsAsMinutesAndZeroPaddedSeconds() {
        assertEquals("2:30", RestCountdown.formatMmSs(150))
        assertEquals("0:07", RestCountdown.formatMmSs(7))
        assertEquals("0:00", RestCountdown.formatMmSs(0))
        assertEquals("1:00", RestCountdown.formatMmSs(60))
        assertEquals("10:05", RestCountdown.formatMmSs(605))
    }

    @Test
    fun formattingIsIndependentOfTheDeviceLocale() {
        // "%d" through a locale with non-Latin digits (Arabic-Indic under ar-EG) would render a
        // countdown the layout was never measured for. Pinned here because the watch inherits the
        // paired phone's locale, so this is not a hypothetical device.
        val original = java.util.Locale.getDefault()
        try {
            java.util.Locale.setDefault(java.util.Locale.forLanguageTag("ar-EG"))
            assertEquals("2:30", RestCountdown.formatMmSs(150))
        } finally {
            java.util.Locale.setDefault(original)
        }
    }

    @Test
    fun aNegativeInputNeverRendersASignedFigure() {
        assertEquals("0:00", RestCountdown.formatMmSs(-65))
    }

    // ── The tick cadence (P-04) ────────────────────────────────────────────────────────────────

    @Test
    fun theTickLandsOnTheBoundaryTheFigureActuallyChangesAt() {
        // Received at the watch's 5_000 with 150 s left. 400 ms of local time have passed, so the
        // displayed figure (150) drops to 149 in 600 ms — not in a full second, and not now.
        assertEquals(600L, RestCountdown.msUntilNextTick(timer(), nowMs = 5_400L, receivedAtMs = 5_000L))
        assertEquals(1L, RestCountdown.msUntilNextTick(timer(), nowMs = 5_999L, receivedAtMs = 5_000L))
    }

    @Test
    fun aWholeSecondRemainingWaitsAFullSecondRatherThanNotAtAll() {
        // Exactly 149 s left: the figure reads 149 for another whole second.
        assertEquals(
            RestCountdown.TICK_MS,
            RestCountdown.msUntilNextTick(timer(), nowMs = 6_000L, receivedAtMs = 5_000L)
        )
    }

    @Test
    fun everyTickIsAtMostOneSecondAndAtLeastOne() {
        // The property that matters for battery and for correctness together: never faster than 1 Hz,
        // and never a zero-length sleep that would spin.
        (0..2_500 step 37).forEach { elapsed ->
            val wait = RestCountdown.msUntilNextTick(timer(), nowMs = 5_000L + elapsed, receivedAtMs = 5_000L)
            assertTrue("elapsed $elapsed gave $wait", wait in 1L..RestCountdown.TICK_MS)
        }
    }

    @Test
    fun aPausedOrFinishedTimerStillTicksSlowlyForTheRowUnderIt() {
        assertEquals(
            RestCountdown.TICK_MS,
            RestCountdown.msUntilNextTick(timer(paused = true, pausedRemainingSeconds = 40), 5_000L, 5_000L)
        )
        assertEquals(
            RestCountdown.TICK_MS,
            RestCountdown.msUntilNextTick(timer(), nowMs = 900_000L, receivedAtMs = 5_000L)
        )
    }
}
