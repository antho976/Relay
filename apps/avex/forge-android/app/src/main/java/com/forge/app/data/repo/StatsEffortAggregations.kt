package com.forge.app.data.repo

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.HeatmapCell
import com.forge.app.ui.gym.stats.state.InsightFlag
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.WeekActivityRow
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

// Pure effort / consistency / activity / insight aggregation helpers extracted from
// StatsRepository. No DAO or DI dependencies — buildInsights takes the current time as
// a parameter rather than reaching for a clock.

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

internal fun buildInsights(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    weekSets: List<com.forge.app.data.db.projections.SetWithExerciseId>,
    dayTypeRows: List<com.forge.app.data.db.dao.SessionDao.DayTypeStats>,
    nowMs: Long
): List<InsightFlag> {
    val insights = mutableListOf<InsightFlag>()
    // Best time-of-day (#41)
    val zone = ZoneId.systemDefault()
    val prsByHour = allSets.filter { it.weightLb != null }
        .groupBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(zone).hour }
    val bestHour = prsByHour.maxByOrNull { it.value.size }?.key
    if (bestHour != null) {
        val label = when {
            bestHour < 10 -> "morning"
            bestHour < 13 -> "late morning"
            bestHour < 17 -> "afternoon"
            else -> "evening"
        }
        insights.add(InsightFlag("⏰", "Best time to train", "You log the most sets in the $label (${bestHour}:00)."))
    }
    // Most improved exercise (#41): biggest % gain in max weight over last 3 months
    val threeMonthsAgo = nowMs - 90L * 24 * 3600 * 1000
    val recentByExercise = allSets.filter { it.weightLb != null && it.sessionStartedAt >= threeMonthsAgo }
        .groupBy { it.exerciseId }
    val mostImproved = recentByExercise.entries.mapNotNull { (exId, sets) ->
        // Reduce to one max-weight value per session (oldest→newest), then split at the midpoint.
        // Splitting raw set rows weighted the halves by how many sets each session had, not by time.
        val perSession = sets.groupBy { it.sessionStartedAt }
            .toSortedMap()
            .map { (_, ss) -> ss.maxOf { it.weightLb!! } }
        if (perSession.size < 2) return@mapNotNull null
        val mid = perSession.size / 2
        val first = perSession.take(mid).maxOrNull() ?: return@mapNotNull null
        val last = perSession.drop(mid).maxOrNull() ?: return@mapNotNull null
        if (first <= 0) return@mapNotNull null
        val pct = ((last - first) / first * 100).toInt()
        val name = Program.exercise(exId)?.name ?: return@mapNotNull null
        Triple(name, pct, last)
    }.maxByOrNull { it.second }
    if (mostImproved != null && mostImproved.second > 5) {
        insights.add(InsightFlag("📈", "Most improved", "${mostImproved.first} is up ~${mostImproved.second}% in 3 months."))
    }
    // Muscle balance: flag if one muscle group dominates weekly volume
    val weekVolumeByMuscle = weekSets.groupBy { Program.exercise(it.exerciseId)?.muscle?.displayName ?: "Other" }
        .mapValues { (_, sets) -> sets.sumOf { (it.weightLb ?: 0.0) * it.reps } }
    val totalWeekVol = weekVolumeByMuscle.values.sum()
    if (totalWeekVol > 0) {
        val dominant = weekVolumeByMuscle.maxByOrNull { it.value }
        if (dominant != null && dominant.value / totalWeekVol > 0.5) {
            insights.add(InsightFlag("⚖️", "Muscle balance", "${dominant.key} is over 50% of your weekly volume. Consider balancing."))
        }
    }
    // Volume drop deload suggestion (#80): compare the older 3 vs the newer 3 of the most recent
    // 6 *sessions*. (Was comparing raw set-timestamps without .distinct(), so a single full workout
    // collapsed the window and the banner fired almost always.)
    val volBySession = allSets
        .filter { it.weightLb != null }
        .groupBy { it.sessionStartedAt }
        .mapValues { (_, ss) -> ss.sumOf { (it.weightLb ?: 0.0) * it.reps } }
    val recent6 = volBySession.toSortedMap().entries.toList().takeLast(6)
    if (recent6.size == 6) {
        val firstHalfVol = recent6.take(3).sumOf { it.value }
        val secondHalfVol = recent6.drop(3).sumOf { it.value }
        if (firstHalfVol > 0 && secondHalfVol < firstHalfVol * 0.8) {
            insights.add(InsightFlag("💤", "Consider a deload", "Volume has dropped 20%+ recently. You might benefit from a recovery week."))
        }
    }
    return insights
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
