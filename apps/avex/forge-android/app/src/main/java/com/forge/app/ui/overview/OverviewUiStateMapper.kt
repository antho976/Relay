package com.forge.app.ui.overview

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.Program
import com.forge.app.domain.notify.Milestones
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

// The ids and the lines themselves live in [Milestones]: a fired milestone waits in the
// notifications feed, which has to rebuild its line from the id alone (DESIGN §11).

internal fun buildOverviewUiState(
    stats: StatsRepository.WeeklyStats,
    recentCardio: List<CardioEntry>,
    shown: Set<String>,
    memory: OnThisDayMemory?,
    trophiesUnlocked: Int,
    distanceKm: Double,
    dayVolStats: Map<String, SessionDao.DayVolumeStats>,
    cardioTargetMin: Int = 0,
    weightUnit: WeightUnit = WeightUnit.LB,
    useMiles: Boolean = false
): OverviewUiState {
    val gymItems = stats.recentGymSessions.map { session ->
        val day = Program.days.firstOrNull { it.key == session.dayKey }
        val durationMin = session.durationMinutes()
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
            title = Program.dayDisplayName(session.dayKey),
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
        // Distance renders in the row's right column (where gym volume sits), not the subtitle.
        val sub = "${entry.durationMin} min"
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
        .take(3)
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
        bestSessionThisWeekLb = stats.bestSessionThisWeekLb,
        pendingMilestone = computePendingMilestone(stats, shown, weightUnit),
        onThisDayMemory = memory,
        nextUpDayKey = stats.nextUpDayKey,
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
    shown: Set<String>,
    weightUnit: WeightUnit
): MilestoneEvent? {
    if (stats.totalFinishedSessions >= 100 && Milestones.SESSIONS_100 !in shown) {
        return MilestoneEvent(
            Milestones.SESSIONS_100,
            Milestones.messageFor(Milestones.SESSIONS_100, weightUnit).orEmpty()
        )
    }
    if (stats.volumeLb >= Milestones.VOLUME_THRESHOLD_LB && Milestones.VOLUME_10K_WEEK !in shown) {
        // Threshold stays in lb (the stored unit) so it fires consistently; the label honours the
        // user's unit — "10k lb" / "4.5k kg" — instead of always reading lb.
        // The line itself comes from the catalogue so the feed can rebuild it from the id later.
        return MilestoneEvent(
            Milestones.VOLUME_10K_WEEK,
            Milestones.messageFor(Milestones.VOLUME_10K_WEEK, weightUnit).orEmpty()
        )
    }
    val firstMs = stats.firstFinishedSessionMs
    if (firstMs != null && Milestones.FIRST_FULL_MONTH !in shown) {
        val zone = ZoneId.systemDefault()
        val firstMonth = YearMonth.from(Instant.ofEpochMilli(firstMs).atZone(zone))
        if (firstMonth < YearMonth.now(zone)) {
            return MilestoneEvent(
                Milestones.FIRST_FULL_MONTH,
                Milestones.messageFor(Milestones.FIRST_FULL_MONTH, weightUnit).orEmpty()
            )
        }
    }
    return null
}
