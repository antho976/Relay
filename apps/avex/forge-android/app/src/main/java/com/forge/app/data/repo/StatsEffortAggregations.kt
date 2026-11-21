package com.forge.app.data.repo

import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.adapt.InsightEngine
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.TrainingTimes
import com.forge.app.ui.gym.stats.state.WeeklyDuration
import java.time.Instant
import java.time.ZoneId
import kotlin.math.roundToInt

// Pure effort / consistency / activity aggregation helpers extracted from
// StatsRepository. No DAO or DI dependencies.

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
            // Active-time minutes via the shared reader, so this chart agrees with the session-detail
            // and history durations; keep only sane session lengths.
            val min = (s.durationMinutes() ?: return@mapNotNull null).takeIf { it in 10..240 }
                ?: return@mapNotNull null
            // Bucket by startedAt — consistent with the weekly tonnage and training-times trend charts
            // (the dot-grid is a separate current-week widget and deliberately uses finishedAt).
            val d = Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate()
            d.minusDays(d.dayOfWeek.value.toLong() - 1) to min
        }
        .groupBy({ it.first }, { it.second })
    return byWeek.entries
        .sortedBy { it.key }
        .takeLast(maxWeeks)
        .map { (weekStart, mins) ->
            val sorted = mins.sorted()
            val mid = sorted.size / 2
            // True median: average the two middle elements on even-sized weeks, rounded (Int field) —
            // was the upper element, which over-reported.
            val median = if (sorted.size % 2 == 0)
                ((sorted[mid - 1] + sorted[mid]) / 2.0).roundToInt()
            else sorted[mid]
            WeeklyDuration(
                weekLabel = weekStart.toString().substring(5),
                medianMin = median
            )
        }
}
