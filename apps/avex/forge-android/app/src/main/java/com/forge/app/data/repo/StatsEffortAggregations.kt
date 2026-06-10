package com.forge.app.data.repo

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.adapt.InsightEngine
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.TrainingTimes
import com.forge.app.ui.gym.stats.state.WeekActivityRow
import com.forge.app.ui.gym.stats.state.WeeklyDuration
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

// Pure effort / consistency / activity aggregation helpers extracted from
// StatsRepository. No DAO or DI dependencies.

/** Weekly session count that counts toward the consistency streak. */
private const val CONSISTENCY_TARGET = 3

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
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    onVacation: (LocalDate) -> Boolean = { false }
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
            // A holiday week (every day on vacation) doesn't break the streak (#135).
            (0..6).all { onVacation(w.plusDays(it.toLong())) } -> {}
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

/**
 * "When you train": sessions per day of week + the sets-weighted best hour. The hour
 * label reuses InsightEngine's wording so the card and the insight can never disagree.
 */
internal fun buildTrainingTimes(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    zone: ZoneId = ZoneId.systemDefault(),
    minSetsForHour: Int = 30
): TrainingTimes? {
    if (allSets.isEmpty()) return null
    val dow = IntArray(7)
    allSets.map { it.sessionStartedAt }.distinct().forEach { ms ->
        dow[Instant.ofEpochMilli(ms).atZone(zone).dayOfWeek.value - 1]++
    }
    val bestHourLabel = if (allSets.size >= minSetsForHour) {
        allSets.groupingBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(zone).hour }
            .eachCount()
            .maxByOrNull { it.value }
            ?.let { (hour, _) -> "${InsightEngine.timeOfDayLabel(hour)} ($hour:00)" }
    } else null
    return TrainingTimes(sessionsByDayOfWeek = dow.toList(), bestHourLabel = bestHourLabel)
}

/** Median session length per ISO week (sane 10–240 min only), oldest → newest. */
internal fun buildWeeklyDurations(
    sessions: List<Session>,
    zone: ZoneId = ZoneId.systemDefault(),
    maxWeeks: Int = 12
): List<WeeklyDuration> {
    val byWeek = sessions
        .mapNotNull { s ->
            val fin = s.finishedAt ?: return@mapNotNull null
            val min = ((fin - s.startedAt) / 60_000).toInt().takeIf { it in 10..240 } ?: return@mapNotNull null
            val d = Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate()
            d.minusDays(d.dayOfWeek.value.toLong() - 1) to min
        }
        .groupBy({ it.first }, { it.second })
    return byWeek.entries
        .sortedBy { it.key }
        .takeLast(maxWeeks)
        .map { (weekStart, mins) ->
            WeeklyDuration(
                weekLabel = weekStart.toString().substring(5),
                medianMin = mins.sorted()[mins.size / 2]
            )
        }
}

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
