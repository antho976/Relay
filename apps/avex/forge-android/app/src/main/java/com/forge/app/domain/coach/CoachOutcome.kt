package com.forge.app.domain.coach

/**
 * Plain-English status of a decided coach change — makes the otherwise-invisible 14-day
 * [OutcomeWatcher] window legible: an applied change reads "still watching · ~N days left" until
 * the watcher rules, then "worked" / "didn't stick". Pure (caller passes nowMs); used by both the
 * Week Brief's applied rows and the Settings coach-history rows.
 */
object CoachOutcome {

    /** A short status to append to a decision row, or null when there's nothing to add (open proposal). */
    fun label(
        status: String,
        outcome: String,
        appliedAtMs: Long?,
        nowMs: Long,
        zone: java.time.ZoneId = java.time.ZoneId.systemDefault()
    ): String? = when (status) {
        "applied", "folded" -> when (outcome) {
            "ok" -> "worked"
            "failed" -> "didn't stick"
            else -> {
                // Counted the same way OutcomeWatcher closes the window — in calendar days. The
                // elapsed-ms form truncated, so this line said "~0 days left" for a whole week
                // while the verdict waited on the next weekly pass.
                val daysLeft = appliedAtMs?.let {
                    OutcomeWatcher.WINDOW_DAYS - java.time.temporal.ChronoUnit.DAYS.between(
                        java.time.Instant.ofEpochMilli(it).atZone(zone).toLocalDate(),
                        java.time.Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
                    ).toInt()
                }
                if (daysLeft != null && daysLeft > 0)
                    "still watching · ~$daysLeft day${if (daysLeft == 1) "" else "s"} left"
                else "still watching"
            }
        }
        "skipped" -> "you skipped this"
        "reverted" -> "you undid this"
        // Recorded while the coach was switched off — watched, never acted on (see ensureWeeklyPass).
        "shadow" -> "observed while paused"
        else -> null
    }
}
