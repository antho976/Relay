package com.forge.app.domain.coach

/**
 * Plain-English status of a decided coach change — makes the otherwise-invisible 14-day
 * [OutcomeWatcher] window legible: an applied change reads "still watching · ~N days left" until
 * the watcher rules, then "worked" / "didn't stick". Pure (caller passes nowMs); used by both the
 * Week Brief's applied rows and the Settings coach-history rows.
 */
object CoachOutcome {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** A short status to append to a decision row, or null when there's nothing to add (open proposal). */
    fun label(status: String, outcome: String, appliedAtMs: Long?, nowMs: Long): String? = when (status) {
        "applied", "folded" -> when (outcome) {
            "ok" -> "worked"
            "failed" -> "didn't stick"
            else -> {
                val daysLeft = appliedAtMs?.let {
                    ((OutcomeWatcher.WINDOW_DAYS * DAY_MS - (nowMs - it)) / DAY_MS).toInt()
                }
                if (daysLeft != null && daysLeft > 0)
                    "still watching · ~$daysLeft day${if (daysLeft == 1) "" else "s"} left"
                else "still watching"
            }
        }
        "skipped" -> "you skipped this"
        "reverted" -> "you undid this"
        else -> null
    }
}
