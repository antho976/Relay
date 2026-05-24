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

/** One row in the overview RECENT section — gym session or cardio entry. */
data class OverviewRecentItem(
    val dayLabel: String,   // "YESTERDAY", "MON", "SUN", etc.
    val title: String,      // "Lower A", "Cardio · Run"
    val subtitle: String,   // "6 ex · 45 min" or "20 min · 3 km"
    val tag: String         // "QUADS", "MOVE", "PUSH", etc.
)

data class OverviewUiState(
    val workoutsThisWeek: Int = 0,
    val volumeThisWeekLb: Double = 0.0,
    val cardioMinutesThisWeek: Int = 0,
    val totalFinishedSessions: Int = 0,
    val lastDeloadAtSessionCount: Int = 0,
    val streakDays: Int = 0,
    val daysSinceLastSession: Int? = null,
    val pendingMilestone: MilestoneEvent? = null,
    val onThisDayMemory: OnThisDayMemory? = null,
    val plannedNextDay: String = "",
    /** Next gym day in the rotation. */
    val nextUpDayKey: String = "upper-a",
    /** 0=Mon..6=Sun indices that had a gym session this ISO week. */
    val weekDaysTrained: Set<Int> = emptySet(),
    /** Combined gym + cardio, sorted newest first, capped at 2. */
    val recentItems: List<OverviewRecentItem> = emptyList(),
    val trophiesUnlocked: Int = 0,
    val cardioDistanceKm: Double = 0.0
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
