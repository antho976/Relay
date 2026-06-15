package com.forge.app.ui.overview

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.session.SessionType
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
    shown: Set<String>,
    memory: OnThisDayMemory?,
    plannedDay: String,
    trophiesUnlocked: Int,
    distanceKm: Double,
    dayVolStats: Map<String, SessionDao.DayVolumeStats>,
    cardioTargetMin: Int = 0
): OverviewUiState {
    val gymItems = stats.recentGymSessions.map { session ->
        val day = Program.days.firstOrNull { it.key == session.dayKey }
        // Prefer real ACTIVE training time; fall back to wall-clock only when it wasn't recorded
        // (pre-feature sessions stamp 0) — mirrors the reader pattern documented on Session.
        val durationMin = if (session.activeSeconds > 0) session.activeSeconds / 60
            else session.finishedAt?.let { ((it - session.startedAt) / 60_000).toInt() }
        val exCount = day?.exercises?.size ?: 0
        val sub = listOfNotNull(
            if (exCount > 0) "$exCount ex" else null,
            if (session.setCount > 0) "${session.setCount} sets" else null,
            durationMin?.let { "${it} min" }
        ).joinToString(" · ")
        val volStats = dayVolStats[session.dayKey]
        // Compare against the average of the OTHER sessions for this day type, not an average that
        // includes this very session (which diluted the % and read 0% for a day's only session).
        val vsAvgPct = if (volStats != null && session.totalVolumeLb != null && volStats.sessionCount > 1) {
            val othersAvg = (volStats.avgVolume * volStats.sessionCount - session.totalVolumeLb) /
                (volStats.sessionCount - 1)
            if (othersAvg > 0) (((session.totalVolumeLb - othersAvg) / othersAvg) * 100).toInt() else null
        } else null
        val isBest = volStats != null && session.totalVolumeLb != null && volStats.sessionCount > 1 &&
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
            durationMin = durationMin,
            statusPill = sessionStatusPill(session)
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
        cardioWeeklyTargetMin = cardioTargetMin,
        totalFinishedSessions = stats.totalFinishedSessions,
        streakDays = stats.streakDays,
        pendingMilestone = computePendingMilestone(stats, shown),
        onThisDayMemory = memory,
        plannedNextDay = plannedDay,
        // A user-chosen "Train X today" overrides the rotation default until it's consumed when a
        // session starts. Ignore a stale key that isn't part of the current program.
        nextUpDayKey = plannedDay.takeIf { it.isNotBlank() && Program.dayKeys.contains(it) }
            ?: stats.nextUpDayKey,
        weekDaysTrained = stats.weekDaysTrained,
        cardioWeekDays = cardioWeekDays,
        recentItems = recentItems,
        trophiesUnlocked = trophiesUnlocked,
        cardioDistanceKm = distanceKm
    )
}

/**
 * One status chip for a finished session, or "" for a plain session. An explicit session-type
 * marker wins over the user's end-of-session quick tags (#107, #109); only the first tag is shown
 * so the row stays uncluttered.
 */
private fun sessionStatusPill(session: Session): String {
    // An explicit session-type marker (or a here-marked deload) wins over the user's quick tags.
    val type = if (session.deloadMarkedHere) SessionType.DELOAD else SessionType.fromKey(session.sessionType)
    type?.pillLabel?.let { return it }
    // Otherwise show only the first quick tag, so the row stays uncluttered (#107, #109).
    return session.tags.split(",").map { it.trim() }.firstOrNull { it.isNotEmpty() }?.uppercase().orEmpty()
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
