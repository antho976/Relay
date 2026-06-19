package com.forge.app.ui.overview.state

import androidx.compose.runtime.Immutable

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
    /** A single status chip (gym only): DELOAD / TEST / TECHNIQUE / first quick tag. Empty = none. */
    val statusPill: String = "",
    /** Marquee lift — the session's heaviest set, e.g. "Bench Press 185 × 5". Filled in the VM. */
    val topLift: String? = null,
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

/**
 * Sub-gate "still learning" nudge (CD-1): shown only when the coach has nothing actionable yet
 * AND is below its activation session gate, so a quiet coach reads as "warming up", not absent.
 */
data class CoachLearningHint(val sessionsLogged: Int, val sessionsToGo: Int)

/**
 * Sub-threshold fatigue nudge (Tier 3): the coach is active but holding, and recovery signals are
 * building toward — but not yet at — a deload. Surfaces the otherwise-invisible System 5 pulse.
 */
data class FatigueHint(val score: Int, val threshold: Int, val topDriver: String?)

/**
 * "Recovery snapshot" (Health Connect): the user's recent resting HR is meaningfully above their
 * own baseline — a classic under-recovered tell. Shown as its own Overview card when present.
 */
data class RecoveryAlert(val restingBpm: Int, val baselineBpm: Int, val deltaBpm: Int)

@Immutable
data class OverviewUiState(
    val workoutsThisWeek: Int = 0,
    /** The user's chosen days/week — the "of N" denominator, NOT a hardcoded 6 (multi-user). */
    val weeklyWorkoutTarget: Int = 4,
    val volumeThisWeekLb: Double = 0.0,
    val cardioMinutesThisWeek: Int = 0,
    val cardioWeeklyTargetMin: Int = 0,
    val totalFinishedSessions: Int = 0,
    val streakDays: Int = 0,
    /** Days since the last finished session (vacation-aware); null when no sessions yet. Drives the comeback nudge. */
    val daysSinceLastSession: Int? = null,
    /** Highest single-session volume (lb) logged in the current ISO week; null/0 when none. */
    val bestSessionThisWeekLb: Double? = null,
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
    /** Combined gym + cardio, sorted newest first, capped at 3. */
    val recentItems: List<OverviewRecentItem> = emptyList(),
    val trophiesUnlocked: Int = 0,
    /** "X to go · Trophy Name" for the locked trophy closest to unlocking, or null (#136 near-miss). */
    val topNearMiss: String? = null,
    val cardioDistanceKm: Double = 0.0,
    /** Day key of an in-progress (unfinished) workout, if any — drives the resume banner + CTA. */
    val activeSessionDayKey: String? = null,
    /**
     * Actionable adaptation-engine recommendations (deload, plateau ladder), arbitrated and
     * capped. Replaces the old fixed-counter deload / comeback / 3-days-straight flags.
     */
    val coach: List<CoachItem> = emptyList(),
    /** Non-null only when [coach] is empty and the coach hasn't activated yet (CD-1). */
    val coachLearning: CoachLearningHint? = null,
    /** Non-null only when [coach] is empty, the coach IS active, and fatigue is building (Tier 3). */
    val coachFatigue: FatigueHint? = null,
    /** Non-null when Health Connect resting HR is elevated vs the user's baseline — recovery card. */
    val recoveryAlert: RecoveryAlert? = null,
    /** True when a big cardio block was logged in the last ~36h — drives a "go easier today" nudge. */
    val heavyCardioRecent: Boolean = false
) {
    val hasActiveSession: Boolean get() = activeSessionDayKey != null
}
