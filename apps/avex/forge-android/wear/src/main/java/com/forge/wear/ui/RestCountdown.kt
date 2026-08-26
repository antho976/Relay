package com.forge.wear.ui

import com.forge.shared.protocol.TimerStateDto
import java.util.Locale

/**
 * The rest countdown's arithmetic, kept out of [TimerView] so it can be tested at all.
 *
 * It looks like three lines of subtraction and it is the wrist's most safety-critical number: it is
 * what the athlete stands there watching between sets, and it is derived from a timestamp taken on
 * a DIFFERENT DEVICE's clock. Living inside a `@Composable` beside `System.currentTimeMillis()` and
 * a 200 ms polling loop, none of it could be checked without an emulator, so the clock-skew rule
 * below — the one thing here that is not obvious — had no test at all.
 */
internal object RestCountdown {

    /**
     * Seconds still to run, from the phone's timer state and two local instants.
     *
     * [nowMs] and [receivedAtMs] are both read from the WATCH's clock: the moment being rendered,
     * and the moment this timer instance first arrived. That pairing is the whole point.
     * `endAtMs` is an absolute instant on the PHONE's clock, so rendering it against the watch's
     * own turned every millisecond of skew between the two devices into a millisecond of error in
     * the countdown — silently, and worse the longer the two had been out of sync.
     * `publishedAtMs` lets the payload carry a DURATION instead, which both clocks agree on:
     *
     *     remaining = (endAtMs − publishedAtMs) − (watchNow − watchReceivedAt)
     *
     * A phone too old to send `publishedAtMs` leaves it 0, and we fall back to the raw instant —
     * which is exactly the pre-fix behaviour, so an un-upgraded phone is no worse off than before.
     */
    fun remainingSeconds(timer: TimerStateDto, nowMs: Long, receivedAtMs: Long): Int {
        // A paused timer's remaining time is frozen at pause and travels in the payload; the local
        // clocks say nothing about it. coerceAtLeast because a malformed payload would otherwise
        // render as "-1:-05" — the one formatting a negative can produce.
        if (timer.paused) return timer.pausedRemainingSeconds.coerceAtLeast(0)

        val remainingMs =
            if (timer.publishedAtMs > 0L) (timer.endAtMs - timer.publishedAtMs) - (nowMs - receivedAtMs)
            else timer.endAtMs - nowMs

        // Round UP: a timer with 200 ms left reads "0:01" until it is genuinely done. Truncating
        // instead shows 0:00 for most of the final second, which reads as a stuck timer.
        return ((remainingMs + 999) / 1000).toInt().coerceAtLeast(0)
    }

    /** Ring fill, 1.0 at the start of the rest down to 0.0 at zero. */
    fun ringProgress(remainingSeconds: Int, totalSeconds: Int): Float =
        // A zero or negative total means the phone has not told us how long this rest is; an empty
        // ring is honest, where dividing would be NaN and paint nothing at all.
        if (totalSeconds <= 0) 0f
        // Clamped because +30 raises the remaining time the instant it is tapped, while totalSeconds
        // only catches up on the phone's next publish — for those few hundred milliseconds the
        // ratio genuinely exceeds 1.
        else (remainingSeconds.toFloat() / totalSeconds).coerceIn(0f, 1f)

    /**
     * "2:30" / "0:07". Minutes are not zero-padded; seconds always are.
     *
     * Locale.US, not the device default, for the same reason the phone's CardioPace and
     * HoldFormatter pin it: `%d` renders through the locale's own digits, so on an Arabic-locale
     * watch this read "٢:٣٠" while the phone beside it — showing the SAME rest timer — read "2:30".
     * The watch has no translated strings of its own, so those digits appeared inside otherwise
     * English chrome, in a serif figure style measured for Latin numerals.
     */
    fun formatMmSs(totalSeconds: Int): String {
        val safe = totalSeconds.coerceAtLeast(0)
        return String.format(Locale.US, "%d:%02d", safe / 60, safe % 60)
    }
}
