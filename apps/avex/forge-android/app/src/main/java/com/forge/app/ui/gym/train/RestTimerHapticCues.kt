package com.forge.app.ui.gym.train

import com.forge.app.ui.common.ForgeHapticType

/**
 * Which rest-timer moments earn a haptic, as a pure state machine the screen feeds each reading.
 *
 * Two cues exist: the warning as the countdown drops into its final [WARNING_SECONDS], and the
 * "rest complete" pulse on the not-finished → finished edge. Each fires once per crossing. The
 * tracker is seeded with the reading it first sees, so a screen rebuilt mid-rest (rotation) or
 * opened onto an already-finished timer replays nothing the user already felt.
 *
 * It lives beside [DayScreen] because that is the ONE owner that reads the Feedback strength
 * setting and routes through `forgeHaptic`; the bubble that draws these moments no longer buzzes
 * them, which is what stopped Off from being ignored and every cue from playing twice.
 */
class RestTimerHapticCues(secondsRemaining: Int?, finished: Boolean) {

    private var prevSeconds: Int? = secondsRemaining
    private var prevFinished: Boolean = finished

    /**
     * Record the latest reading and return the cues THIS transition earns, in the order to play
     * them. Usually empty. [secondsRemaining] is null when no rest is running.
     */
    fun advance(secondsRemaining: Int?, finished: Boolean): List<ForgeHapticType> {
        val cues = mutableListOf<ForgeHapticType>()
        val prev = prevSeconds
        // Descending INTO the warning window, never on a rest that simply starts inside it, and a
        // skipped tick (12 → 9 after a doze) still counts as one crossing.
        if (!finished && secondsRemaining != null && prev != null &&
            prev > WARNING_SECONDS && secondsRemaining in 1..WARNING_SECONDS
        ) {
            cues += ForgeHapticType.COUNTDOWN_TICK
        }
        if (finished && !prevFinished) cues += ForgeHapticType.PR_OR_FINISH
        prevSeconds = secondsRemaining
        prevFinished = finished
        return cues
    }

    companion object {
        /** The countdown's warning threshold, in seconds. */
        const val WARNING_SECONDS = 10
    }
}
