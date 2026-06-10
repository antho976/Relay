package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.ExerciseUnit
import kotlin.math.roundToInt

/**
 * The session's intensity intent (Session.intensity, #123), typed for the engine.
 * Readiness autoregulation (System 6) will later *suggest* this value; the user's explicit
 * pick always wins.
 */
enum class IntensityIntent {
    LIGHT, NORMAL, HARD;

    companion object {
        fun fromCode(code: String?): IntensityIntent = when (code) {
            "light" -> LIGHT
            "hard" -> HARD
            else -> NORMAL
        }
    }
}

/** Numeric rep range parsed from a plan's display string ("8-10" → 8..10, "15" → 15..15). */
data class RepRange(val min: Int, val max: Int) {
    companion object {
        /** Null when the text holds no digits (pure-AMRAP/timed entries stay suggestion-free). */
        fun parse(text: String): RepRange? {
            val nums = Regex("\\d+").findAll(text).map { it.value.toInt() }.toList()
            if (nums.isEmpty()) return null
            return RepRange(nums.min(), nums.max())
        }
    }
}

/**
 * System 1 of the adaptation engine: double progression + plateau detection. Pure —
 * no DAO/Android/clock dependencies; both entry points are deterministic functions of
 * their inputs (mirrors [com.forge.app.domain.trophy.TrophyEvaluator]).
 *
 * Two entry points:
 *  - [suggestNextLoad] — the in-session suggestion chip: next load for one exercise from
 *    its previous bout. Cheap; safe on the day-screen build path.
 *  - [evaluate] — the snapshot pass: plateau detection over the whole program, with an
 *    escalation ladder (micro-load / reset → rep-range shift → variation swap).
 *
 * Double progression: reps climb within the slot's range first; weight rises only when
 * every top-weight set reached the top of the range at acceptable effort. Per-set RPE,
 * when logged, outranks the coarse per-exercise rating.
 */
object ProgressionAdvisor {

    // ── Chip path ──────────────────────────────────────────────────────────────

    /**
     * Suggested next load from the previous bout on this exercise, or null when there is
     * no defensible suggestion (cold start, bodyweight, mid-range progress, HARD-but-not-
     * brutal effort — the silent cases are deliberate).
     *
     * Unit-aware: DUMBBELL moves in [AdaptThresholds.dumbbellStepLb] steps (scaled);
     * PLATES moves in whole plates and ignores fine scaling (a 15 lb plate is too coarse —
     * any down-scale just suppresses increases); BODYWEIGHT never gets a weight suggestion.
     *
     * Scaling: an explicit pre-session [intensity] pick always wins (user intent is not
     * second-guessed); on a NORMAL day, [readiness] (System 6) fills in — a bounded ±few-%
     * autoregulation nudge whose reason is carried into the chip.
     */
    fun suggestNextLoad(
        exerciseId: String,
        exerciseName: String,
        prevSets: List<LoggedSet>,
        prevEffort: EffortRating?,
        repsText: String,
        unit: ExerciseUnit,
        plateLb: Double,
        intensity: IntensityIntent = IntensityIntent.NORMAL,
        readiness: Recommendation.ReadinessScale? = null,
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.WeightChange? {
        if (unit == ExerciseUnit.BODYWEIGHT) return null
        val working = prevSets.filter { !it.isAssisted && it.weightLb != null }
        val prevMax = working.maxOfOrNull { it.weightLb!! } ?: return null

        val scale: Double
        val scaleNote: String?
        when {
            intensity != IntensityIntent.NORMAL -> {
                scale = intensityScale(intensity, t)
                scaleNote = "intensity adjusted"
            }
            readiness != null -> {
                scale = 1 + readiness.percent / 100.0
                scaleNote = if (readiness.percent < 0) "eased ${-readiness.percent}% — readiness low"
                else "+${readiness.percent}% — readiness high"
            }
            else -> {
                scale = 1.0
                scaleNote = null
            }
        }

        // Per-set RPE outranks the coarse exercise rating when present.
        val maxRpe = working.mapNotNull { it.rpe }.maxOrNull()
        val backOff: Boolean
        val okToProgress: Boolean
        val backOffReason: String
        if (maxRpe != null) {
            backOff = maxRpe >= t.rpeBrutalMin
            okToProgress = maxRpe <= t.rpeEasyMax
            backOffReason = "last RPE hit ${trim(maxRpe)}"
        } else {
            backOff = prevEffort == EffortRating.BRUTAL
            okToProgress = prevEffort == null ||
                prevEffort == EffortRating.EASY || prevEffort == EffortRating.JUST_RIGHT
            backOffReason = "last rated brutal"
        }

        val range = RepRange.parse(repsText)
        val topSets = working.filter { it.weightLb == prevMax }
        val hitTopOnAllTopSets = range != null && topSets.all { it.reps >= range.max }

        return when {
            backOff -> backOffSuggestion(exerciseId, exerciseName, prevMax, unit, plateLb, scale, scaleNote, backOffReason, t)
            hitTopOnAllTopSets && okToProgress ->
                progressSuggestion(exerciseId, exerciseName, prevMax, unit, plateLb, scale, scaleNote, t)
            else -> null
        }
    }

    private fun progressSuggestion(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        unit: ExerciseUnit,
        plateLb: Double,
        scale: Double,
        scaleNote: String?,
        t: AdaptThresholds
    ): Recommendation.WeightChange? = when (unit) {
        ExerciseUnit.DUMBBELL -> {
            val target = floorToGrid((prevMax + t.dumbbellStepLb) * scale, t.dumbbellStepLb)
            if (target <= 0.0) null
            else weightChange(
                exerciseId, exerciseName, prevMax, target,
                inputText = trim(target),
                reason = withNote("hit top of range", scaleNote)
            )
        }
        ExerciseUnit.PLATES ->
            // A whole plate is too big a step up on a deliberately light / low-readiness day.
            if (scale < 1.0) null
            else plateChange(exerciseId, exerciseName, prevMax, prevMax + plateLb, plateLb, "hit top of range")
        ExerciseUnit.BODYWEIGHT -> null
    }

    private fun backOffSuggestion(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        unit: ExerciseUnit,
        plateLb: Double,
        scale: Double,
        scaleNote: String?,
        reason: String,
        t: AdaptThresholds
    ): Recommendation.WeightChange? = when (unit) {
        ExerciseUnit.DUMBBELL -> {
            val target = floorToGrid((prevMax - t.dumbbellStepLb) * scale, t.dumbbellStepLb)
            if (target <= 0.0) null
            else weightChange(
                exerciseId, exerciseName, prevMax, target,
                inputText = trim(target),
                reason = withNote(reason, scaleNote)
            )
        }
        ExerciseUnit.PLATES -> {
            val target = prevMax - plateLb
            // Can't drop below one plate — stay silent rather than suggest the impossible.
            if (target < plateLb) null
            else plateChange(exerciseId, exerciseName, prevMax, target, plateLb, reason)
        }
        ExerciseUnit.BODYWEIGHT -> null
    }

    // ── Snapshot path: plateau ladder ──────────────────────────────────────────

    /**
     * Plateau detection over every program slot with enough history. The stall length is
     * the number of bouts since the last meaningful e1RM improvement, so a recent PR
     * naturally resets it. Emits at most ONE recommendation per exercise — the highest
     * rung of the ladder the stall has reached:
     *
     *   stall ≥ [AdaptThresholds.plateauMinBouts]        → weight nudge (micro-load when
     *       effort has been low — likely sandbagging — or a ~10% reset when effort is high)
     *   stall ≥ [AdaptThresholds.repShiftAfterStalledBouts] → rep-range shift
     *   stall ≥ [AdaptThresholds.swapAfterStalledBouts]     → variation swap
     */
    fun evaluate(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<Recommendation> {
        val out = mutableListOf<Recommendation>()
        val seen = HashSet<String>()
        for (day in s.program) {
            for (slot in day.slots) {
                if (!seen.add(slot.exerciseId)) continue
                if (slot.unit == ExerciseUnit.BODYWEIGHT) continue
                val bouts = (s.exerciseHistory[slot.exerciseId] ?: continue)
                    .filter { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } }
                if (bouts.size <= t.plateauMinBouts) continue

                val e1rms = bouts.map { bestE1rm(it.sets) }
                var best = e1rms.first()
                var lastImprovedIdx = 0
                for (i in 1 until e1rms.size) {
                    if (e1rms[i] > best * (1 + t.stallTolerance)) {
                        best = e1rms[i]
                        lastImprovedIdx = i
                    }
                }
                val stall = e1rms.lastIndex - lastImprovedIdx
                if (stall < t.plateauMinBouts) continue

                val confidence = if (stall >= t.highConfidenceStall) Confidence.HIGH else Confidence.MEDIUM
                out += plateauSuggestion(slot, bouts, stall, confidence, s.prefs.plateLb, t)
            }
        }
        return out
    }

    private fun plateauSuggestion(
        slot: ProgramSlotSnap,
        bouts: List<ExerciseBout>,
        stall: Int,
        confidence: Confidence,
        plateLb: Double,
        t: AdaptThresholds
    ): Recommendation {
        val prevMax = bouts.last().sets
            .filter { it.weightLb != null && !it.isAssisted }
            .maxOf { it.weightLb!! }

        if (stall >= t.swapAfterStalledBouts && slot.swapCandidateIds.isNotEmpty()) {
            return Recommendation.VariationSwap(
                exerciseId = slot.exerciseId,
                exerciseName = slot.name,
                candidateIds = slot.swapCandidateIds.take(t.swapCandidateCount),
                reason = "No estimated-1RM gain on ${slot.name} in $stall sessions — a fresh variation can restart progress",
                confidence = confidence
            )
        }
        if (stall >= t.repShiftAfterStalledBouts) {
            val range = RepRange.parse(slot.repsText)
            val to = if (range != null && range.max >= 12) "8-10" else "12-15"
            return Recommendation.RepRangeShift(
                exerciseId = slot.exerciseId,
                exerciseName = slot.name,
                fromReps = slot.repsText,
                toReps = to,
                reason = "${slot.name} has stalled for $stall sessions — changing the rep range changes the stimulus",
                confidence = confidence
            )
        }

        // Effort signature over the recent window decides reset vs micro-load.
        val window = bouts.takeLast(minOf(stall, t.plateauMinBouts))
        val highEffortCount = window.count { b ->
            val rpe = b.sets.mapNotNull { it.rpe }.maxOrNull()
            (rpe != null && rpe >= 9.0) || b.effort == EffortRating.HARD || b.effort == EffortRating.BRUTAL
        }
        return if (highEffortCount >= t.highEffortCountInWindow) {
            val target = when (slot.unit) {
                ExerciseUnit.PLATES -> (prevMax - plateLb).coerceAtLeast(plateLb)
                else -> floorToGrid(prevMax * (1 - t.resetFraction), t.dumbbellStepLb)
            }
            weightChange(
                slot.exerciseId, slot.name, prevMax, target,
                inputText = inputTextFor(target, slot.unit, plateLb),
                reason = "${slot.name} stalled $stall sessions at high effort — drop ~${(t.resetFraction * 100).roundToInt()}% and build back up",
                confidence = confidence
            )
        } else {
            val target = when (slot.unit) {
                ExerciseUnit.PLATES -> prevMax + plateLb
                else -> floorToGrid(prevMax + t.dumbbellStepLb, t.dumbbellStepLb)
            }
            weightChange(
                slot.exerciseId, slot.name, prevMax, target,
                inputText = inputTextFor(target, slot.unit, plateLb),
                reason = "${slot.name} hasn't gained in $stall sessions but effort has stayed low — push a small increase",
                confidence = confidence
            )
        }
    }

    private fun bestE1rm(sets: List<LoggedSet>): Double =
        sets.filter { it.weightLb != null && !it.isAssisted }
            .maxOf { E1rm.epley(it.weightLb!!, it.reps) }

    // ── Shared helpers ─────────────────────────────────────────────────────────

    private fun intensityScale(intensity: IntensityIntent, t: AdaptThresholds): Double = when (intensity) {
        IntensityIntent.LIGHT -> t.intensityLightScale
        IntensityIntent.HARD -> t.intensityHardScale
        IntensityIntent.NORMAL -> 1.0
    }

    private fun withNote(reason: String, note: String?): String =
        if (note != null) "$reason · $note" else reason

    private fun weightChange(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        target: Double,
        inputText: String,
        reason: String,
        confidence: Confidence = Confidence.MEDIUM
    ) = Recommendation.WeightChange(
        exerciseId = exerciseId,
        exerciseName = exerciseName,
        previousMaxLb = prevMax,
        targetWeightLb = target,
        deltaLb = target - prevMax,
        inputText = inputText,
        reason = reason,
        confidence = confidence
    )

    private fun plateChange(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        targetLb: Double,
        plateLb: Double,
        reason: String
    ) = weightChange(
        exerciseId, exerciseName, prevMax, targetLb,
        inputText = inputTextFor(targetLb, ExerciseUnit.PLATES, plateLb),
        reason = reason
    )

    /**
     * Text for the weight field, in the exercise's input unit and parseable by WeightParser:
     * a plate count on PLATES ("3 plates"), explicit lb when off the plate grid ("50 lb"),
     * plain lb otherwise.
     */
    private fun inputTextFor(targetLb: Double, unit: ExerciseUnit, plateLb: Double): String {
        if (unit != ExerciseUnit.PLATES) return trim(targetLb)
        val plates = targetLb / plateLb
        val whole = plates.roundToInt()
        return if (kotlin.math.abs(plates - whole) < 0.01) {
            if (whole == 1) "1 plate" else "$whole plates"
        } else {
            "${trim(targetLb)} lb"
        }
    }

    /** Floor to the increment grid — matches the old suggestion's rounding exactly. */
    private fun floorToGrid(weight: Double, grid: Double): Double = (weight / grid).toInt() * grid

    private fun trim(v: Double): String = if (v % 1.0 == 0.0) "${v.toInt()}" else "$v"
}
