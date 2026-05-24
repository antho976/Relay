package com.forge.app.data.repo

import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import kotlinx.coroutines.flow.Flow
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages user-defined program customizations (#90, #91, #92).
 * The static [Program] is the source of truth; overrides from this repo are applied on top.
 */
@Singleton
class ProgramCustomizationRepository @Inject constructor(
    private val dao: ProgramCustomizationDao
) {
    fun observeForDay(dayKey: String): Flow<List<ProgramCustomization>> =
        dao.observeForDay(dayKey)

    /** Returns the effective exercise list for a day, applying all customizations. */
    suspend fun effectivePlanForDay(dayKey: String): List<ExercisePlan> {
        val day = Program.days.firstOrNull { it.key == dayKey } ?: return emptyList()
        val customizations = dao.forDay(dayKey)
        val customByExId = customizations.associateBy { it.exerciseId }

        val staticExercises: List<ExercisePlan> = day.exercises.mapNotNull { plan ->
            val c = customByExId[plan.id]
            if (c?.removed == true) return@mapNotNull null
            if (c == null) plan
            else plan.copy(
                reps = c.repRangeOverride ?: plan.reps,
                sets = if (c.setsOverride > 0) c.setsOverride else plan.sets
            )
        }

        // Added exercises (exerciseId starts with "custom_")
        val addedExercises: List<ExercisePlan> = customizations
            .filter { it.exerciseId.startsWith("custom_") && !it.removed }
            .map { c ->
                ExercisePlan(
                    id = c.exerciseId,
                    name = c.customName ?: "Custom",
                    sets = if (c.setsOverride > 0) c.setsOverride else 3,
                    reps = c.repRangeOverride ?: "8-10",
                    unit = com.forge.app.program.ExerciseUnit.DUMBBELL,
                    muscle = c.customMuscle?.let { runCatching { MuscleGroup.fromCode(it) }.getOrNull() }
                        ?: MuscleGroup.CHEST,
                    difficulty = com.forge.app.program.Difficulty.INTERMEDIATE,
                    note = ""
                )
            }

        // Merge and sort by orderOverride
        val allWithOrder = (staticExercises.mapIndexed { i, plan ->
            val order = customByExId[plan.id]?.orderOverride ?: (i * 10)
            order to plan
        } + addedExercises.map { plan ->
            val order = customByExId[plan.id]?.orderOverride ?: 999
            order to plan
        }).sortedBy { it.first }

        return allWithOrder.map { it.second }
    }

    /** Override rep range for an exercise (#90). */
    suspend fun setRepRange(dayKey: String, exerciseId: String, repRange: String) {
        val existing = dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }
        dao.upsert((existing ?: ProgramCustomization(dayKey, exerciseId)).copy(repRangeOverride = repRange))
    }

    /** Override set count for an exercise. */
    suspend fun setSetsOverride(dayKey: String, exerciseId: String, sets: Int) {
        val existing = dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }
        dao.upsert((existing ?: ProgramCustomization(dayKey, exerciseId)).copy(setsOverride = sets))
    }

    /** Remove an exercise from a day (#91). */
    suspend fun removeExercise(dayKey: String, exerciseId: String) {
        val existing = dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }
        dao.upsert((existing ?: ProgramCustomization(dayKey, exerciseId)).copy(removed = true))
    }

    /** Restore a removed exercise. */
    suspend fun restoreExercise(dayKey: String, exerciseId: String) {
        val existing = dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }
            ?: return
        dao.upsert(existing.copy(removed = false))
    }

    /** Add a custom exercise to a day (#91). */
    suspend fun addCustomExercise(
        dayKey: String,
        name: String,
        muscle: MuscleGroup,
        sets: Int = 3,
        repRange: String = "8-10"
    ): String {
        val id = "custom_${UUID.randomUUID().toString().take(8)}"
        dao.upsert(ProgramCustomization(
            dayKey = dayKey,
            exerciseId = id,
            customName = name,
            customMuscle = muscle.code,
            setsOverride = sets,
            repRangeOverride = repRange
        ))
        return id
    }

    /** Persist exercise order within a day (#92). */
    suspend fun setOrder(dayKey: String, exerciseId: String, order: Int) {
        val existing = dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }
        dao.upsert((existing ?: ProgramCustomization(dayKey, exerciseId)).copy(orderOverride = order))
    }

    /** Reset all customizations for a day. */
    suspend fun resetDay(dayKey: String) = dao.clearDay(dayKey)
}
