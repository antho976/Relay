package com.forge.app.program

import kotlin.math.roundToInt

/**
 * Rest recommendations + session-length estimates (program-unlock "professional plans" Phase 4 —
 * session shaping). Pure functions over a [DayPlan]/[ExercisePlan] so they're usable from both the
 * rest timer and the day-list UI, and unit-testable. Rest scales with movement type (compounds need
 * longer) and how heavy the reps are; the time estimate is deliberately rough (rounded to 5 min).
 */
object SessionEstimate {

    private val BIG_MUSCLES = setOf(
        MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES
    )
    private const val WORK_SECONDS_PER_SET = 45
    private const val WARMUP_SECONDS = 300
    /** Canonical rest bases (seconds) — also the defaults a user's Session-settings override falls back to. */
    const val COMPOUND_REST = 120
    const val ISOLATION_REST = 90
    private const val HEAVY_REST_BONUS = 60

    /**
     * Compound vs isolation classification: generated plans carry tags; legacy/tagless
     * plans fall back to a muscle-size heuristic. Public so the adaptation engine's rest
     * tuning buckets by the same definition this estimate uses.
     */
    fun isCompound(plan: ExercisePlan): Boolean =
        if (plan.tags.isNotEmpty()) ExerciseTag.COMPOUND in plan.tags
        else plan.muscle in BIG_MUSCLES

    /**
     * Recommended rest: 2 minutes for compounds, 90 seconds for isolation, +60s for heavy compounds.
     * [compoundBase]/[isolationBase] default to the canonical values; the rest timer passes the user's
     * Session-settings overrides so a "I rest 4 min on compounds" preference flows through everywhere.
     */
    fun restSeconds(
        plan: ExercisePlan,
        compoundBase: Int = COMPOUND_REST,
        isolationBase: Int = ISOLATION_REST
    ): Int {
        val base = if (isCompound(plan)) compoundBase else isolationBase
        // Only genuinely low-rep prescriptions earn extra rest. An 8-12 range, timed hold,
        // or per-side notation does not establish heavy loading.
        val heavy = if (isCompound(plan) && isHeavy(plan.reps)) HEAVY_REST_BONUS else 0
        return base + heavy
    }

    /**
     * Rough whole-session time in minutes (work + between-set rest + a warmup allowance),
     * rounded to 5. [restFor] defaults to the canonical [restSeconds]; the adaptation
     * engine passes a personally-tuned variant so the day-card "~min" matches the user's
     * actual pace once enough realized-rest data exists.
     */
    fun estimateMinutes(plan: DayPlan, restFor: (ExercisePlan) -> Int = ::restSeconds): Int {
        if (plan.exercises.isEmpty()) return 0
        val workSeconds = plan.exercises.sumOf { ex ->
            ex.sets * WORK_SECONDS_PER_SET + (ex.sets - 1).coerceAtLeast(0) * restFor(ex)
        }
        val minutes = ((workSeconds + WARMUP_SECONDS) / 60.0).roundToInt()
        return ((minutes + 2) / 5) * 5 // nearest 5
    }

    private fun isHeavy(reps: String): Boolean {
        val match = Regex("""^(\d+)(?:-(\d+))?$""").matchEntire(reps.trim()) ?: return false
        val upper = match.groupValues[2].ifEmpty { match.groupValues[1] }.toIntOrNull() ?: return false
        return upper in 1..6
    }
}
