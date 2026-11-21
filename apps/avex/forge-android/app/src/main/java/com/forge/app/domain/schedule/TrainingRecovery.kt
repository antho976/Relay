package com.forge.app.domain.schedule

import com.forge.app.program.DayPlan
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExerciseTag
import com.forge.app.program.MovementPattern
import com.forge.app.program.MuscleGroup
import java.time.LocalDate
import java.time.temporal.ChronoUnit

/** Conservative scheduling guard: a full intervening calendar day between muscle exposures.
 * Counts prime movers and substantial dynamic assistance, not every stabilising contraction.
 * It governs suggestions; people can still choose a different session themselves.
 */
object TrainingRecovery {
    fun muscles(day: DayPlan): Set<MuscleGroup> = day.exercises.flatMap { ex ->
        val def = ExerciseLibrary.byId(ex.id) ?: ExerciseLibrary.byName(ex.name)
        val primary = ex.muscle
        val secondary = if (ExerciseTag.COMPOUND !in (def?.tags ?: ex.tags)) emptySet() else when (def?.let(ExerciseLibrary::patternOf)) {
            MovementPattern.HORIZONTAL_PUSH -> setOf(MuscleGroup.CHEST, MuscleGroup.TRICEPS, MuscleGroup.SHOULDERS)
            MovementPattern.VERTICAL_PUSH -> setOf(MuscleGroup.SHOULDERS, MuscleGroup.TRICEPS)
            MovementPattern.HORIZONTAL_PULL -> setOf(MuscleGroup.BACK, MuscleGroup.BICEPS, MuscleGroup.REAR_DELTS)
            MovementPattern.VERTICAL_PULL -> setOf(MuscleGroup.BACK, MuscleGroup.BICEPS)
            MovementPattern.SQUAT, MovementPattern.LUNGE -> setOf(MuscleGroup.QUADS, MuscleGroup.GLUTES)
            MovementPattern.HINGE -> setOf(MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES)
            else -> emptySet()
        }
        secondary + primary
    }.toSet()

    /** Values are days from today until the next exposure may be suggested (0, 1 or 2).
     * Unknown historical keys (for example after regeneration) conservatively cool the whole plan.
     */
    fun daysUntilRecovered(
        days: List<DayPlan>,
        completed: List<Pair<String, LocalDate>>,
        today: LocalDate
    ): Map<String, Int> {
        val musclesByDay = days.associate { it.key to muscles(it) }
        return musclesByDay.mapValues { (_, muscles) ->
            completed.maxOfOrNull { (key, date) ->
                val age = ChronoUnit.DAYS.between(date, today)
                if (age !in 0..1) 0
                else if (musclesByDay[key]?.intersect(muscles)?.isEmpty() == true) 0
                else (2 - age).toInt()
            } ?: 0
        }
    }
}
