package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.pr.PrDetector
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.state.ExerciseSessionPoint
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.VsLastStatus
import java.util.concurrent.TimeUnit

/**
 * TEMP: fills the day screen with placeholder "last session", "suggested next", and
 * sparkline data when no real history exists yet, so the UI can be evaluated against
 * the mockup. Flip to false (or delete the dummy blocks) once real sessions accumulate.
 */
private const val DUMMY_TRAINING_DATA = true

internal suspend fun DayViewModel.buildExerciseUi(
    plan: ExercisePlan,
    logged: LoggedExercise?,
    expandedDefault: Boolean,
    expandedOverride: Boolean?,
    bonusSets: Int = 0
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

    val sessionHistory = workoutRepo.sessionAggregatesForExercise(plan.id, limit = 8)
        .map { agg ->
            ExerciseSessionPoint(
                sessionStartedAt = agg.sessionStartedAt,
                durationMin = agg.sessionFinishedAt?.let { end ->
                    TimeUnit.MILLISECONDS.toMinutes(end - agg.sessionStartedAt).toInt().coerceAtLeast(0)
                },
                volumeLb = agg.volumeLb,
                topWeightLb = agg.topWeightLb
            )
        }

    // ── TEMP dummy fallbacks (gated by DUMMY_TRAINING_DATA) ───────────────────────
    val displayPrior = if (DUMMY_TRAINING_DATA && prior.isEmpty()) {
        List(plan.sets.coerceAtLeast(3)) { i ->
            LoggedSet(
                id = -(i + 1L), loggedExerciseId = -1L, setIndex = i,
                weightText = "40", weightLb = 40.0, reps = 10 - i, completedAt = 0L, rpe = 8.0
            )
        }
    } else prior

    val displaySuggested = suggestedWeight ?: if (DUMMY_TRAINING_DATA) "45" else null
    val displayReason = suggestionReason
        ?: if (DUMMY_TRAINING_DATA && displaySuggested != null) "matches set 1" else null

    val displayHistory = if (DUMMY_TRAINING_DATA && sessionHistory.isEmpty()) {
        val now = System.currentTimeMillis()
        val day = 24L * 60 * 60 * 1000
        listOf(
            ExerciseSessionPoint(now - 3 * day, 14, 320.0, 40.0),
            ExerciseSessionPoint(now - 10 * day, 13, 300.0, 37.5),
            ExerciseSessionPoint(now - 17 * day, 15, 290.0, 37.5),
            ExerciseSessionPoint(now - 24 * day, 12, 270.0, 35.0),
            ExerciseSessionPoint(now - 31 * day, 14, 260.0, 35.0),
            ExerciseSessionPoint(now - 38 * day, 13, 240.0, 32.5)
        )
    } else sessionHistory
    // ──────────────────────────────────────────────────────────────────────────────

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
        suggestedWeight = displaySuggested,
        suggestionReason = displayReason,
        priorSets = displayPrior,
        allTimePbText = allTimePbText,
        allTimePbLb = allTimePbLb,
        vsLastStatus = vsLastStatus,
        goalWeightLb = goalWeightLb,
        restTimerOverrideSeconds = persistent?.restTimerOverrideSeconds,
        pinnedNote = persistent?.pinnedNote ?: "",
        supersetGroup = logged?.supersetGroup,
        sessionHistory = displayHistory,
        bonusSets = bonusSets
    )
}

/**
 * Second-pass annotation: for each exercise, compute the suggested weight delta for the
 * *next* exercise (used by the "UP NEXT  +5 ↑" pill). Pure function — no DB access.
 */
internal fun annotateNextExerciseDeltas(exercises: List<ExerciseUiState>): List<ExerciseUiState> =
    exercises.mapIndexed { idx, ex ->
        val next = exercises.getOrNull(idx + 1) ?: return@mapIndexed ex
        val nextSuggestedLb = next.suggestedWeight?.toDoubleOrNull() ?: return@mapIndexed ex
        val nextPrevMaxLb = next.priorSets
            .filter { it.loggedExerciseId != next.loggedExerciseId }
            .mapNotNull { it.weightLb }
            .maxOrNull() ?: return@mapIndexed ex
        val delta = nextSuggestedLb - nextPrevMaxLb
        if (kotlin.math.abs(delta) < 0.5) return@mapIndexed ex
        val sign = if (delta > 0) "+" else "−"
        val abs = kotlin.math.abs(delta)
        val deltaStr = if (abs % 1.0 == 0.0) "$sign${abs.toInt()}" else "$sign$abs"
        ex.copy(nextSuggestedWeightDelta = deltaStr)
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
