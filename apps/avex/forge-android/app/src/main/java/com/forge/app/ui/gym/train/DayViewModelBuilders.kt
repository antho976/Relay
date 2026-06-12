package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.adapt.IntensityIntent
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.adapt.RestAdvisor
import com.forge.app.domain.adapt.RestPrescription
import com.forge.app.domain.pr.PrDetector
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.state.ExerciseSessionPoint
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.VsLastStatus
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import java.util.concurrent.TimeUnit

/**
 * When true, fills the day screen with fabricated "last session" / "suggested next" /
 * sparkline data. Off by default — the UI shows real data only, with empty-state hints
 * (e.g. "first time", "Log a couple of sessions…") where there's nothing yet.
 */
private const val DUMMY_TRAINING_DATA = false

internal suspend fun DayViewModel.buildExerciseUi(
    plan: ExercisePlan,
    logged: LoggedExercise?,
    expandedDefault: Boolean,
    expandedOverride: Boolean?,
    plateLb: Double,
    dbMaxLb: Double?,
    bonusSets: Int = 0
): ExerciseUiState = coroutineScope {
    val sessionId = _state.value.sessionId ?: error("sessionId required")
    // One card needs six independent DB reads — fan them out so a build costs ~2 sequential
    // round-trips instead of ~7 (this runs per exercise on session open and on every set log).
    val setsDeferred = async { logged?.let { workoutRepo.setsFor(it.id) }.orEmpty() }
    val prevDeferred = async {
        val prevLE = workoutRepo.lastLoggedExerciseBefore(plan.id, sessionId)
        prevLE to prevLE?.let { workoutRepo.setsFor(it.id) }.orEmpty()
    }
    val persistentDeferred = async { customizationRepo.getSwap(plan.id) }
    // Bounded stand-ins for "every set ever logged" (which grew without limit): the per-rep
    // max-weight frontier answers PR questions identically, and the PB is a single LIMIT 1 row.
    val frontierDeferred = async {
        workoutRepo.repMaxFrontierForExercise(plan.id, logged?.id).map { row ->
            // Wrapped as synthetic sets so PrDetector / the SetInputRow PR hint consume the
            // frontier through the same code path as real history (only weightLb/reps are read).
            LoggedSet(
                loggedExerciseId = 0L, setIndex = 0, weightText = "",
                weightLb = row.weightLb, reps = row.reps, completedAt = 0L
            )
        }
    }
    val hasHistoryDeferred = async { workoutRepo.hasHistoryForExercise(plan.id, logged?.id) }
    val pbDeferred = async { workoutRepo.personalBestSet(plan.id) }
    val goalDeferred = async { goalRepo.get(plan.id)?.targetWeightLb }
    val aggregatesDeferred = async { workoutRepo.sessionAggregatesForExercise(plan.id, limit = 8) }

    val sets = setsDeferred.await()
    val (prevLE, prevSets) = prevDeferred.await()
    val prevFirstSet = prevSets.firstOrNull()
    val preview = prevFirstSet?.let { "Last: ${it.weightText} × ${it.reps}" }
    val persistent = persistentDeferred.await()

    val priorFrontier = frontierDeferred.await()
    val (prSetIds, wasPr) = computePrFlags(priorFrontier, hasHistoryDeferred.await(), sets)

    if (logged != null && logged.wasPr != wasPr) {
        workoutRepo.updateExercise(logged.copy(wasPr = wasPr))
    }

    // Double-progression suggestion (#12/#13) — pure rules in the adaptation engine; the
    // VM only assembles inputs. inputText is unit-correct (plate count on PLATES exercises).
    val stepMode = stepCalibration.modeFor(plan.id, plan.unit.code)
    val suggestion = ProgressionAdvisor.suggestNextLoad(
        exerciseId = plan.id,
        exerciseName = plan.name,
        prevSets = prevSets,
        prevEffort = prevLE?.difficulty,
        repsText = plan.reps,
        unit = plan.unit,
        plateLb = plateLb,
        intensity = IntensityIntent.fromCode(_state.value.sessionIntensity),
        readiness = readiness,
        dbMaxLb = dbMaxLb,
        // Doubling the step only makes sense on the DB grid; PLATES move whole plates anyway.
        fastStep = stepMode == com.forge.app.domain.coach.StepMode.FASTER && plan.unit == com.forge.app.program.ExerciseUnit.DUMBBELL,
        consolidate = stepMode == com.forge.app.domain.coach.StepMode.CONSOLIDATE
    )

    val pbSet = pbDeferred.await()
    val allTimePbLb = pbSet?.weightLb
    val allTimePbText = pbSet?.let { "${it.weightText} × ${it.reps}" }
    val goalWeightLb = goalDeferred.await()

    val currentVolume = sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
    val prevVolume = prevSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
    val vsLastStatus = when {
        sets.isEmpty() || prevSets.isEmpty() -> null
        currentVolume > prevVolume * 1.05 -> VsLastStatus.BEATING
        currentVolume >= prevVolume * 0.95 -> VsLastStatus.MATCHING
        else -> VsLastStatus.UNDER
    }

    val sessionHistory = aggregatesDeferred.await()
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
    val displayPrior = if (DUMMY_TRAINING_DATA && prevSets.isEmpty()) {
        List(plan.sets.coerceAtLeast(3)) { i ->
            LoggedSet(
                id = -(i + 1L), loggedExerciseId = -1L, setIndex = i,
                weightText = "40", weightLb = 40.0, reps = 10 - i, completedAt = 0L, rpe = 8.0
            )
        }
    } else prevSets

    val displaySuggested = suggestion?.inputText ?: if (DUMMY_TRAINING_DATA) "45" else null
    val displayReason = suggestion?.reason
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

    ExerciseUiState(
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
        suggestedDeltaLb = suggestion?.deltaLb,
        suggestedTargetLb = suggestion?.targetWeightLb,
        priorSets = displayPrior,
        priorFrontier = priorFrontier,
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
 * Second-pass annotation: surface each exercise's suggested weight delta on the *previous*
 * card (the "UP NEXT  +5 ↑" pill). Reads the lb delta the advisor computed — the old
 * version re-derived it from the suggestion *string*, which broke on PLATES exercises
 * (a plate-count text compared against pounds).
 */
internal fun annotateNextExerciseDeltas(exercises: List<ExerciseUiState>): List<ExerciseUiState> =
    exercises.mapIndexed { idx, ex ->
        val delta = exercises.getOrNull(idx + 1)?.suggestedDeltaLb ?: return@mapIndexed ex
        if (kotlin.math.abs(delta) < 0.5) return@mapIndexed ex
        val sign = if (delta > 0) "+" else "−"
        val abs = kotlin.math.abs(delta)
        val deltaStr = if (abs % 1.0 == 0.0) "$sign${abs.toInt()}" else "$sign$abs"
        ex.copy(nextSuggestedWeightDelta = deltaStr)
    }

/**
 * Rest duration + explanation for the timer started after a set. Delegates to the
 * adaptation engine's RestAdvisor: manual override → canonical SessionEstimate base
 * (+30s after brutal) → personal tuning from realized rest behavior ([DayViewModel.restTuning]).
 */
internal fun DayViewModel.computeRestPrescription(
    plan: ExercisePlan,
    effortRating: EffortRating?,
    overrideSeconds: Int? = null
): RestPrescription = RestAdvisor.restSeconds(plan, effortRating, overrideSeconds, restTuning)

internal fun computePrFlags(
    priorFrontier: List<LoggedSet>,
    hasPriorHistory: Boolean,
    currentSets: List<LoggedSet>
): Pair<Set<Long>, Boolean> {
    if (currentSets.isEmpty()) return emptySet<Long>() to false
    // No prior history → first-ever time on this exercise; there's no record to beat, so
    // nothing flags gold. (This intentionally diverges from PrDetector.isPr's documented
    // "empty history = PR" rule — that rule is about a single proposed set, whereas here an
    // entire first session shouldn't paint every set gold.) Checked against ALL prior sets:
    // a bodyweight/assisted-only history still counts as history even though it contributes
    // nothing to the weighted frontier.
    if (!hasPriorHistory) return emptySet<Long>() to false
    // Judge each set against prior sessions AND the earlier sets of THIS session: once a
    // heavier set beats the record, a later lighter set in the same session must not also
    // flag as a PR. Walk sets in performed order, growing the comparison history as we go.
    // isPr only reads (weightLb, reps, isAssisted), so the frontier substitutes exactly for
    // the full prior-set list it used to receive.
    val running = priorFrontier.toMutableList()
    val prIds = mutableSetOf<Long>()
    for (set in currentSets.sortedBy { it.setIndex }) {
        // An assisted set is never itself a PR (and assisted history is excluded inside isPr).
        if (!set.isAssisted && PrDetector.isPr(running, set.weightLb, set.reps)) prIds.add(set.id)
        running.add(set)
    }
    return prIds to prIds.isNotEmpty()
}
