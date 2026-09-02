package com.forge.app.ui.gym.train

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.warmup.WarmupEngine
import com.forge.app.domain.warmup.WarmupExercise
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.SessionEstimate
import com.forge.app.ui.gym.train.state.ExerciseUiState
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update

/**
 * Derives this session's warmup from the exercises actually queued and the loads the user is
 * actually going to lift, then folds it into state.
 */
internal suspend fun DayViewModel.rebuildWarmupProtocol() {
    val current = _state.value
    if (current.isWarmupComplete || current.exercises.isEmpty()) return

    val metric = runCatching { settingsRepo.weightUnit.first() }.getOrNull() == WeightUnit.KG
    // Loads are stored in pounds even on a plate machine; the engine's PLATES scale is a count.
    val plateLb = runCatching { settingsRepo.plateWeightLb.first() }.getOrNull()
        ?.takeIf { it > 0.0 } ?: DEFAULT_PLATE_LB

    val queued = current.exercises
        .filterNot { it.skipped }
        .map { it.toWarmupExercise(metric, plateLb) }
    if (queued.isEmpty()) return

    val protocol = WarmupEngine.build(
        exercises = queued,
        customDrills = current.customWarmupItems
    )
    _state.update { it.copy(warmupProtocol = protocol) }
}

/**
 * The working load in stored pounds. [toWarmupExercise] turns it into a plate count on
 * [ExerciseUnit.PLATES], the engine's scale for that unit, so the ramp arithmetic and the set row
 * agree.
 *
 * Preference order is what the user is most likely to actually load: the progression engine's
 * target for today, then the heaviest set they did last time, then their all-time best. Falling
 * all the way through leaves it null, and the engine prescribes an unloaded rehearsal set rather
 * than a made-up number.
 */
private fun ExerciseUiState.workingLoad(): Double? =
    suggestedTargetLb
        ?: priorSets.mapNotNull { it.weightLb }.maxOrNull()
        ?: allTimePbLb

/** Fallback when the plate-weight preference cannot be read; matches the settings default. */
private const val DEFAULT_PLATE_LB = 15.0

private fun ExerciseUiState.toWarmupExercise(metric: Boolean, plateLb: Double): WarmupExercise {
    val unit = effectiveUnit
    return WarmupExercise(
        id = plan.id,
        name = effectiveName,
        muscle = plan.muscle,
        unit = unit,
        isCompound = SessionEstimate.isCompound(plan),
        workingLoad = workingLoad()?.let { if (unit == ExerciseUnit.PLATES) it / plateLb else it },
        // The LOW end of the planned range is the heaviest set in it, so it sets the intensity the
        // ramp has to reach. Reading the high end would under-build the ladder for "6 to 10".
        targetReps = minRepsOf(plan.reps),
        loadStep = WarmupEngine.loadIncrement(unit, metric)
    )
}

/** Smallest rep count named in a range string ("8-10" to 8, "10/leg" to 10). Defaults to 10. */
private fun minRepsOf(reps: String): Int =
    Regex("\\d+").findAll(reps).mapNotNull { it.value.toIntOrNull() }.minOrNull() ?: 10
