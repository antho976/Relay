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
     * This form drops [NextUp.placement], so it answers "what would they do next" and NOT "what does
     * the schedule call for today". Anything deciding whether TODAY is a training day must use
     * [resolveNextUpWithOffset] and read the placement.
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
    ): String? = resolveNextUpWithOffset(
        mode, todayIndex, schedule, dayKeys, lastFinishedDayKey, trainedTodayKeys
    )?.dayKey

    /**
     * The "next up" program day, what it is relative to today, and how far off it is.
     *
     * [NextUp.placement] is the answer, not [NextUp.daysAhead]. Offset zero used to carry two
     * completely different meanings — "the schedule says train today" and "the schedule says
     * nothing, here is a guess" — and the Today directive could only read the first. So a weekday
     * schedule whose slots are all rest, or all naming days a regenerate has since removed, fell
     * through to the sequence guess at offset zero and was announced as today's scheduled training:
     * a workout the user's own schedule does not contain, on a day it marks as rest.
     */
    fun resolveNextUpWithOffset(
        mode: String,
        todayIndex: Int,
        schedule: List<String>,
        dayKeys: List<String>,
        lastFinishedDayKey: String?,
        trainedTodayKeys: Set<String>
    ): NextUp? {
        if (dayKeys.isEmpty()) return null
        if (mode == MODE_WEEKDAY) {
            nextScheduled(todayIndex, schedule, dayKeys.toSet(), trainedTodayKeys)?.let { return it }
            // Nothing in the whole week resolves. The sequence guess is still offered — a user is
            // never left without a suggestion — but it is labelled for what it is, so a caller that
            // needs a DATE can refuse it and one that only needs a suggestion can take it.
            return NextUp(
                sequenceNextUp(dayKeys, lastFinishedDayKey),
                daysAhead = 0,
                placement = Placement.UNSCHEDULED
            )
        }
        // Sequence mode has no calendar at all: "next" is simply what to do now.
        return NextUp(sequenceNextUp(dayKeys, lastFinishedDayKey), daysAhead = 0)
    }

    /**
     * The day a caller may open as TODAY'S session, or null when the schedule does not put one here.
     *
     * The one place that decision is made. Reading it off [NextUp.daysAhead] instead is what let an
     * unresolvable weekday schedule — every slot rest, or every slot naming a day the program no
     * longer has — arrive as today's scheduled training, because the sequence fallback carries the
     * same offset zero a genuinely scheduled session does.
     */
    fun trainTodayKey(resolved: NextUp?): String? =
        resolved?.dayKey?.takeIf { resolved.placement == Placement.TODAY }

    /**
     * The day to announce as coming up, or null when nothing is scheduled ahead. Only ever a dated
     * answer: a suggestion with no date behind it ([Placement.UNSCHEDULED]) is not "in N days".
     */
    fun upcomingKey(resolved: NextUp?): String? =
        resolved?.dayKey?.takeIf { resolved.placement == Placement.UPCOMING }

    /** What a resolved day is relative to today. */
    enum class Placement {
        /** The schedule (or the sequence model) puts this day now. */
        TODAY,
        /** A weekday schedule puts this day [NextUp.daysAhead] days from now; today is a rest day. */
        UPCOMING,
        /**
         * A weekday schedule resolves nothing at all this week — every slot rest, or every slot
         * naming a day the program no longer has. The day is a suggestion with no date behind it,
         * and must never be presented as today's scheduled training.
         */
        UNSCHEDULED
    }

    /** A resolved program day and its distance from today in calendar days (0 = today). */
    data class NextUp(
        val dayKey: String,
        val daysAhead: Int,
        val placement: Placement = Placement.TODAY
    )

    /** Scan today → +6 days for the first scheduled, in-program, not-already-done-today workout. */
    private fun nextScheduled(
        todayIndex: Int,
        schedule: List<String>,
        validKeys: Set<String>,
        trainedTodayKeys: Set<String>
    ): NextUp? {
        for (i in 0 until SLOTS) {
            val weekday = (todayIndex + i) % SLOTS
            val key = schedule.getOrElse(weekday) { "" }
            if (key.isBlank() || key !in validKeys) continue
            if (i == 0 && key in trainedTodayKeys) continue
            return NextUp(key, daysAhead = i, placement = if (i == 0) Placement.TODAY else Placement.UPCOMING)
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
