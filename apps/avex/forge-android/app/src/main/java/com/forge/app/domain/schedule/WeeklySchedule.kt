package com.forge.app.domain.schedule

/**
 * Resolves which program day is "next up" — either by a fixed weekly schedule (each weekday maps to
 * a program day or a rest slot) or by the legacy sequence model (the day after the last one you
 * finished, cycling). Pure so all three consumers — the day list, the Overview/StatsRepository
 * feed, and the home-screen widget — share ONE decision instead of three drifting copies.
 *
 * The schedule is stored as a comma-joined string of [SLOTS] entries, index 0 = Monday … 6 = Sunday,
 * an empty entry meaning "rest". Weekday mode is calendar-driven: each day reads its own weekday
 * slot fresh, so a missed Monday is simply skipped on Tuesday — there is no catch-up.
 */
object WeeklySchedule {

    const val MODE_SEQUENCE = "sequence"
    const val MODE_WEEKDAY = "weekday"
    const val SLOTS = 7

    /** Parse the stored schedule string into exactly [SLOTS] slots ("" = rest), padded/truncated. */
    fun parse(stored: String): List<String> {
        val parts = if (stored.isEmpty()) emptyList() else stored.split(",")
        return (0 until SLOTS).map { parts.getOrElse(it) { "" }.trim() }
    }

    /** Encode [slots] back to the stored string (always [SLOTS] comma-separated entries). */
    fun encode(slots: List<String>): String =
        (0 until SLOTS).joinToString(",") { slots.getOrElse(it) { "" } }

    /** A friendly default when the user hasn't set a schedule yet: program days on the first weekdays. */
    fun defaultFor(dayKeys: List<String>): List<String> =
        (0 until SLOTS).map { dayKeys.getOrElse(it) { "" } }

    /**
     * The "next up" program day, or null when there is no resolvable day (empty program). In weekday
     * mode the schedule is consulted first; if it has nothing upcoming (e.g. only rest slots remain),
     * it falls back to the sequence model so the user is never left without a suggestion.
     *
     * @param todayIndex 0 = Monday … 6 = Sunday.
     * @param trainedTodayKeys day keys already finished today — today's slot is skipped if it's one,
     *   so after you train Monday's session the app rolls forward to the next scheduled day.
     */
    fun resolveNextUp(
        mode: String,
        todayIndex: Int,
        schedule: List<String>,
        dayKeys: List<String>,
        lastFinishedDayKey: String?,
        trainedTodayKeys: Set<String>
    ): String? {
        if (dayKeys.isEmpty()) return null
        if (mode == MODE_WEEKDAY) {
            nextScheduled(todayIndex, schedule, dayKeys.toSet(), trainedTodayKeys)?.let { return it }
        }
        return sequenceNextUp(dayKeys, lastFinishedDayKey)
    }

    /** Scan today → +6 days for the first scheduled, in-program, not-already-done-today workout. */
    private fun nextScheduled(
        todayIndex: Int,
        schedule: List<String>,
        validKeys: Set<String>,
        trainedTodayKeys: Set<String>
    ): String? {
        for (i in 0 until SLOTS) {
            val weekday = (todayIndex + i) % SLOTS
            val key = schedule.getOrElse(weekday) { "" }
            if (key.isBlank() || key !in validKeys) continue
            if (i == 0 && key in trainedTodayKeys) continue
            return key
        }
        return null
    }

    /** Legacy model: the day after the last one finished, cycling; first day if none/stale. */
    private fun sequenceNextUp(dayKeys: List<String>, lastFinishedDayKey: String?): String {
        if (lastFinishedDayKey == null) return dayKeys.first()
        val idx = dayKeys.indexOf(lastFinishedDayKey)
        return if (idx < 0) dayKeys.first() else dayKeys[(idx + 1) % dayKeys.size]
    }
}
