package com.forge.app.data.repo

import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.DayTypeVolumeStats
import com.forge.app.ui.gym.stats.state.LifetimeMetrics
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.RepRangeDist
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import com.forge.app.ui.gym.stats.state.WeeklyTonnage

// Pure volume / muscle / day-type aggregation helpers extracted from StatsRepository.
// No DAO or DI dependencies — they fold already-loaded projections into UI state.

internal fun buildVolumeByMuscle(
    sets: List<com.forge.app.data.db.projections.SetWithExerciseId>
): List<MuscleVolume> {
    val byMuscle = mutableMapOf<com.forge.app.program.MuscleGroup, Double>()
    sets.forEach { s ->
        val plan = Program.exercise(s.exerciseId) ?: return@forEach
        val volume = (s.weightLb ?: 0.0) * s.reps
        byMuscle.merge(plan.muscle, volume, Double::plus)
    }
    return byMuscle
        .map { (muscle, volume) -> MuscleVolume(muscle = muscle, volumeLb = volume) }
        .sortedByDescending { it.volumeLb }
}

/** Working sets per muscle group in the current rolling week (#volume landmarks). */
internal fun buildWeeklySetsByMuscle(
    sets: List<com.forge.app.data.db.projections.SetWithExerciseId>
): List<MuscleSetCount> {
    val byMuscle = mutableMapOf<com.forge.app.program.MuscleGroup, Int>()
    sets.forEach { s ->
        val plan = Program.exercise(s.exerciseId) ?: return@forEach
        byMuscle.merge(plan.muscle, 1, Int::plus)
    }
    return byMuscle.map { (m, n) -> MuscleSetCount(muscle = m, sets = n) }
        .sortedByDescending { it.sets }
}

/**
 * Sets-per-muscle for a SINGLE session's detail page — folds each exercise's set count onto the
 * muscle group its [Program.exercise] maps to. Exercises with no library match (stale/removed ids)
 * are skipped. Mirrors [buildWeeklySetsByMuscle] so the two read identically.
 *
 * @param perExercise (exerciseId, setCount) for each non-skipped exercise that logged sets.
 */
internal fun buildSessionMuscleSplit(
    perExercise: List<Pair<String, Int>>
): List<MuscleSetCount> {
    val byMuscle = mutableMapOf<com.forge.app.program.MuscleGroup, Int>()
    perExercise.forEach { (exerciseId, sets) ->
        val plan = Program.exercise(exerciseId) ?: return@forEach
        byMuscle.merge(plan.muscle, sets, Int::plus)
    }
    return byMuscle.map { (m, n) -> MuscleSetCount(muscle = m, sets = n) }
        .sortedByDescending { it.sets }
}

internal fun buildVolumeDeloadTrend(
    rows: List<com.forge.app.data.db.dao.SessionDao.SessionVolumeDeloadRow>
): List<VolumeDeloadPoint> {
    return rows
        .filter { it.totalVolumeLb != null && (it.totalVolumeLb ?: 0.0) > 0 }
        .takeLast(30)
        .map { row ->
            VolumeDeloadPoint(
                sessionDate = row.startedAt,
                dayKey = row.dayKey,
                totalVolumeLb = row.totalVolumeLb ?: 0.0,
                isDeload = row.deloadMarkedHere
            )
        }
}

/**
 * Weekly tonnage: per-session volume points bucketed into ISO weeks, oldest → newest.
 * A week reads as deload when any session in it was deload-marked.
 */
internal fun buildWeeklyTonnage(
    points: List<VolumeDeloadPoint>,
    zone: java.time.ZoneId = java.time.ZoneId.systemDefault(),
    maxWeeks: Int = 12
): List<WeeklyTonnage> {
    if (points.isEmpty()) return emptyList()
    return points
        .groupBy { p ->
            val d = java.time.Instant.ofEpochMilli(p.sessionDate).atZone(zone).toLocalDate()
            d.minusDays(d.dayOfWeek.value.toLong() - 1)
        }
        .entries
        .sortedBy { it.key }
        .takeLast(maxWeeks)
        .map { (weekStart, ps) ->
            WeeklyTonnage(
                weekLabel = weekStart.toString().substring(5), // "MM-dd"
                volumeLb = ps.sumOf { it.totalVolumeLb },
                isDeload = ps.any { it.isDeload }
            )
        }
}

/** Split all logged sets into strength (1–5) / hypertrophy (6–12) / endurance (13+). */
internal fun buildRepRangeDist(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): RepRangeDist? {
    if (allSets.isEmpty()) return null
    var s = 0; var h = 0; var e = 0
    allSets.forEach {
        when {
            it.reps <= 5 -> s++
            it.reps <= 12 -> h++
            else -> e++
        }
    }
    return RepRangeDist(strength = s, hypertrophy = h, endurance = e)
}

internal fun buildDayTypeBestVsAvg(
    rows: List<com.forge.app.data.db.dao.SessionDao.DayVolumeStats>
): List<DayTypeVolumeStats> {
    return rows.mapNotNull { row ->
        val dayName = Program.days.firstOrNull { it.key == row.dayKey }?.defaultName ?: return@mapNotNull null
        DayTypeVolumeStats(
            dayKey = row.dayKey,
            dayName = dayName,
            avgVolumeLb = row.avgVolume,
            maxVolumeLb = row.maxVolume,
            sessionCount = row.sessionCount
        )
    }.sortedBy { it.dayKey }
}

/**
 * Lifetime at-a-glance for the Stats Overview tiles: total training sessions (distinct session
 * starts that logged working sets), lifetime working tonnage, and the per-session averages. Pure —
 * folds the already-loaded working-set population, so it carries the same tracked/non-skipped/
 * unassisted definition as the rest of the screen. Null when nothing's been logged.
 */
internal fun buildLifetimeMetrics(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): LifetimeMetrics? {
    if (allSets.isEmpty()) return null
    val totalSessions = allSets.map { it.sessionStartedAt }.distinct().size
    val lifetimeVolume = allSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
    return LifetimeMetrics(
        lifetimeVolumeLb = lifetimeVolume,
        totalSessions = totalSessions,
        avgSessionVolumeLb = if (totalSessions > 0) lifetimeVolume / totalSessions else 0.0,
        avgSetCount = if (totalSessions > 0) allSets.size.toDouble() / totalSessions else 0.0
    )
}

