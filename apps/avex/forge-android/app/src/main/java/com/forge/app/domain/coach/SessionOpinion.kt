package com.forge.app.domain.coach

/**
 * The coach's one-line read of a session you just finished — derived purely from that session's
 * own result (PRs, the duel vs last time, volume delta, plan completion). Kept synchronous and
 * dependency-free on purpose: it runs at the celebratory finish moment, so it must be instant and
 * can't block on the heavy weekly coach pass.
 *
 * Returns null for an empty session (nothing to opine on).
 */
object SessionOpinion {

    fun of(
        setCount: Int,
        prCount: Int,
        ghostBeats: Int,
        ghostComparable: Int,
        vsLastVolumeDelta: Double?,
        isBestSession: Boolean,
        honestyPct: Int?
    ): String? {
        if (setCount == 0) return null

        val cleanSweep = ghostComparable > 0 && ghostBeats == ghostComparable
        // Strict majority — an even split (e.g. 3 of 6) is a draw, not "edged out".
        val wonDuel = ghostComparable > 0 && ghostBeats * 2 > ghostComparable

        // Lead with the single strongest thing that happened, stated as the fact it is (§11): the
        // lines used to close on poster copy ("You found another gear.", "Momentum is building.",
        // "Consistency is what moves the needle."), the cliches the voice rules name.
        val lead = when {
            isBestSession -> "The most volume you've moved on this day."
            prCount > 0 -> "$prCount new ${if (prCount == 1) "PR" else "PRs"} today."
            cleanSweep -> "Clean sweep. You beat last session on every comparable set."
            wonDuel -> "You edged out last session on $ghostBeats of $ghostComparable sets."
            vsLastVolumeDelta != null && vsLastVolumeDelta > 0 -> "More volume than last time."
            vsLastVolumeDelta != null && vsLastVolumeDelta < 0 ->
                "A touch lighter than last time. Fine if it was planned, otherwise watch recovery."
            vsLastVolumeDelta != null -> "The same volume as last time."
            else -> "First time through this day: these numbers are the baseline."
        }

        // At most one caveat, and it states the share rather than scolding.
        val caveat = if (honestyPct != null && honestyPct < 60)
            " You logged $honestyPct% of the planned sets." else ""

        return lead + caveat
    }
}
