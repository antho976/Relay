package com.forge.app.program

/**
 * Planned weekly working sets per muscle, summed from the active program's slots. This is
 * the *committed* plan (whatever VolumeModel allocated at generation time, plus any manual
 * set edits) — so Stats can show actual-vs-plan against what the user actually agreed to,
 * not a re-derived ideal. Pure; testable with hand-built DayPlans.
 */
object VolumeTargets {

    fun plannedWeeklySetsByMuscle(days: List<DayPlan>): Map<MuscleGroup, Int> =
        days.flatMap { it.exercises }
            .groupBy { it.muscle }
            .mapValues { (_, slots) -> slots.sumOf { it.sets } }
}
