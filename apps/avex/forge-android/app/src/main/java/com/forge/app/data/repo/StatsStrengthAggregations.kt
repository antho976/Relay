package com.forge.app.data.repo

import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.ExerciseYoY
import com.forge.app.ui.gym.stats.state.HistoryPoint
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.RepMaxEntry
import com.forge.app.ui.gym.stats.state.RepMaxSet
import com.forge.app.ui.gym.stats.state.StrengthCurve
import com.forge.app.ui.gym.stats.state.TimeToPrEntry
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

// Pure strength / PR / e1RM aggregation helpers extracted from StatsRepository.
// These operate on already-loaded set projections — no DAO or DI dependencies — so
// they live as top-level functions the repository orchestrator calls.

private const val STRENGTH_CURVE_MAX_POINTS = 10

/** Compound lifts shown on the radar chart (#124). */
private val RADAR_EXERCISE_IDS = listOf("ua1", "ua2", "la1", "ub1", "ub2", "lb1")

internal fun buildStrengthCurveFor(
    exerciseId: String,
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): StrengthCurve? {
    val plan = Program.exercise(exerciseId) ?: return null
    val maxPerSession = allSets
        .filter { it.exerciseId == exerciseId && it.weightLb != null }
        .groupBy { it.sessionStartedAt }
        .map { (_, sessionSets) -> sessionSets.maxOf { it.weightLb!! } }
        .takeLast(STRENGTH_CURVE_MAX_POINTS)
    if (maxPerSession.isEmpty()) return null
    return StrengthCurve(plan = plan, points = maxPerSession)
}

internal fun buildPrEntries(
    rows: List<com.forge.app.data.db.projections.RecentPrRow>,
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<PrEntry> {
    // Group sets by sessionStartedAt + exerciseId so we can look up the PR set per row.
    // (LoggedExercise row uniquely identifies session+exercise; we approximate via
    // session date which is good enough for display purposes here.)
    return rows.map { row ->
        val candidateSets = allSets.filter {
            it.exerciseId == row.exerciseId && it.sessionStartedAt == row.sessionStartedAt
        }
        val prSet = candidateSets.maxByOrNull { it.weightLb ?: 0.0 }
        val name = row.swappedName
            ?: Program.exercise(row.exerciseId)?.name
            ?: row.exerciseId
        PrEntry(
            date = row.sessionStartedAt,
            exerciseName = name,
            weightText = prSet?.weightLb?.let { "${it.toInt()} lb" } ?: "—",
            reps = prSet?.reps ?: 0
        )
    }
}

internal fun buildHallOfFame(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    bodyweightLb: Double? = null
): List<PrRecord> {
    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapNotNull { (exerciseId, sets) ->
            val plan = Program.exercise(exerciseId) ?: return@mapNotNull null
            val bestSet = sets.maxByOrNull { it.weightLb!! } ?: return@mapNotNull null
            val rel = if (bodyweightLb != null && bodyweightLb > 0)
                (bestSet.weightLb!! / bodyweightLb * 10).toInt() / 10.0
            else null
            PrRecord(
                exerciseId = exerciseId,
                exerciseName = plan.name,
                maxWeightLb = bestSet.weightLb!!,
                bestReps = bestSet.reps,
                sessionDate = bestSet.sessionStartedAt,
                muscle = plan.muscle,
                relativeStrength = rel
            )
        }
        .sortedWith(compareBy({ it.muscle.displayName }, { it.exerciseName }))
}

internal fun buildExerciseHistory(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): Map<String, List<HistoryPoint>> {
    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapValues { (_, sets) ->
            sets
                .groupBy { it.sessionStartedAt }
                .map { (sessionDate, sessionSets) ->
                    HistoryPoint(
                        sessionDate = sessionDate,
                        maxWeightLb = sessionSets.maxOf { it.weightLb!! }
                    )
                }
                .sortedBy { it.sessionDate }
        }
}

/** Epley estimated 1-rep max. */
private fun e1rm(weightLb: Double, reps: Int): Double = weightLb * (1.0 + reps / 30.0)

/** Per-lift estimated-1RM progression: best e1RM per session, with growth rate + stall flag. */
internal fun buildE1rmLifts(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<E1rmLift> {
    return allSets
        .filter { it.weightLb != null && it.weightLb > 0 }
        .groupBy { it.exerciseId }
        .mapNotNull { (id, sets) ->
            val name = Program.exercise(id)?.name ?: return@mapNotNull null
            val perSession = sets.groupBy { it.sessionStartedAt }.toSortedMap()
            val points = perSession.map { (_, ss) -> ss.maxOf { e1rm(it.weightLb!!, it.reps) } }
            if (points.isEmpty()) return@mapNotNull null
            val dates = perSession.keys.toList()
            val first = points.first()
            val current = points.last()
            val monthlyPct = if (points.size >= 2 && first > 0) {
                val months = ((dates.last() - dates.first()) / (30.44 * 24 * 60 * 60 * 1000)).coerceAtLeast(0.5)
                (current - first) / first / months * 100.0
            } else null
            val stalling = points.size >= 3 && run {
                val recent = points.takeLast(3)
                val hi = recent.max()
                hi > 0 && (hi - recent.min()) / hi < 0.01
            }
            E1rmLift(
                exerciseId = id, exerciseName = name, currentE1rm = current,
                history = points, monthlyPct = monthlyPct, stalling = stalling
            )
        }
        .sortedByDescending { it.currentE1rm }
        .take(6)
}

/** Best weight at each rep count for the single most-trained lift. */
internal fun buildRepMaxes(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): RepMaxSet? {
    val byExercise = allSets.filter { it.weightLb != null && it.weightLb > 0 }.groupBy { it.exerciseId }
    val top = byExercise.maxByOrNull { it.value.size } ?: return null
    val name = Program.exercise(top.key)?.name ?: return null
    val entries = top.value
        .groupBy { it.reps }
        .map { (reps, ss) -> RepMaxEntry(reps = reps, weightLb = ss.maxOf { it.weightLb!! }) }
        .sortedBy { it.reps }
    return if (entries.isEmpty()) null else RepMaxSet(exerciseName = name, entries = entries)
}

/** Average estimated-1RM growth per month across lifts, as a percent. */
internal fun computeProgressiveOverload(lifts: List<E1rmLift>): Double? =
    lifts.mapNotNull { it.monthlyPct }.takeIf { it.isNotEmpty() }?.average()

internal fun buildCompoundMaxes(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): Map<String, Double> {
    return allSets
        .filter { it.weightLb != null && it.exerciseId in RADAR_EXERCISE_IDS }
        .groupBy { it.exerciseId }
        .mapValues { (_, sets) -> sets.maxOf { it.weightLb!! } }
}

internal fun buildExerciseYoY(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<ExerciseYoY> {
    val zone = ZoneId.systemDefault()
    val now = LocalDate.now(zone)
    val thisYearStart = now.withDayOfYear(1).atStartOfDay(zone).toInstant().toEpochMilli()
    val lastYearStart = now.minusYears(1).withDayOfYear(1).atStartOfDay(zone).toInstant().toEpochMilli()
    val lastYearEnd = thisYearStart

    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapNotNull { (exerciseId, sets) ->
            val plan = Program.exercise(exerciseId) ?: return@mapNotNull null
            val thisYearMax = sets.filter { it.sessionStartedAt >= thisYearStart }.maxOfOrNull { it.weightLb!! }
            val lastYearMax = sets.filter { it.sessionStartedAt in lastYearStart until lastYearEnd }.maxOfOrNull { it.weightLb!! }
            if (thisYearMax == null || lastYearMax == null) return@mapNotNull null
            ExerciseYoY(
                exerciseId = exerciseId,
                exerciseName = plan.name,
                thisYearMaxLb = thisYearMax,
                lastYearMaxLb = lastYearMax
            )
        }
        .sortedByDescending { it.delta }
}

internal fun buildTimeToPr(
    rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExercisePrDate>
): List<TimeToPrEntry> {
    return rows
        .groupBy { it.exerciseId }
        .mapNotNull { (exerciseId, dates) ->
            if (dates.size < 2) return@mapNotNull null
            val sorted = dates.sortedBy { it.sessionDate }
            val avgMs = sorted.zipWithNext { a, b -> b.sessionDate - a.sessionDate }.average()
            val avgDays = (avgMs / (24 * 60 * 60 * 1000)).toInt().coerceAtLeast(1)
            val name = Program.exercise(exerciseId)?.name ?: return@mapNotNull null
            TimeToPrEntry(exerciseId = exerciseId, exerciseName = name, avgDaysBetween = avgDays, prCount = dates.size)
        }
        .sortedBy { it.avgDaysBetween }
}

internal fun buildPrsByDayOfWeek(prTimes: List<Long>): List<Int> {
    val zone = ZoneId.systemDefault()
    val counts = IntArray(7)
    prTimes.forEach { ms ->
        val dow = Instant.ofEpochMilli(ms).atZone(zone).dayOfWeek.value - 1 // 0=Mon
        counts[dow]++
    }
    return counts.toList()
}
