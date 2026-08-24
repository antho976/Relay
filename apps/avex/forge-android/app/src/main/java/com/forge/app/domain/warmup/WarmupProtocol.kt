package com.forge.app.domain.warmup

import com.forge.app.program.ExerciseUnit

/**
 * The three phases of a warmup, in the order they must run.
 *
 * This is the RAMP protocol (Jeffreys 2006) with its Activate and Mobilise halves folded into one
 * user-facing phase, because at home-gym scale they are the same drills. The ordering is the whole
 * point: temperature first (cheap, systemic), then range of motion through the joints about to be
 * loaded, then task-specific rehearsal at rising load. Reversing any two wastes the phase before it.
 */
enum class WarmupPhase(val label: String) {
    /** General pulse raiser. Raises muscle temperature, heart rate and blood flow. */
    RAISE("Raise"),

    /** Dynamic range-of-motion work through the joints this session loads. Never static holds. */
    MOBILIZE("Mobilize"),

    /** Specific rehearsal: the actual movement at rising load, up to the working set. */
    RAMP("Ramp")
}

/**
 * One thing the user does, in order. [seconds] is the time allowance (work plus any rest that
 * follows) and only feeds the protocol's total estimate.
 */
sealed interface WarmupStep {
    val id: String
    val phase: WarmupPhase
    val seconds: Int
}

/**
 * A raise or mobilise drill. [prescription] is the dose ("10 each side"), [why] the one-line reason
 * it earns a place. Both render, so both obey the copy rules: dry, imperative, no em dashes.
 */
data class WarmupDrill(
    override val id: String,
    override val phase: WarmupPhase,
    val name: String,
    val prescription: String,
    val why: String,
    override val seconds: Int
) : WarmupStep

/**
 * One specific-warmup set on a real lift.
 *
 * [load] is in the exercise's OWN input scale, matching what the user types on the set row: a plate
 * count on [ExerciseUnit.PLATES], raw pounds otherwise, and null on bodyweight or when no working
 * weight is known yet. [percentOfWorking] is of the working LOAD, not of 1RM, because that is the
 * number the user can act on without knowing their max.
 */
data class WarmupRampSet(
    override val id: String,
    val exerciseName: String,
    val unit: ExerciseUnit,
    val load: Double?,
    val reps: Int,
    val percentOfWorking: Int,
    val restSeconds: Int,
    override val seconds: Int
) : WarmupStep {
    override val phase: WarmupPhase get() = WarmupPhase.RAMP
}

/**
 * A whole warmup, ready to step through. [steps] is already in performance order, so the UI never
 * sorts or groups it.
 */
data class WarmupProtocol(val steps: List<WarmupStep>) {

    val isEmpty: Boolean get() = steps.isEmpty()

    /** Total time allowance across every step, in seconds. */
    val totalSeconds: Int get() = steps.sumOf { it.seconds }

    /** Rounded minutes for the header meta. Always at least 1 when there is anything to do. */
    val totalMinutes: Int get() = if (isEmpty) 0 else ((totalSeconds + 30) / 60).coerceAtLeast(1)

    companion object {
        val EMPTY = WarmupProtocol(emptyList())
    }
}
