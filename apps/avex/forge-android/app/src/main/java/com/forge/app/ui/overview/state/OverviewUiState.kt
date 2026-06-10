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
    val dayLabel: String,
    val title: String,
    val subtitle: String,
    val tag: String,
    val id: Long = -1L,
    val timestampMs: Long = 0L,
    val isGym: Boolean = true,
    val volumeLb: Double? = null,
    val prCount: Int = 0,
    val vsAvgPct: Int? = null,
    val isBest: Boolean = false,
    val durationMin: Int? = null,
    val distanceKm: Double? = null,
)

/**
 * One actionable adaptation-engine recommendation on the Overview coach feed.
 * [applyLabel] non-null = a one-tap apply exists (e.g. "Generate deload week").
 */
data class CoachItem(
    val id: String,
    val title: String,
    val body: String,
    val applyLabel: String? = null
)

data class OverviewUiState(
    val workoutsThisWeek: Int = 0,
    val volumeThisWeekLb: Double = 0.0,
    val cardioMinutesThisWeek: Int = 0,
    val cardioWeeklyTargetMin: Int = 0,
    val totalFinishedSessions: Int = 0,
    val streakDays: Int = 0,
    val pendingMilestone: MilestoneEvent? = null,
    val onThisDayMemory: OnThisDayMemory? = null,
    val plannedNextDay: String = "",
    /** Next gym day in the rotation. */
    val nextUpDayKey: String = "upper-a",
    /** 0=Mon..6=Sun indices that had a gym session this ISO week. */
    val weekDaysTrained: Set<Int> = emptySet(),
    /** 0=Mon..6=Sun indices that had a cardio entry this ISO week. */
    val cardioWeekDays: Set<Int> = emptySet(),
    /** Custom name for the next-up day set by the user, or null if using the program default. */
    val customDayName: String? = null,
    /** Combined gym + cardio, sorted newest first, capped at 2. */
    val recentItems: List<OverviewRecentItem> = emptyList(),
    val trophiesUnlocked: Int = 0,
    val cardioDistanceKm: Double = 0.0,
    /** Day key of an in-progress (unfinished) workout, if any — drives the resume banner + CTA. */
    val activeSessionDayKey: String? = null,
    /**
     * Actionable adaptation-engine recommendations (deload, plateau ladder), arbitrated and
     * capped. Replaces the old fixed-counter deload / comeback / 3-days-straight flags.
     */
    val coach: List<CoachItem> = emptyList()
) {
    val hasActiveSession: Boolean get() = activeSessionDayKey != null
}
