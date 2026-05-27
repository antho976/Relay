package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.pr.PrDetector
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.VsLastStatus

internal suspend fun DayViewModel.buildExerciseUi(
    plan: ExercisePlan,
    logged: LoggedExercise?,
    expandedDefault: Boolean,
    expandedOverride: Boolean?
): ExerciseUiState {
    val sessionId = _state.value.sessionId ?: error("sessionId required")
    val sets = logged?.let { workoutRepo.setsFor(it.id) }.orEmpty()
    val prevLE = workoutRepo.lastLoggedExerciseBefore(plan.id, sessionId)
    val prevSets = prevLE?.let { workoutRepo.setsFor(it.id) }.orEmpty()
    val prevFirstSet = prevSets.firstOrNull()
    val preview = prevFirstSet?.let { "Last: ${it.weightText} × ${it.reps}" }
    val persistent = customizationRepo.getSwap(plan.id)

    val allHistory = workoutRepo.historyForExercise(plan.id)
    val prior = allHistory.filter { it.loggedExerciseId != logged?.id }
    val (prSetIds, wasPr) = computePrFlags(prior, sets)

    if (logged != null && logged.wasPr != wasPr) {
        workoutRepo.updateExercise(logged.copy(wasPr = wasPr))
    }

    val (suggestedWeight, suggestionReason) = computeWeightSuggestion(plan, prevLE, prevSets)

    val pbSet = allHistory.filter { it.weightLb != null }.maxByOrNull { it.weightLb!! }
    val allTimePbLb = pbSet?.weightLb
    val allTimePbText = pbSet?.let { best ->
        val maxRepsAtWeight = allHistory.filter { it.weightLb == best.weightLb }.maxOf { it.reps }
        "${best.weightText} × $maxRepsAtWeight"
    }
    val goalWeightLb = goalRepo.get(plan.id)?.targetWeightLb

    val currentVolume = sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
    val prevVolume = prevSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
    val vsLastStatus = when {
        sets.isEmpty() || prevSets.isEmpty() -> null
        currentVolume > prevVolume * 1.05 -> VsLastStatus.BEATING
        currentVolume >= prevVolume * 0.95 -> VsLastStatus.MATCHING
        else -> VsLastStatus.UNDER
    }

    return ExerciseUiState(
        plan = plan,
        loggedExerciseId = logged?.id,
        loggedSets = sets,
        lastSessionPreviewText = preview,
        prefillWeight = prevFirstSet?.weightText,
        difficulty = logged?.difficulty,
        note = logged?.note,
        skipped = logged?.skipped ?: false,
        isExpanded = expandedOverride ?: expandedDefault,
        wasPr = wasPr,
        prSetIds = prSetIds,
        sessionSwapName = logged?.swappedName,
        sessionSwapUnit = logged?.swappedUnit,
        persistentSwapName = persistent?.swappedName,
        persistentSwapUnit = persistent?.swappedUnit,
        suggestedWeight = suggestedWeight,
        suggestionReason = suggestionReason,
        priorSets = prior,
        allTimePbText = allTimePbText,
        allTimePbLb = allTimePbLb,
        vsLastStatus = vsLastStatus,
        goalWeightLb = goalWeightLb,
        restTimerOverrideSeconds = persistent?.restTimerOverrideSeconds,
        pinnedNote = persistent?.pinnedNote ?: "",
        supersetGroup = logged?.supersetGroup
    )
}

internal fun DayViewModel.computeWeightSuggestion(
    plan: ExercisePlan,
    prevLE: LoggedExercise?,
    prevSets: List<LoggedSet>
): Pair<String?, String?> {
    if (prevLE == null || prevSets.isEmpty()) return null to null
    val prevMaxWeight = prevSets.mapNotNull { it.weightLb }.maxOrNull() ?: return null to null
    val prevMaxReps = prevSets.maxOf { it.reps }
    val planMaxReps = parseMaxReps(plan.reps)
    val hitTopOfRange = planMaxReps != null && prevMaxReps >= planMaxReps
    val difficulty = prevLE.difficulty

    val (adjustmentLb, reason) = when {
        hitTopOfRange && (difficulty == null || difficulty == EffortRating.EASY ||
            difficulty == EffortRating.JUST_RIGHT) -> 2.5 to "hit top of range"
        difficulty == EffortRating.BRUTAL -> -2.5 to "last rated brutal"
        else -> return null to null
    }

    val intensityMultiplier = when (_state.value.sessionIntensity) {
        "light" -> 0.9
        "hard"  -> 1.05
        else    -> 1.0
    }
    val suggested = ((prevMaxWeight + adjustmentLb) * intensityMultiplier)
        .let { w -> (w / 2.5).toInt() * 2.5 }
    if (suggested <= 0.0) return null to null
    val adjustedReason = if (intensityMultiplier != 1.0) "$reason · intensity adjusted" else reason
    val suggestedStr = if (suggested % 1.0 == 0.0) "${suggested.toInt()}" else "$suggested"
    return suggestedStr to adjustedReason
}

internal fun DayViewModel.computeTimerDuration(
    plan: ExercisePlan,
    effortRating: EffortRating?,
    overrideSeconds: Int? = null
): Int {
    if (overrideSeconds != null) return overrideSeconds
    val base = when (plan.muscle) {
        MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.QUADS,
        MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES -> 180
        else -> 90
    }
    return base + if (effortRating == EffortRating.BRUTAL) 30 else 0
}

internal fun computePrFlags(
    prior: List<LoggedSet>,
    currentSets: List<LoggedSet>
): Pair<Set<Long>, Boolean> {
    if (currentSets.isEmpty()) return emptySet<Long>() to false
    val prIds = currentSets
        .filter { PrDetector.isPr(prior, it.weightLb, it.reps) }
        .map { it.id }.toSet()
    return prIds to prIds.isNotEmpty()
}

internal fun parseMaxReps(repsText: String): Int? =
    repsText.split(Regex("[^0-9]+")).mapNotNull { it.toIntOrNull() }.maxOrNull()
