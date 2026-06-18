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

        // Lead with the single strongest thing that happened.
        val lead = when {
            isBestSession -> "Best session yet for this day — that's a clear step up."
            prCount > 0 -> "$prCount new ${if (prCount == 1) "PR" else "PRs"} — you found another gear today."
            cleanSweep -> "Clean sweep — you beat last session on every comparable set."
            wonDuel -> "You edged out last session on $ghostBeats of $ghostComparable sets."
            vsLastVolumeDelta != null && vsLastVolumeDelta > 0 -> "Volume's up on last time — momentum is building."
            vsLastVolumeDelta != null && vsLastVolumeDelta < 0 ->
                "A touch lighter than last time — fine if it was planned, otherwise watch recovery."
            else -> "Solid work in the bank — consistency is what moves the needle."
        }

        // At most one gentle, actionable caveat — keep the finish moment encouraging.
        val caveat = if (honestyPct != null && honestyPct < 60)
            " You left a chunk of the plan unlogged; close those sets next time if you can." else ""

        return lead + caveat
    }
}
