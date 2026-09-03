package com.forge.app.domain.coach

import com.forge.app.domain.adapt.workingStrengthSets
import com.forge.app.domain.adapt.isWorkingStrengthSet
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.EffortModel
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.shared.weight.ProtocolWeightUnit
import com.forge.shared.weight.WeightSteps
import kotlin.math.roundToInt

/**
 * What today's session should actually ask of you (Coach v3 B2): per-exercise targets with the
 * intent behind them, computed BEFORE you start rather than chip by chip once you're already there.
 *
 * Three things v2 couldn't do:
 *  - **readiness shapes the whole session**, not one exercise at a time;
 *  - **soreness routes around itself** — a muscle you flagged this morning comes in lighter;
 *  - **a new or swapped exercise gets a starting weight** instead of a blank field, seeded from
 *    what you already lift on similar movements ("what weight do I start with?" is a
 *    Decision-Zero question, and v2 answered it with silence).
 *
 * Every prescription rounds to the shared weight-step table, so the coach can never ask for a
 * weight the bar can't hold.
 */
object PreSessionBrief {

    /**
     * @param targetWeightLb null for bodyweight movements and for exercises with no defensible
     *   number yet — silence beats a guess.
     * @param intent the one line explaining this target.
     * @param coldStart true when the weight is a seed from similar lifts, not this lift's history.
     * @param easedForSoreness true when the target was pulled back because the muscle was flagged.
     */
    data class ExerciseTarget(
        val exerciseId: String,
        val name: String,
        val muscle: MuscleGroup,
        val targetWeightLb: Double?,
        val setsText: String,
        val repsText: String,
        val intent: String,
        val coldStart: Boolean = false,
        val easedForSoreness: Boolean = false
    )

    data class Brief(
        val dayKey: String,
        val targets: List<ExerciseTarget>,
        /** The session-level line: what today is for. */
        val intent: String
    )

    /** How much a sore muscle's working weight backs off. */
    private const val SORENESS_SCALE = 0.9

    /**
     * Cold-start seed: a new movement starts at this fraction of what the athlete already handles
     * on the same muscle. Deliberately conservative — the first session on a lift is for learning
     * the groove, and an over-heavy first set is the one that ends in a bad rep.
     */
    private const val COLD_START_FRACTION = 0.7

    fun build(
        s: AdaptationSnapshot,
        dayKey: String,
        readiness: Recommendation.ReadinessScale?,
        life: LifeEvents.State,
        weightUnit: ProtocolWeightUnit = ProtocolWeightUnit.LB,
        /**
         * The active training block's phase, or null when there is no block (H-02).
         *
         * The brief is what the athlete reads BEFORE the session, and it was computed from
         * readiness and life events alone — so a Deload week and a Peak week opened with the same
         * numbers as an Accumulate one, contradicting the phase the Coach tab was showing them.
         */
        phase: BlockPhase? = null,
        t: AdaptThresholds = AdaptThresholds()
    ): Brief? {
        val day = s.program.firstOrNull { it.dayKey == dayKey } ?: return null
        val readinessScale = 1 + (readiness?.percent ?: 0) / 100.0
        // One composition rule, shared with the per-set suggestion, so the brief and the session
        // cannot describe two different days.
        val loadScale = BlockPhase.composedLoadScale(phase, readinessScale * life.loadScale)

        val targets = day.slots
            // An injured movement isn't "eased", it's off today's session entirely.
            .filterNot { life.isRestricted(it.exerciseId) || life.isRestricted(it.muscle) }
            .map { slot -> target(s, slot, loadScale, life, weightUnit, t) }

        return Brief(
            dayKey = dayKey,
            targets = targets,
            intent = sessionIntent(readiness, life)
        )
    }

    private fun target(
        s: AdaptationSnapshot,
        slot: ProgramSlotSnap,
        loadScale: Double,
        life: LifeEvents.State,
        weightUnit: ProtocolWeightUnit,
        t: AdaptThresholds
    ): ExerciseTarget {
        val sore = slot.muscle in life.soreMuscles
        val bouts = TodayDirective.trainingBouts(s, slot.exerciseId)
        val lastWorking = bouts.lastOrNull()?.sets
            ?.filter { it.isWorkingStrengthSet() && it.setType != EffortModel.SET_TYPE_WARMUP }
            ?.mapNotNull { it.weightLb }
            ?.maxOrNull()

        val isPlates = slot.unit == ExerciseUnit.PLATES
        val step = WeightSteps.weightStep(weightUnit, isPlates).let { if (isPlates) it * PLATE_LB else it }

        val (raw, coldStart) = when {
            slot.unit == ExerciseUnit.BODYWEIGHT -> null to false
            lastWorking != null -> lastWorking to false
            else -> seedFromSimilar(s, slot) to true
        }

        val scaled = raw?.let { base ->
            val adjusted = (if (sore) base * SORENESS_SCALE else base) * loadScale
            // An untouched weight is returned verbatim: it's a weight they actually lifted, and
            // snapping 47.5 to a 5 lb grid would silently move it. Only an ADJUSTED number gets
            // rounded — and always downward, because rounding a 5% hold back up to the same weight
            // would quietly ignore the readiness read that asked for it.
            if (!coldStart && kotlin.math.abs(adjusted - base) < 0.01) base
            else floorToStep(adjusted, step)
        }

        val intent = when {
            slot.unit == ExerciseUnit.BODYWEIGHT -> "Reps at the top of the range"
            coldStart && scaled != null -> "First time here, seeded from your similar lifts"
            coldStart -> "First time here, so find a weight you can control"
            sore -> "Eased while ${slot.muscle.displayName.lowercase()} is sore"
            loadScale < 0.99 -> "Held under today"
            loadScale > 1.01 -> "Push today"
            else -> "Same as last time, earn the reps"
        }

        return ExerciseTarget(
            exerciseId = slot.exerciseId,
            name = slot.name,
            muscle = slot.muscle,
            targetWeightLb = scaled,
            setsText = slot.targetSets.toString(),
            repsText = slot.repsText,
            intent = intent,
            coldStart = coldStart,
            easedForSoreness = sore
        )
    }

    /**
     * A starting weight for a movement with no history: the athlete's typical working weight on the
     * same muscle, backed off. Null when there's nothing comparable — better a blank field than a
     * number pulled from nowhere.
     */
    private fun seedFromSimilar(s: AdaptationSnapshot, slot: ProgramSlotSnap): Double? {
        val sameMuscleIds = s.program.flatMap { it.slots }
            .filter { it.muscle == slot.muscle && it.unit == slot.unit && it.exerciseId != slot.exerciseId }
            .map { it.exerciseId }
        val weights = sameMuscleIds.flatMap { id ->
            TodayDirective.trainingBouts(s, id).takeLast(3).flatMap { bout ->
                bout.sets.workingStrengthSets().mapNotNull { it.weightLb }
            }
        }
        if (weights.isEmpty()) return null
        return weights.average() * COLD_START_FRACTION
    }

    private fun sessionIntent(readiness: Recommendation.ReadinessScale?, life: LifeEvents.State): String = when {
        life.layoff?.returning == true -> "Easing back in, so nothing is chased today"
        life.soreMuscles.isNotEmpty() -> "Working around what you flagged as sore"
        (readiness?.percent ?: 0) <= -3 -> "Low readiness, so today holds rather than pushes"
        (readiness?.percent ?: 0) >= 3 -> "Readiness is high, so today is a good day to push"
        else -> "A normal session: earn the reps before the weight moves"
    }

    /** Floor to a loadable step, never below one step — the coach never asks for a phantom plate. */
    private fun floorToStep(value: Double, step: Double): Double {
        if (step <= 0) return value
        val snapped = kotlin.math.floor(value / step) * step
        return if (snapped < step) step else snapped
    }

    /** Plate exercises step in half-plates; the table speaks plates, the engine speaks pounds. */
    private const val PLATE_LB = 45.0
}
