package com.forge.app.ui.overview

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.repo.StatsRepository
import com.forge.app.program.Program
import com.forge.app.ui.overview.state.MilestoneEvent
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.overview.state.OverviewUiState
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale

// Pure mapping from the OverviewViewModel's source flows into OverviewUiState.
// Kept out of the ViewModel so the flow wiring there stays readable.

internal const val MILESTONE_SESSIONS_100 = "sessions_100"
internal const val MILESTONE_VOLUME_10K = "volume_10k_week"
internal const val MILESTONE_FIRST_MONTH = "first_full_month"

internal fun buildOverviewUiState(
    stats: StatsRepository.WeeklyStats,
    recentCardio: List<CardioEntry>,
    lastDeload: Int,
    shown: Set<String>,
    memory: OnThisDayMemory?,
    plannedDay: String,
    trophiesUnlocked: Int,
    distanceKm: Double,
    dayVolStats: Map<String, SessionDao.DayVolumeStats>
): OverviewUiState {
    val gymItems = stats.recentGymSessions.map { session ->
        val day = Program.days.firstOrNull { it.key == session.dayKey }
        val durationMin = session.finishedAt?.let { ((it - session.startedAt) / 60_000).toInt() }
        val exCount = day?.exercises?.size ?: 0
        val sub = listOfNotNull(
            if (exCount > 0) "$exCount ex" else null,
            durationMin?.let { "${it} min" }
        ).joinToString(" · ")
        val volStats = dayVolStats[session.dayKey]
        val vsAvgPct = if (volStats != null && session.totalVolumeLb != null && volStats.avgVolume > 0)
            (((session.totalVolumeLb - volStats.avgVolume) / volStats.avgVolume) * 100).toInt()
        else null
        val isBest = volStats != null && session.totalVolumeLb != null &&
            session.totalVolumeLb >= volStats.maxVolume
        Pair(session.startedAt, OverviewRecentItem(
            dayLabel = relativeDay(session.startedAt),
            title = day?.defaultName ?: session.dayKey,
            subtitle = sub,
            tag = day?.word ?: "",
            id = session.id,
            timestampMs = session.startedAt,
            isGym = true,
            volumeLb = session.totalVolumeLb,
            prCount = session.prCount,
            vsAvgPct = vsAvgPct,
            isBest = isBest,
            durationMin = durationMin
        ))
    }
    val cardioItems = recentCardio.map { entry ->
        val typeName = entry.type.replaceFirstChar { it.uppercase() }
        val sub = listOfNotNull(
            "${entry.durationMin} min",
            entry.distanceKm?.takeIf { it > 0 }?.let { "${it} km" }
        ).joinToString(" · ")
        Pair(entry.date, OverviewRecentItem(
            dayLabel = relativeDay(entry.date),
            title = "Cardio · $typeName",
            subtitle = sub,
            tag = "MOVE",
            id = entry.id,
            timestampMs = entry.date,
            isGym = false,
            durationMin = entry.durationMin,
            distanceKm = entry.distanceKm
        ))
    }
    val recentItems = (gymItems + cardioItems)
        .sortedByDescending { it.first }
        .take(2)
        .map { it.second }

    val zone2 = ZoneId.systemDefault()
    val todayLocal = LocalDate.now(zone2)
    val isoWeekStart = todayLocal.with(DayOfWeek.MONDAY)
    val cardioWeekDays = recentCardio.mapNotNull { entry ->
        val d = Instant.ofEpochMilli(entry.date).atZone(zone2).toLocalDate()
        if (!d.isBefore(isoWeekStart) && !d.isAfter(todayLocal)) d.dayOfWeek.value - 1 else null
    }.toSet()

    return OverviewUiState(
        workoutsThisWeek = stats.workouts,
        volumeThisWeekLb = stats.volumeLb,
        cardioMinutesThisWeek = stats.cardioMinutes,
        totalFinishedSessions = stats.totalFinishedSessions,
        lastDeloadAtSessionCount = lastDeload,
        streakDays = stats.streakDays,
        daysSinceLastSession = stats.daysSinceLastSession,
        pendingMilestone = computePendingMilestone(stats, shown),
        onThisDayMemory = memory,
        plannedNextDay = plannedDay,
        nextUpDayKey = stats.nextUpDayKey,
        weekDaysTrained = stats.weekDaysTrained,
        cardioWeekDays = cardioWeekDays,
        recentItems = recentItems,
        trophiesUnlocked = trophiesUnlocked,
        cardioDistanceKm = distanceKm
    )
}

private fun relativeDay(epochMs: Long): String {
    val zone = ZoneId.systemDefault()
    val date = Instant.ofEpochMilli(epochMs).atZone(zone).toLocalDate()
    val today = LocalDate.now(zone)
    return when (date) {
        today -> "TODAY"
        today.minusDays(1) -> "YESTERDAY"
        else -> date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()
    }
}

private fun computePendingMilestone(
    stats: StatsRepository.WeeklyStats,
    shown: Set<String>
): MilestoneEvent? {
    if (stats.totalFinishedSessions >= 100 && MILESTONE_SESSIONS_100 !in shown) {
        return MilestoneEvent(MILESTONE_SESSIONS_100, "100 workouts complete. You've earned this.")
    }
    if (stats.volumeLb >= 10_000.0 && MILESTONE_VOLUME_10K !in shown) {
        return MilestoneEvent(MILESTONE_VOLUME_10K, "10,000 lb this week. Volume beast.")
    }
    val firstMs = stats.firstFinishedSessionMs
    if (firstMs != null && MILESTONE_FIRST_MONTH !in shown) {
        val zone = ZoneId.systemDefault()
        val firstMonth = YearMonth.from(Instant.ofEpochMilli(firstMs).atZone(zone))
        if (firstMonth < YearMonth.now(zone)) {
            return MilestoneEvent(MILESTONE_FIRST_MONTH, "First full month of training. You're building something real.")
        }
    }
    return null
}
