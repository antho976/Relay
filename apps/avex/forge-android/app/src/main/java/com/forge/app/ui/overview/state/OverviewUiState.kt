package com.forge.app.ui.overview.state

/** One-shot milestone toast (#56). Shown once and persisted so it never re-fires. */
data class MilestoneEvent(val id: String, val message: String)

/** A session from N months ago on or near today's date (#106). */
data class OnThisDayMemory(
    val monthsAgo: Int,
    val dayName: String,
    val totalVolumeLb: Double,
    val prCount: Int,
    val sessionDate: Long
)

data class OverviewUiState(
    val workoutsThisWeek: Int = 0,
    val volumeThisWeekLb: Double = 0.0,
    val cardioMinutesThisWeek: Int = 0,
    val totalFinishedSessions: Int = 0,
    val lastDeloadAtSessionCount: Int = 0,
    /** Consecutive training days (streak). 0 = no active streak. */
    val streakDays: Int = 0,
    /** Calendar days since last finished session. null = never trained. */
    val daysSinceLastSession: Int? = null,
    /** Pending one-shot milestone to show as a snackbar (#56). Null when none. */
    val pendingMilestone: MilestoneEvent? = null,
    /** "On this day" memory card (#106). Null when no qualifying session found. */
    val onThisDayMemory: OnThisDayMemory? = null,
    /** Planned next training day set via "Plan tomorrow" (#147). Empty = not planned. */
    val plannedNextDay: String = ""
) {
    val sessionsSinceLastDeload: Int
        get() = (totalFinishedSessions - lastDeloadAtSessionCount).coerceAtLeast(0)

    val needsDeload: Boolean
        get() = sessionsSinceLastDeload >= DELOAD_THRESHOLD

    /** Show comeback banner when user hasn't trained in 5+ days (#57). */
    val showComebackBanner: Boolean
        get() = (daysSinceLastSession ?: 0) >= 5

    /** Show rest warning when training 3+ consecutive days (#58). */
    val showConsecutiveWarning: Boolean
        get() = streakDays >= 3

    companion object {
        const val DELOAD_THRESHOLD: Int = 24
    }
}
