package com.forge.app.program

/**
 * Maps the user's training inputs onto generation policy (program-unlock Phase 2 of the
 * "professional plans" roadmap): the onboarding **goal** ([com.forge.app.data.prefs] `USER_GOAL`)
 * reshapes rep ranges, and **experience** scales volume + filters movement difficulty. These are the
 * only two knobs here; both are pure functions so the generator stays testable.
 */
object GoalProfiles {

    /**
     * Rep range for a slot's [scheme], shifted by [goal]:
     * - `get_stronger` → lower (heavier),
     * - `lose_weight` → higher (more conditioning),
     * - `build_muscle` / `general_fitness` / anything else → the scheme's default hypertrophy range.
     */
    fun reps(goal: String, scheme: RepScheme): String = when (goal) {
        "get_stronger" -> when (scheme) {
            RepScheme.STRENGTH -> "4-6"
            RepScheme.HYPERTROPHY -> "6-8"
            RepScheme.PUMP -> "8-12"
        }
        "lose_weight" -> when (scheme) {
            RepScheme.STRENGTH -> "8-12"
            RepScheme.HYPERTROPHY -> "12-15"
            RepScheme.PUMP -> "15-20"
        }
        else -> scheme.reps
    }

    /** Highest movement difficulty a given experience level should be handed. Beginners skip ADVANCED. */
    fun maxDifficulty(experience: String): Difficulty = when (experience) {
        "beginner" -> Difficulty.INTERMEDIATE
        else -> Difficulty.ADVANCED
    }

    /** Per-set volume multiplier — beginners do less, advanced do a touch more. */
    fun volumeFactor(experience: String): Double = when (experience) {
        "beginner" -> 0.8
        "advanced" -> 1.15
        else -> 1.0
    }
}
