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
    /** ua1 = DB Bench Press · ub2 = Incline DB Bench Press. */
    val BENCH_EXERCISE_IDS: List<String> = listOf("ua1", "ub2")

    /** la1 = Goblet Squat (Lower A) · lb4 = Goblet Squat (Lower B). */
    val SQUAT_EXERCISE_IDS: List<String> = listOf("la1", "lb4")
}
