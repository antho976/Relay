package com.forge.app.data.repo

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.HeatmapCell
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.WeekActivityRow
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

// Pure effort / consistency / activity aggregation helpers extracted from
// StatsRepository. No DAO or DI dependencies.

internal const val HEATMAP_DAYS = 49
internal const val HEATMAP_WINDOW_MS: Long = HEATMAP_DAYS.toLong() * 24 * 60 * 60 * 1000

/** Weekly session count that counts toward the consistency streak. */
private const val CONSISTENCY_TARGET = 3

internal fun buildHeatmap(timestamps: List<Long>): List<HeatmapCell> {
    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    val countsByDate: Map<LocalDate, Int> = timestamps
        .groupingBy { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
        .eachCount()
    // Order oldest → newest so the UI can lay it out as 7 rows × 7 cols
    return (HEATMAP_DAYS - 1 downTo 0).map { offset ->
        val date = today.minusDays(offset.toLong())
        HeatmapCell(date = date, count = countsByDate[date] ?: 0)
    }
}

/** Count of sets logged at each RPE value (only sets where RPE was recorded). */
internal fun buildRpeDistribution(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<RpeBucket> {
    return allSets.mapNotNull { it.rpe }
        .groupingBy { it }
        .eachCount()
        .map { (rpe, count) -> RpeBucket(rpe = rpe, count = count) }
        .sortedBy { it.rpe }
}

/** Average RPE per finished session, oldest → newest (only sessions that recorded RPE). */
internal fun buildAvgRpePerSession(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<Double> {
    return allSets
        .filter { it.rpe != null }
        .groupBy { it.sessionStartedAt }
        .toSortedMap()
        .map { (_, ss) -> ss.mapNotNull { it.rpe }.average() }
}

internal fun buildEffortDistribution(
    rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.EffortWithDate>
): List<WeeklyEffortCounts> {
    val zone = ZoneId.systemDefault()
    val grouped = rows.groupBy { row ->
        val date = Instant.ofEpochMilli(row.sessionDate).atZone(zone).toLocalDate()
        date.minusDays(date.dayOfWeek.value.toLong() - 1) // ISO week start Monday
    }
    return grouped.entries
        .sortedBy { it.key }
        .takeLast(8)
        .map { (weekStart, weekRows) ->
            val label = weekStart.toString().substring(5) // "MM-dd"
            WeeklyEffortCounts(
                weekLabel = label,
                easy = weekRows.count { it.difficulty == "EASY" },
                justRight = weekRows.count { it.difficulty == "JUST_RIGHT" },
                hard = weekRows.count { it.difficulty == "HARD" },
                brutal = weekRows.count { it.difficulty == "BRUTAL" }
            )
        }
}

/** Session count per ISO week for the last 12 weeks, oldest → newest. */
internal fun buildWeeklySessionCounts(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<Int> {
    val zone = ZoneId.systemDefault()
    val byWeek = allSets.map { it.sessionStartedAt }.distinct()
        .groupingBy {
            val d = Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
            d.minusDays(d.dayOfWeek.value.toLong() - 1)
        }
        .eachCount()
    val now = LocalDate.now(zone)
    val thisWeek = now.minusDays(now.dayOfWeek.value.toLong() - 1)
    return (11 downTo 0).map { i -> byWeek[thisWeek.minusWeeks(i.toLong())] ?: 0 }
}

/** Consecutive recent weeks (incl. an in-progress current week) hitting the session target. */
internal fun computeConsistencyStreak(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): Int {
    if (allSets.isEmpty()) return 0
    val zone = ZoneId.systemDefault()
    val sessionsPerWeek = allSets.map { it.sessionStartedAt }.distinct()
        .groupingBy {
            val d = Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
            d.minusDays(d.dayOfWeek.value.toLong() - 1) // ISO week start (Monday)
        }
        .eachCount()
    val now = LocalDate.now(zone)
    val thisWeek = now.minusDays(now.dayOfWeek.value.toLong() - 1)
    var streak = 0
    for (i in 0 until 52) {
        val w = thisWeek.minusWeeks(i.toLong())
        val count = sessionsPerWeek[w] ?: 0
        when {
            count >= CONSISTENCY_TARGET -> streak++
            i == 0 -> {} // current week may still be in progress — don't break the streak
            else -> return streak
        }
    }
    return streak
}

internal fun buildExerciseFrequency(
    rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExerciseFreqRow>
): List<ExerciseFrequency> {
    val maxSessions = rows.maxOfOrNull { it.sessionCount } ?: 1
    return rows.mapNotNull { row ->
        val name = Program.exercise(row.exerciseId)?.name ?: return@mapNotNull null
        ExerciseFrequency(
            exerciseId = row.exerciseId,
            exerciseName = name,
            sessionCount = row.sessionCount,
            outOf = maxSessions
        )
    }.sortedByDescending { it.sessionCount }.take(10)
}

// buildInsights moved to the adaptation engine (InsightEngine, System 4): the time-of-day,
// most-improved, and muscle-dominance rules live there with snapshot-wide gating, and the
// old volume-drop deload rule (#80) was superseded by DeloadAdvisor's multi-signal score.

internal fun buildWeekActivity(
    sessions: List<Session>,
    cardioEntries: List<CardioEntry>
): List<WeekActivityRow> {
    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val dayLabels = listOf("MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN")
    return (0..6).map { dow ->
        val date = isoWeekStart.plusDays(dow.toLong())
        val session = sessions.firstOrNull { s ->
            Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate() == date
        }
        val cardio = if (session == null) cardioEntries.firstOrNull { c ->
            Instant.ofEpochMilli(c.date).atZone(zone).toLocalDate() == date
        } else null
        val dayPlan = session?.let { s -> Program.days.firstOrNull { it.key == s.dayKey } }
        WeekActivityRow(
            dayOfWeek = dow,
            dayLabel = dayLabels[dow],
            sessionName = dayPlan?.defaultName,
            muscleWord = dayPlan?.word,
            durationMin = session?.finishedAt?.let { fin -> ((fin - session.startedAt) / 60_000).toInt() },
            setCount = session?.setCount ?: 0,
            hasPr = (session?.prCount ?: 0) > 0,
            cardioType = cardio?.type?.replaceFirstChar { it.uppercase() },
            cardioDurationMin = cardio?.durationMin,
            cardioDistanceKm = cardio?.distanceKm
        )
    }
}
