package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.volume.VolumeCalculator
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
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.weightInputValue

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
    weightUnit: WeightUnit,
    bonusSets: Int = 0,
    finishedEarly: Boolean = false
): ExerciseUiState = coroutineScope {
    val sessionId = _state.value.sessionId ?: error("sessionId required")
    // The swap id drives attribution: every history/PR/stats read below keys on the EXERCISE ACTUALLY
    // PERFORMED (#11) — the swapped exercise when this slot is swapped, else the slot id. UI matching
    // still keys on plan.id (the slot) — see DayViewModelRefresh.
    //
    // Reads that DON'T need the swap id (sets, goal) start immediately and overlap the swap lookup;
    // only effectiveExerciseId and the reads that use it wait on getSwap. (Runs per exercise on
    // session open and on every set log, so the overlap matters.)
    val setsDeferred = async { logged?.let { workoutRepo.setsFor(it.id) }.orEmpty() }
    // Goals stay slot-keyed (the goal-setter opens per slot), so they survive a swap.
    val goalDeferred = async { goalRepo.get(plan.id)?.targetWeightLb }
    val persistent = customizationRepo.getSwap(plan.id)
    // Guard on a non-blank swap name: a cleared swap can leave a blank-name row behind (it shares the
    // row with a rest-timer override / pinned note), and its swappedExerciseId must NOT re-attribute
    // stats once the swap is gone (#11).
    val effectiveExerciseId = logged?.exerciseId
        ?: persistent?.takeIf { it.swappedName.isNotBlank() }?.swappedExerciseId
        ?: plan.id

    // Swap-aware unit: a swapped slot logs + progresses in the swapped exercise's unit, not the
    // base plan's, so a bodyweight→weighted swap suggests a weight (not more reps) and vice-versa.
    // Mirrors ExerciseUiState.effectiveUnit's precedence (session swap → persistent → base).
    val effectiveUnit = when {
        !logged?.swappedName.isNullOrBlank() -> logged?.swappedUnit
        !persistent?.swappedName.isNullOrBlank() -> persistent?.swappedUnit
        else -> null
    }?.let { com.forge.app.program.ExerciseUnit.fromCode(it) } ?: plan.unit

    // The remaining reads key on effectiveExerciseId; fan them out concurrently.
    val prevDeferred = async {
        val prevLE = workoutRepo.lastLoggedExerciseBefore(effectiveExerciseId, sessionId)
        prevLE to prevLE?.let { workoutRepo.setsFor(it.id) }.orEmpty()
    }
    // Bounded stand-ins for "every set ever logged" (which grew without limit): the per-rep
    // max-weight frontier answers PR questions identically, and the PB is a single LIMIT 1 row.
    val frontierDeferred = async {
        workoutRepo.repMaxFrontierForExercise(effectiveExerciseId, logged?.id).map { row ->
            // Wrapped as synthetic sets so PrDetector / the SetInputRow PR hint consume the
            // frontier through the same code path as real history (only weightLb/reps are read).
            LoggedSet(
                loggedExerciseId = 0L, setIndex = 0, weightText = "",
                weightLb = row.weightLb, reps = row.reps, completedAt = 0L
            )
        }
    }
    val hasHistoryDeferred = async { workoutRepo.hasHistoryForExercise(effectiveExerciseId, logged?.id) }
    val pbDeferred = async { workoutRepo.personalBestSet(effectiveExerciseId) }
    val aggregatesDeferred = async { workoutRepo.sessionAggregatesForExercise(effectiveExerciseId, limit = 8) }

    val sets = setsDeferred.await()
    val (prevLE, prevSets) = prevDeferred.await()
    val prevFirstSet = prevSets.firstOrNull()
    // No prior-session set → first time on this exercise: prompt a baseline instead of a blank helper.
    // Stored weights are lb (see LoggedSet). Everything below renders in the user's unit rather
    // than printing the raw stored text, which showed a kg user "Last: 220.5 × 5" for their 100 kg.
    // PLATES is the exception: its text is a plate COUNT, which is unit-independent already.
    val isPlates = effectiveUnit == com.forge.app.program.ExerciseUnit.PLATES
    fun displayWeight(set: LoggedSet): String {
        val lb = set.weightLb
        return if (isPlates || lb == null) set.weightText else formatWeight(lb, weightUnit)
    }
    val preview = prevFirstSet?.let { "Last: ${displayWeight(it)} × ${it.reps}" }
        ?: "First time — set your baseline"

    val priorFrontier = frontierDeferred.await()
    val (prSetIds, wasPr) = computePrFlags(priorFrontier, hasHistoryDeferred.await(), sets)
    // Only the single best PR set carries the gold ★ + gold text (heaviest → most reps → latest),
    // so a string of ascending PRs doesn't paint every row gold (#3/#4/#7). prSetIds still drives
    // the PR count + confetti, so each new PR still celebrates.
    val bestPrSetId = sets.filter { it.id in prSetIds }
        .maxWithOrNull(compareBy({ it.weightLb ?: Double.NEGATIVE_INFINITY }, { it.reps }, { it.setIndex }))
        ?.id

    // One column, not a whole row rebuilt from a snapshot this pass read earlier. The rebuild is
    // asynchronous, so a rating, note, skip, swap or target change landing while it was in flight
    // was reverted by the stale copy this used to write back.
    if (logged != null && logged.wasPr != wasPr) {
        workoutRepo.setWasPr(logged.id, wasPr)
    }

    // Double-progression suggestion (#12/#13) — pure rules in the adaptation engine; the
    // VM only assembles inputs. inputText is unit-correct (plate count on PLATES exercises).
    val stepMode = stepCalibration.modeFor(effectiveExerciseId, effectiveUnit.code)
    val suggestion = ProgressionAdvisor.suggestNextLoad(
        exerciseId = effectiveExerciseId,
        exerciseName = plan.name,
        prevSets = prevSets,
        prevEffort = prevLE?.difficulty,
        repsText = plan.reps,
        unit = effectiveUnit,
        plateLb = plateLb,
        intensity = IntensityIntent.fromCode(_state.value.sessionIntensity),
        readiness = readiness,
        // The block's phase, composed with today's scale rather than left to shape only the plan's
        // set counts (H-02).
        phase = blockPhase,
        dbMaxLb = dbMaxLb,
        // Doubling the step only makes sense on the DB grid; PLATES move whole plates anyway.
        fastStep = stepMode == com.forge.app.domain.coach.StepMode.FASTER && effectiveUnit == com.forge.app.program.ExerciseUnit.DUMBBELL,
        consolidate = stepMode == com.forge.app.domain.coach.StepMode.CONSOLIDATE
    )

    // Bodyweight movements have no weight to suggest, so they get a rep-progression cue instead (CO5).
    val repSuggestion = if (effectiveUnit == com.forge.app.program.ExerciseUnit.BODYWEIGHT)
        ProgressionAdvisor.suggestNextReps(effectiveExerciseId, plan.name, prevSets, prevLE?.difficulty, plan.reps)
    else null

    val pbSet = pbDeferred.await()
    val allTimePbLb = pbSet?.weightLb
    val allTimePbText = pbSet?.let { "${displayWeight(it)} × ${it.reps}" }
    val goalWeightLb = goalDeferred.await()

    // Through the canonical helper: a timed hold's reps is a duration, so open-coding this made a
    // 90-second plank outweigh the whole rest of the session in the "vs last time" verdict.
    val currentVolume = VolumeCalculator.sessionVolumeLb(sets)
    val prevVolume = VolumeCalculator.sessionVolumeLb(prevSets)
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

    // inputText is lb (or, on a PLATES exercise, a plate count). It seeds the weight FIELD, which
    // is labelled in the display unit — so an unconverted lb number invited a kg user to log
    // "Suggested next → 222.5" as 222.5 kg, i.e. 490 lb.
    val displaySuggested = suggestion?.let { s ->
        if (isPlates) s.inputText else weightInputValue(s.targetWeightLb, weightUnit)
    } ?: if (DUMMY_TRAINING_DATA) "45" else null
    // Tier 3 — make the invisible weight calibration visible: append the SuggestionCalibrator's
    // StepMode to the suggestion reason, so it shows wherever the reason renders (card + input row).
    val stepModeNote = when (stepMode) {
        com.forge.app.domain.coach.StepMode.FASTER -> "bigger jumps fit you"
        com.forge.app.domain.coach.StepMode.CONSOLIDATE -> "holding to consolidate"
        else -> null
    }
    val baseReason = suggestion?.reason
        ?: if (DUMMY_TRAINING_DATA && displaySuggested != null) "matches set 1" else null
    val displayReason = if (displaySuggested != null)
        listOfNotNull(baseReason, stepModeNote).joinToString(" · ").ifBlank { null }
    else baseReason

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
        effectiveExerciseId = effectiveExerciseId,
        loggedExerciseId = logged?.id,
        loggedSets = sets,
        lastSessionPreviewText = preview,
        prefillWeight = prevFirstSet?.let {
            val lb = it.weightLb
            if (isPlates || lb == null) it.weightText else weightInputValue(lb, weightUnit)
        },
        difficulty = logged?.difficulty,
        note = logged?.note,
        skipped = logged?.skipped ?: false,
        isExpanded = expandedOverride ?: expandedDefault,
        wasPr = wasPr,
        prSetIds = prSetIds,
        bestPrSetId = bestPrSetId,
        sessionSwapName = logged?.swappedName,
        sessionSwapUnit = logged?.swappedUnit,
        persistentSwapName = persistent?.swappedName,
        persistentSwapUnit = persistent?.swappedUnit,
        suggestedWeight = displaySuggested,
        suggestionReason = displayReason,
        suggestedDeltaLb = suggestion?.deltaLb,
        suggestedTargetLb = suggestion?.targetWeightLb,
        suggestedReps = repSuggestion?.targetReps,
        suggestedRepsReason = repSuggestion?.reason,
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
        bonusSets = bonusSets,
        finishedEarly = finishedEarly
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
): RestPrescription = RestAdvisor.restSeconds(
    plan, effortRating, overrideSeconds, restTuning,
    compoundBase = restBaseCompound, isolationBase = restBaseIsolation
)

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
        // The duration goes in too: a timed hold's `reps` is a hold length, and without saying so
        // every weighted plank painted itself gold and then sat in `running` as a rep count no
        // real set could reach.
        if (!set.isAssisted && PrDetector.isPr(running, set.weightLb, set.reps, set.durationSeconds)) {
            prIds.add(set.id)
        }
        running.add(set)
    }
    return prIds to prIds.isNotEmpty()
}
