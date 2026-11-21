package com.forge.app.domain.notify

/**
 * Pure decision for the "you've hit a PR milestone" notification (engagement thread). Kept free of
 * Android types like [TrainingReminder] so the round-number rule + the "only on the session that
 * crossed it" guard stay unit-tested.
 */
object PrMilestone {

    /** Lifetime personal-record counts worth celebrating. Ascending. */
    val MILESTONES = listOf(10, 25, 50, 100, 250, 500, 1000)

    data class Nudge(val title: String, val body: String)

    /**
     * A milestone notification when this session pushed the user's lifetime PR count across one of
     * [MILESTONES] — i.e. some milestone M with `lifetimePrCount - sessionPrCount < M <= lifetimePrCount`.
     * Returns the HIGHEST milestone crossed (a single session rarely clears more than one), or null
     * when the session set no PRs or crossed no milestone.
     *
     * @param lifetimePrCount total PRs after this session is counted.
     * @param sessionPrCount PRs set in this session.
     */
    fun check(lifetimePrCount: Int, sessionPrCount: Int): Nudge? {
        if (sessionPrCount <= 0) return null
        val before = lifetimePrCount - sessionPrCount
        val crossed = MILESTONES.lastOrNull { it in (before + 1)..lifetimePrCount } ?: return null
        return Nudge(
            // DESIGN §11: no exclamation marks, no em dashes, no praise ungrounded in data. The
            // count is the fact; "the work is showing" was an opinion the app cannot support.
            title = "$crossed personal records",
            body = "That's $crossed all-time PRs logged in Avex."
        )
    }
}
