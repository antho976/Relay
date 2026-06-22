package com.forge.app.data.repo

import com.forge.app.data.db.projections.SetWithExerciseAndSession
import com.forge.app.domain.adapt.E1rm
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.DayLoad
import com.forge.app.ui.gym.stats.state.RepWeightPoint
import com.forge.app.ui.gym.stats.state.StrengthCurve
import java.time.Instant
import java.time.ZoneId

// Pure aggregation helpers for the rebuilt Stats charts (Tier 6 strength curve, Tier 7/8 daily load).
// Operate on already-loaded set projections — no DAO/DI — so the repository orchestrator calls them.

/**
 * Per-exercise load-rep scatter for the most-trained lifts: every working set as a (reps, weight)
 * point, plus the lift's Epley e1RM so the UI can draw the fitted curve and the extrapolated 1-rep
 * point. Capped at [maxLifts] (by set count) and gated at [minPoints] so a curve only shows once the
 * scatter is worth fitting.
 */
internal fun buildStrengthCurves(
    allSets: List<SetWithExerciseAndSession>,
    maxLifts: Int = 3,
    minPoints: Int = 6
): List<StrengthCurve> =
    allSets
        .filter { it.weightLb != null && it.weightLb > 0 && it.reps in 1..15 }
        .groupBy { it.exerciseId }
        .mapNotNull { (id, sets) ->
            if (sets.size < minPoints) return@mapNotNull null
            val name = Program.exercise(id)?.name ?: return@mapNotNull null
            StrengthCurve(
                exerciseId = id,
                exerciseName = name,
                points = sets.map { RepWeightPoint(it.reps, it.weightLb!!) },
                e1rmLb = sets.maxOf { E1rm.epley(it.weightLb!!, it.reps) }
            )
        }
        .sortedByDescending { it.points.size }
        .take(maxLifts)

/**
 * Per-day training load (set count + tonnage), oldest → newest. Days with no logged work are absent;
 * the calendar/Banister consumers fill the gaps. Bucketed by session start in the system zone, the
 * same basis every other Stats window uses.
 */
internal fun buildDailyActivity(
    allSets: List<SetWithExerciseAndSession>,
    zone: ZoneId = ZoneId.systemDefault()
): List<DayLoad> =
    allSets
        .groupBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(zone).toLocalDate().toEpochDay() }
        .map { (day, sets) ->
            DayLoad(
                epochDay = day,
                sets = sets.size,
                volumeLb = sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
            )
        }
        .sortedBy { it.epochDay }
