package com.forge.app.ui.profile.state

import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.SignatureLift
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

    // Signature
    val topLift: SignatureLift? = null,
    val mostLoggedDay: String? = null,
    val usualHour: String? = null,

    // Goals (all set goals, achieved-first then closest-first; the Profile previews the top few)
    val goals: List<GoalRepository.GoalProgress> = emptyList(),
    // Custom goals (cardio / sessions / volume / bodyweight, auto-tracked) — previewed in the same box.
    val customGoals: List<ExtendedGoalRepository.Progress> = emptyList(),

    // The mirror test (progress photos)
    val photos: List<ProgressPhoto> = emptyList(),

    // Avatar (app-private file)
    val hasAvatar: Boolean = false,
    val avatarStamp: Long = 0L,

    // Trophy case
    val trophyUnlocked: Int = 0,
    val trophyTotal: Int = 0,
    val trophyGrid: List<TrophyCell> = emptyList(),
    val closestTrophy: String? = null,

    // Memory
    val memory: OnThisDayMemory? = null,

    // All-time cardio totals
    val cardioSessions: Int = 0,
    val cardioMinutes: Int = 0,
    val cardioDistanceKm: Double = 0.0
)
