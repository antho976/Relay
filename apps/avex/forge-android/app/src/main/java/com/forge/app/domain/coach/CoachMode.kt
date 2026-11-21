package com.forge.app.domain.coach

/**
 * How much the coach does without asking. Stored as the code, in `coach_mode`.
 *
 *  - [SUGGEST] (the default): every weekly change waits for a tap.
 *  - [AUTO]: a change TYPE applies itself once it has earned trust through accepted proposals
 *    ([TrustLedger]); everything else still waits.
 *  - [HANDS_OFF]: every change applies itself from the first week, and the brief becomes a report of
 *    what changed, each line with its Undo. In a session the coach's weight is already in the field.
 *
 * Hands-off was added on 2026-11-18, when Antho said he wanted the app to do the thinking and leave
 * him the training. It gives up the step where each change is approved first; it keeps everything
 * that made that step safe to give up: the reason on every change, the two-week watch on whether it
 * worked, and Undo. The one thing that still asks is a change that would throw away a workout in
 * progress.
 */
object CoachMode {
    const val SUGGEST = "suggest"
    const val AUTO = "auto"
    const val HANDS_OFF = "handsoff"

    /** Whether [mode] lets the weekly pass apply anything by itself. */
    fun appliesItself(mode: String): Boolean = mode == AUTO || mode == HANDS_OFF

    /** Whether [mode] fills the next set's weight with the coach's target instead of last session's. */
    fun fillsTargets(mode: String): Boolean = mode == HANDS_OFF
}
