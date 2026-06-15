package com.forge.app.domain.trophy

/**
 * Maps each "lift family" trophy rule to the static exercise ids that count toward it.
 * Bench / squat trophies aren't tied to one exercise — the user could trigger them via
 * the flat bench, the incline bench, or (for squat) the goblet variants on either lower
 * day. The lists stay narrow on purpose: the trophy *description* says "Bench pressed
 * 25 lb DBs" / "Goblet squatted 30 lb", so only the variants matching those names count.
 *
 * Kept out of [TrophyEvaluator] so the evaluator stays a pure function of its inputs
 * and these lists can be tweaked without touching rule logic.
 */
object TrophyExercises {
    /**
     * Bench-press variants that count toward the Bench Club trophy. Must list the LIBRARY ids
     * (`db-bench-press`, `incline-db-bench-press`): once the program is DB-backed, a logged row's
     * exercise_id is the library id (ProgramSlot.toPlan → ExerciseDef.id), not the seed slot code.
     * The old seed codes (`ua1`/`ub2`) are kept too so any pre-DB-load / legacy rows still count.
     */
    val BENCH_EXERCISE_IDS: List<String> =
        listOf("ua1", "ub2", "db-bench-press", "incline-db-bench-press")

    /** Goblet-squat variants for the Squat Club trophy. Library id `goblet-squat` + legacy seed codes. */
    val SQUAT_EXERCISE_IDS: List<String> =
        listOf("la1", "lb4", "goblet-squat")
}
