package com.forge.app.ui.gym.train

import com.forge.app.ui.common.ForgeHapticType

/**
 * Which set-log moments earn a cue, as a pure state machine the screen feeds each reading.
 *
 * A rise in the session's PR-set count is a PR ([ForgeHapticType.PR_OR_FINISH]: haptic, confetti,
 * announcement); otherwise a rise in its set count is a logged set ([ForgeHapticType.SET_LOGGED]).
 * The tracker arms on the first reading taken AFTER the session has loaded, so the sets a reopened
 * session brings back become the baseline instead of a celebration.
 *
 * It exists because the screen used to arm on its very first composition, which for a fresh
 * ViewModel is the loading state with no exercises: when a resumed session's sets landed, 0 → N
 * PR sets replayed the whole PR celebration on every reopen (audit 2026-09-26). Modelled on
 * [RestTimerHapticCues], which solved the same replay for the rest timer.
 */
class SetLogHapticCues {

    private var prevSets = UNARMED
    private var prevPrSets = UNARMED

    /**
     * Record the latest reading and return the cue THIS transition earns, or null. Readings taken
     * while [loading] are ignored and leave the tracker unarmed.
     */
    fun advance(loading: Boolean, totalSets: Int, totalPrSets: Int): ForgeHapticType? {
        if (loading) return null
        val cue = when {
            prevSets == UNARMED -> null
            totalPrSets > prevPrSets -> ForgeHapticType.PR_OR_FINISH
            totalSets > prevSets -> ForgeHapticType.SET_LOGGED
            else -> null
        }
        prevSets = totalSets
        prevPrSets = totalPrSets
        return cue
    }

    private companion object {
        const val UNARMED = -1
    }
}
