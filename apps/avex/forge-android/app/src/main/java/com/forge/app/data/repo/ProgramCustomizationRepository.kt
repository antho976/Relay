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

/** orderOverride sentinel meaning "no explicit position" (the ProgramCustomization default). */
private const val UNSET_ORDER = 999

/** One exercise in a day's editable plan — includes removed items (the training view filters them). */
data class EditableExercise(
    val plan: ExercisePlan,
    val removed: Boolean,
    val isCustom: Boolean
)

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

    /**
     * Full editable exercise list for a day (static + custom-added), INCLUDING removed items
     * (flagged). The training view uses [effectivePlanForDay], which drops removed items; editors
     * use this so a removed exercise can still be shown and restored.
     */
    suspend fun editablePlanForDay(dayKey: String): List<EditableExercise> {
        val day = Program.days.firstOrNull { it.key == dayKey } ?: return emptyList()
        val customizations = dao.forDay(dayKey)
        val customByExId = customizations.associateBy { it.exerciseId }

        // orderOverride defaults to the UNSET_ORDER sentinel ("no explicit position"); treat it as
        // unset so editing an exercise's reps/sets doesn't shove it to the bottom of the day.
        fun orderOf(exerciseId: String, fallback: Int): Int =
            customByExId[exerciseId]?.orderOverride?.takeIf { it != UNSET_ORDER } ?: fallback

        val staticExercises = day.exercises.mapIndexed { i, plan ->
            val c = customByExId[plan.id]
            val merged = if (c == null) plan else plan.copy(
                reps = c.repRangeOverride ?: plan.reps,
                sets = if (c.setsOverride > 0) c.setsOverride else plan.sets
            )
            orderOf(plan.id, i * 10) to EditableExercise(merged, removed = c?.removed == true, isCustom = false)
        }

        // Added exercises (exerciseId starts with "custom_") — no natural slot, so they sort after
        // the static ones in insert order. Each gets a distinct fallback, not a shared 999.
        val addedExercises = customizations
            .filter { it.exerciseId.startsWith("custom_") }
            .mapIndexed { j, c ->
                val plan = ExercisePlan(
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
                orderOf(c.exerciseId, 1000 + j) to EditableExercise(plan, removed = c.removed, isCustom = true)
            }

        return (staticExercises + addedExercises).sortedBy { it.first }.map { it.second }
    }

    /** Returns the effective exercise list for a day, applying all customizations (removed dropped). */
    suspend fun effectivePlanForDay(dayKey: String): List<ExercisePlan> =
        editablePlanForDay(dayKey).filterNot { it.removed }.map { it.plan }

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

    /** Reset all customizations for a day. */
    suspend fun resetDay(dayKey: String) = dao.clearDay(dayKey)

    // ─── Coach apply/undo support (auto-coach Phase 3) ────────────────────────

    /** The raw override row for one slot, if any — the coach reads before-state from here. */
    suspend fun overrideFor(dayKey: String, exerciseId: String): ProgramCustomization? =
        dao.forDay(dayKey).firstOrNull { it.exerciseId == exerciseId }

    /** Coach undo: drop a rep-range override (back to the plan's default). */
    suspend fun clearRepRange(dayKey: String, exerciseId: String) {
        val existing = overrideFor(dayKey, exerciseId) ?: return
        dao.upsert(existing.copy(repRangeOverride = null))
    }
}
