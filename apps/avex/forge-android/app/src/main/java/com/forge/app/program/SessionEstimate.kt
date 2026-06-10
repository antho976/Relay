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
    private const val COMPOUND_REST = 180
    private const val ISOLATION_REST = 90
    private const val HEAVY_REST_BONUS = 30

    /**
     * Compound vs isolation classification: generated plans carry tags; legacy/tagless
     * plans fall back to a muscle-size heuristic. Public so the adaptation engine's rest
     * tuning buckets by the same definition this estimate uses.
     */
    fun isCompound(plan: ExercisePlan): Boolean =
        if (plan.tags.isNotEmpty()) ExerciseTag.COMPOUND in plan.tags
        else plan.muscle in BIG_MUSCLES

    /** Recommended rest between sets (seconds): longer for compounds, +30s when the reps are heavy (≤8). */
    fun restSeconds(plan: ExercisePlan): Int {
        val base = if (isCompound(plan)) COMPOUND_REST else ISOLATION_REST
        // Heaviness = the LOW end of the range (the heaviest set you'd do). Using the max rep
        // meant strength ranges like "6-10" were never flagged heavy.
        val heavy = minReps(plan.reps)?.let { if (it <= 8) HEAVY_REST_BONUS else 0 } ?: 0
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

    /** Smallest number embedded in a rep string ("6-10"→6) — the heaviest set of the range. */
    private fun minReps(reps: String): Int? =
        Regex("\\d+").findAll(reps).map { it.value.toInt() }.minOrNull()
}
