package com.forge.app.ui.profile.state

import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.TrophyCell
import com.forge.app.domain.rank.RankInfo
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.domain.rank.XpBreakdown
import com.forge.app.ui.overview.state.OnThisDayMemory

/**
 * The "You" hub state. Most fields are surfaced from data the app already computes; the rank/XP
 * ladder and the offline standing estimate are the new engine outputs (see `domain/rank`).
 */
data class ProfileUiState(
    val loading: Boolean = true,
    val name: String = "",
    val sinceLabel: String = "",

    // Rank + XP
    val rank: RankInfo? = null,
    val xp: XpBreakdown? = null,

    // All-time lifetime tallies
    val totalSessions: Int = 0,
    val totalVolumeLb: Double = 0.0,
    val totalPrs: Int = 0,
    val totalSets: Int = 0,
    val streakDays: Int = 0,
    val longestStreakDays: Int = 0,

    // This-week vs last-week tallies — the All-Time tiles' "vs last week" arrows.
    val workoutsThisWeek: Int = 0,
    val workoutsLastWeek: Int = 0,
    val setsThisWeek: Int = 0,
    val setsLastWeek: Int = 0,
    val prsThisWeek: Int = 0,
    val prsLastWeek: Int = 0,

    // Standing (estimated vs typical lifters, 90 days)
    val standings: List<StandingMetric> = emptyList(),

    // The mirror test (progress photos)
    val photos: List<ProgressPhoto> = emptyList(),

    // Avatar (app-private file)
    val hasAvatar: Boolean = false,
    val avatarStamp: Long = 0L,
    // Key of the provided default currently in use (GYMAP-22), null when it's the user's own photo —
    // lets the picker ring the active default. `showAvatarHint` = the one-time "tap to change" nudge.
    val avatarDefaultKey: String? = null,
    val showAvatarHint: Boolean = false,

    // Trophy case
    val trophyUnlocked: Int = 0,
    val trophyTotal: Int = 0,
    val trophyGrid: List<TrophyCell> = emptyList(),
    val closestTrophy: String? = null,

    // Memory
    val memory: OnThisDayMemory? = null,

    // Cumulative lifted volume (lb), one point per finished session, oldest → newest — the All-Time graph.
    val lifetimeVolumeSeriesLb: List<Double> = emptyList(),

    // Times trained each calendar day this year (gym + cardio), keyed by epoch-day — the THIS YEAR
    // consistency grid. Empty ⇒ no activity yet this year (the section hides).
    val activityByDay: Map<Long, Int> = emptyMap()
)
