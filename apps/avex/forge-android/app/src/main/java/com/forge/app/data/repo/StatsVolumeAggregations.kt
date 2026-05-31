package com.forge.app.data.repo

import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.DayTypeBreakdown
import com.forge.app.ui.gym.stats.state.DayTypeVolumeStats
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.RepRangeDist
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import com.forge.app.ui.gym.stats.state.VolumePoint

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

internal fun buildExerciseVolumeHistory(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): Map<String, List<VolumePoint>> {
    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapValues { (_, sets) ->
            sets.groupBy { it.sessionStartedAt }
                .map { (sessionDate, sessionSets) ->
                    VolumePoint(
                        sessionDate = sessionDate,
                        totalVolumeLb = sessionSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
                    )
                }
                .sortedBy { it.sessionDate }
        }
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

internal fun buildDayTypeBreakdown(rows: List<com.forge.app.data.db.dao.SessionDao.DayTypeStats>): List<DayTypeBreakdown> {
    return rows.mapNotNull { row ->
        val dayName = Program.days.firstOrNull { it.key == row.dayKey }?.defaultName ?: return@mapNotNull null
        DayTypeBreakdown(
            dayKey = row.dayKey,
            dayName = dayName,
            avgDurationMin = (row.avgDurationMin ?: 0.0).toInt(),
            prRate = row.prRate ?: 0.0,
            skipRate = 0.0, // would need logged exercise skip data — approximate as 0 for now
            sessionCount = row.sessionCount
        )
    }.sortedBy { it.dayKey }
}
