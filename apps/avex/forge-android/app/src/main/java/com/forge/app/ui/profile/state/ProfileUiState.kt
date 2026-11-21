package com.forge.app.ui.profile.state

import com.forge.app.data.repo.ProgressPhoto

/**
 * The Profile page's state: who you are, what you have done all time, the days you trained, and
 * your progress photos. Ranks and trophies left the page in the 2026-10-21 redesign, so their
 * readings are no longer carried here.
 */
data class ProfileUiState(
    val loading: Boolean = true,
    val name: String = "",
    val sinceLabel: String = "",

    // All-time lifetime tallies
    val totalSessions: Int = 0,
    val totalVolumeLb: Double = 0.0,
    val totalPrs: Int = 0,
    val totalSets: Int = 0,
    val streakDays: Int = 0,
    val longestStreakDays: Int = 0,

    // This-week tallies (and this month's PRs): the All time counts' change lines.
    val workoutsThisWeek: Int = 0,
    val workoutsLastWeek: Int = 0,
    val setsThisWeek: Int = 0,
    val setsLastWeek: Int = 0,
    val prsThisWeek: Int = 0,
    val prsLastWeek: Int = 0,
    val prsThisMonth: Int = 0,

    // Progress photos
    val photos: List<ProgressPhoto> = emptyList(),

    // Avatar (app-private file), drawn as the cover
    val hasAvatar: Boolean = false,
    val avatarStamp: Long = 0L,
    // Key of the provided default currently in use (GYMAP-22), null when it's the user's own photo —
    // lets the picker ring the active default. `showAvatarHint` = the one-time "tap to change" nudge.
    val avatarDefaultKey: String? = null,
    val showAvatarHint: Boolean = false,

    // Cumulative lifted volume (lb), one point per finished session, oldest → newest, and when each
    // point was lifted (epoch ms, same order) — the All time curve.
    val lifetimeVolumeSeriesLb: List<Double> = emptyList(),
    val lifetimeVolumeTimesMs: List<Long> = emptyList(),

    // Times trained each calendar day over all of history (gym + cardio), keyed by epoch-day, and
    // the days a session was logged as a deload — the Consistency month grid.
    val activityByDay: Map<Long, Int> = emptyMap(),
    val deloadDays: Set<Long> = emptySet()
)
