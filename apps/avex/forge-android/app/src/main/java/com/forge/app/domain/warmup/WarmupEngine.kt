package com.forge.app.domain.warmup

import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import kotlin.math.roundToInt

/**
 * One exercise as the warmup engine needs to see it.
 *
 * [workingLoad] is in the exercise's OWN input scale, matching what the set row accepts: a plate
 * count on [ExerciseUnit.PLATES], raw pounds otherwise. Null means bodyweight, or a first-ever
 * exercise with no history and no suggestion, and the engine degrades to an unloaded feeler set
 * rather than inventing a number.
 */
data class WarmupExercise(
    val id: String,
    val name: String,
    val muscle: MuscleGroup,
    val unit: ExerciseUnit,
    val isCompound: Boolean,
    val workingLoad: Double?,
    val targetReps: Int,
    /**
     * Smallest loadable change, in the same scale as [workingLoad]. Null derives it from [unit] on
     * the imperial grid. The caller passes it explicitly so a kg user gets prescriptions that round
     * to whole kilos, rather than to pounds that render as 22.7 kg.
     */
    val loadStep: Double? = null
)

/**
 * Builds a session's warmup from what the user is actually about to lift.
 *
 * The old warmup was four fixed strings per program day: the same jumping jacks whether the day was
 * heavy squats or lateral raises, and a separate suggester that always proposed 40/60/80% regardless
 * of load. Both halves are replaced here by prescriptions that move with the session.
 *
 * ### What the evidence actually supports
 *
 * - **Specific beats general.** The transferable part of a warmup is rehearsing the task at rising
 *   load. General movement raises temperature and little else, so it is kept short and the ramp
 *   carries the weight (Fradkin 2010; Ribeiro 2020).
 * - **Temperature is worth about 2 to 5% of force output** per degree of muscle temperature, which is
 *   why the raise phase exists at all, and why two to three minutes is the whole dose (Bishop 2003).
 * - **Static stretching costs force.** Held stretches before lifting reduce strength output, scaling
 *   with hold time (Simic 2013). Nothing this engine emits is a static hold. See [MobilityCatalog].
 * - **Ramp depth scales with intensity.** A set at 90% of 1RM needs more rungs beneath it than a set
 *   at 65%, both to prepare the tissue and to calibrate the groove. A fixed three-set ramp is too
 *   much for a 15-rep isolation and too little for a heavy triple.
 * - **A warmup set is never hard.** Reps fall as the load climbs, so the ramp arrives at the working
 *   set without having spent anything on it.
 * - **The effect decays.** Elevated muscle temperature dissipates over roughly 15 to 20 minutes, so
 *   ramp sets belong immediately before their own exercise. This is why [build] ramps only the lift
 *   the user is about to start, and why later lifts get their ramp from [rampFor] on their own card
 *   rather than in the pre-session gate.
 */
object WarmupEngine {

    // ── Intensity ────────────────────────────────────────────────────────────────

    /**
     * The fraction of 1RM implied by a working set of [reps], inverted from the Epley relation the
     * rest of the app already estimates 1RMs with (`E1rm.epley`). Sharing that convention matters
     * more than picking the marginally better formula: a ramp derived from one model and a plateau
     * call derived from another would disagree about the same set.
     *
     * Epley gives `1RM = w × (1 + reps/30)`, so `w / 1RM` collapses to a function of reps alone.
     * 5 reps lands at 86%, 10 at 75%, 15 at 67%, which matches the standard load-rep charts closely
     * enough for choosing how many warmup sets to do.
     */
    fun intensityOf(reps: Int): Double =
        // Epley pins reps <= 1 to the lift itself, so a working single is 100% of 1RM by definition.
        // Mirroring that branch keeps the two in step at the top of the range as well as the middle.
        if (reps <= 1) 1.0 else 1.0 / (1.0 + reps / 30.0)

    // ── Ramp shape ───────────────────────────────────────────────────────────────

    /**
     * Ramp fractions of the working load, by rung count. Two properties are deliberate:
     * the ladder starts higher when it is short (one rung at 55% is a better single warmup than one
     * at 40%), and the gaps compress at the top, so the last jump into the working set is the
     * smallest one. That final small step is what makes the working set feel calibrated instead of
     * sudden.
     */
    private val RAMP_FRACTIONS: Map<Int, List<Double>> = mapOf(
        1 to listOf(0.55),
        2 to listOf(0.50, 0.75),
        3 to listOf(0.45, 0.65, 0.85),
        4 to listOf(0.40, 0.60, 0.75, 0.90),
        5 to listOf(0.40, 0.55, 0.70, 0.83, 0.93)
    )

    /**
     * How many ramp sets a lift earns.
     *
     * [alreadyWarm] is true when an earlier exercise in the same session already ramped this muscle
     * group. That collapses the ramp to at most a single feeler set, because the tissue is warm and
     * the pattern is fresh: repeating a full ladder there is junk volume taken out of the working
     * sets. It is the piece that makes the warmup session-aware rather than per-exercise.
     */
    internal fun rampSetCount(intensity: Double, isCompound: Boolean, alreadyWarm: Boolean): Int = when {
        // Warm muscle, fresh pattern. Only genuinely heavy work still needs a rung to find the groove.
        alreadyWarm -> if (isCompound && intensity >= 0.80) 1 else 0
        // Single-joint work is low absolute load on a muscle the compounds already moved blood through.
        !isCompound -> if (intensity >= 0.80) 2 else 1
        // Set clear of 0.60 rather than on it: 20 reps evaluates to 0.6000000000000001 in binary
        // floating point, so a boundary at exactly 0.60 would put the lightest work on two rungs.
        // Nothing sits between 20 reps (0.600) and 15 reps (0.667), so the gap is free to take.
        intensity < 0.62 -> 1
        intensity < 0.70 -> 2
        intensity < 0.80 -> 3
        intensity < 0.90 -> 4
        else -> 5
    }

    /**
     * Reps for a ramp set, from its ABSOLUTE intensity (`fraction × workingIntensity`) rather than
     * from its share of the working load.
     *
     * The distinction is the whole point. 86% of a 70 lb dumbbell bench done for 8 is only about 68%
     * of a true 1RM, which is a comfortable set of five, not a double. Keying reps off the fraction
     * alone prescribed near-maximal-looking warmup doubles for light high-rep work, and too many
     * reps under a genuinely heavy single. Absolute intensity is what the tissue responds to.
     */
    internal fun rampReps(fraction: Double, workingIntensity: Double): Int {
        // Strictly-less comparisons rather than closed ranges, and bands tuned so a heavy ramp
        // actually tapers: 315 for a triple comes out 8 / 5 / 4 / 2 / 1, which is how that warmup is
        // run in practice. Slower bands drew eights twice in a row, warmup volume spent for nothing.
        val absolute = fraction * workingIntensity
        return when {
            absolute < 0.45 -> 8
            absolute < 0.55 -> 5
            absolute < 0.65 -> 4
            absolute < 0.73 -> 3
            absolute < 0.82 -> 2
            else -> 1
        }
    }

    /**
     * Rest after a ramp set, likewise from absolute intensity. Short at the bottom (the point is
     * blood flow, not recovery) and long enough under a genuinely heavy rung that it does not eat
     * into the working set.
     */
    internal fun rampRest(fraction: Double, workingIntensity: Double): Int {
        val absolute = fraction * workingIntensity
        return when {
            absolute < 0.60 -> 30
            absolute < 0.75 -> 45
            else -> 75
        }
    }

    /**
     * The smallest load change that can actually be loaded, expressed in the exercise's own input
     * scale (pounds, or plates on [ExerciseUnit.PLATES]). Rounding to this keeps every prescription
     * something the user can literally set up: adjustable dumbbells move a fixed grid step, plate
     * machines move a whole plate, barbells and stacks move the smallest pair of change plates.
     *
     * [metric] switches the grid to the metric gym's denominations. Because loads are carried in
     * pounds, the metric steps are converted, which is what makes a kg prescription land on a whole
     * kilo instead of on 22.7.
     */
    internal fun loadIncrement(unit: ExerciseUnit, metric: Boolean = false): Double = when (unit) {
        ExerciseUnit.DUMBBELL -> if (metric) 2.5 / KG_PER_LB else 5.0
        ExerciseUnit.PLATES -> 1.0
        ExerciseUnit.WEIGHT -> if (metric) 2.5 / KG_PER_LB else 2.5
        ExerciseUnit.BODYWEIGHT -> 0.0
    }

    private const val KG_PER_LB = 0.45359237

    /**
     * Ramp sets for one exercise, or empty when it has not earned any.
     *
     * Rounding is to the nearest loadable increment, then clamped to at least one increment so a
     * light isolation never prescribes a zero-pound warmup. Rungs that round onto the working load
     * itself are dropped: a "warmup set" at the working weight is the working set.
     */
    fun rampFor(exercise: WarmupExercise, alreadyWarm: Boolean = false): List<WarmupRampSet> {
        val intensity = intensityOf(exercise.targetReps)
        val count = rampSetCount(intensity, exercise.isCompound, alreadyWarm)
        if (count == 0) return emptyList()

        val working = exercise.workingLoad
        // Bodyweight, or no load known yet. A compound still deserves one unloaded rehearsal set;
        // guessing a number the user never gave us would be worse than prescribing none.
        if (working == null || working <= 0.0 || exercise.unit == ExerciseUnit.BODYWEIGHT) {
            if (!exercise.isCompound || alreadyWarm) return emptyList()
            val reps = (exercise.targetReps / 2).coerceIn(3, 8)
            return listOf(
                WarmupRampSet(
                    id = "ramp-${exercise.id}-feel",
                    exerciseName = exercise.name,
                    unit = exercise.unit,
                    load = null,
                    reps = reps,
                    percentOfWorking = 0,
                    restSeconds = 30,
                    seconds = RAMP_WORK_SECONDS + 30
                )
            )
        }

        val increment = exercise.loadStep ?: loadIncrement(exercise.unit)
        val fractions = RAMP_FRACTIONS.getValue(count)
        val out = mutableListOf<WarmupRampSet>()
        var lastLoad = 0.0
        fractions.forEachIndexed { index, fraction ->
            val rounded = (roundTo(working * fraction, increment)).coerceAtLeast(increment)
            // Drop a rung that rounds onto the working load, or onto the rung below it. On light
            // exercises with a coarse increment several fractions collapse to the same loadable
            // weight, and repeating it adds fatigue without adding preparation.
            if (rounded >= working || rounded <= lastLoad) return@forEachIndexed
            lastLoad = rounded
            // Measure the rung against the load actually prescribed, not the ideal fraction, so
            // rounding onto a coarse grid cannot leave the reps describing a different set.
            val effectiveFraction = rounded / working
            val rest = rampRest(effectiveFraction, intensity)
            out += WarmupRampSet(
                id = "ramp-${exercise.id}-$index",
                exerciseName = exercise.name,
                unit = exercise.unit,
                load = rounded,
                reps = rampReps(effectiveFraction, intensity),
                percentOfWorking = ((rounded / working) * 100).roundToInt(),
                restSeconds = rest,
                seconds = RAMP_WORK_SECONDS + rest
            )
        }
        return out
    }

    // ── Whole protocol ───────────────────────────────────────────────────────────

    /**
     * The pre-session warmup: one raise drill, the mobilise drills the trained joints earn, and the
     * ramp for the FIRST exercise only.
     *
     * Ramping only the first lift is the decay point above, not a simplification. Sets for exercise
     * four would be performed twenty minutes before that exercise starts, by which time the effect
     * has dissipated and the only thing left is the fatigue. Every later lift gets its ramp from
     * [rampFor] at the moment it comes up.
     *
     * [customDrills] are the user's own warmup lines, which replace the generated raise and mobilise
     * drills entirely. Their ramp is still generated: the user chose their mobility work, not their
     * arithmetic.
     */
    fun build(
        exercises: List<WarmupExercise>,
        customDrills: List<String>? = null
    ): WarmupProtocol {
        val steps = mutableListOf<WarmupStep>()

        if (customDrills != null) {
            if (customDrills.isEmpty() && exercises.isEmpty()) return WarmupProtocol.EMPTY
            customDrills.forEachIndexed { index, line ->
                steps += WarmupDrill(
                    id = "custom-$index",
                    phase = WarmupPhase.MOBILIZE,
                    name = line,
                    prescription = "",
                    why = "",
                    seconds = CUSTOM_DRILL_SECONDS
                )
            }
        } else {
            if (exercises.isEmpty()) return WarmupProtocol.EMPTY
            steps += raiseFor(exercises)
            // Rank trained groups by how many exercises hit them, so the cap keeps the joint the
            // session leans on hardest and drops the one it barely touches.
            val ranked = exercises
                .groupingBy { it.muscle }
                .eachCount()
                .entries
                .sortedByDescending { it.value }
                .map { it.key }
            steps += MobilityCatalog.forMuscles(ranked)
        }

        exercises.firstOrNull()?.let { steps += rampFor(it, alreadyWarm = false) }
        return WarmupProtocol(steps)
    }

    /**
     * The pulse raiser. Lower-body sessions get the longer dose: more muscle mass to bring up to
     * temperature, and the hips and knees are about to take the session's heaviest loads. Both doses
     * are deliberately brief. Most of the temperature effect arrives early, the ramp sets carry the
     * rest, and every extra minute here is one the user spends not lifting.
     */
    private fun raiseFor(exercises: List<WarmupExercise>): WarmupDrill {
        val lower = exercises.count { it.muscle in LOWER_BODY } * 2 >= exercises.size
        return if (lower) {
            WarmupDrill(
                id = "raise-lower",
                phase = WarmupPhase.RAISE,
                name = "Bike, brisk walk, or jumping jacks",
                prescription = "90s easy",
                why = "Warm muscle produces more force",
                seconds = 90
            )
        } else {
            WarmupDrill(
                id = "raise-upper",
                phase = WarmupPhase.RAISE,
                name = "Jumping jacks or arm swings",
                prescription = "60s easy",
                why = "Raises muscle temperature before anything loads",
                seconds = 60
            )
        }
    }

    private val LOWER_BODY = setOf(
        MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.CALVES
    )

    /** Time a ramp set takes to perform, before its rest. */
    private const val RAMP_WORK_SECONDS = 25

    /** Allowance for a user-written warmup line, which carries no dose we can read. */
    private const val CUSTOM_DRILL_SECONDS = 45

    private fun roundTo(value: Double, increment: Double): Double =
        if (increment <= 0.0) value else (value / increment).roundToInt() * increment
}
