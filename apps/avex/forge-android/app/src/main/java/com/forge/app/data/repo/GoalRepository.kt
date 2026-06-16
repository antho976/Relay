package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.entities.ExerciseGoal
import com.forge.app.program.Program
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class GoalRepository @Inject constructor(
    private val goalDao: ExerciseGoalDao,
    private val loggedSetDao: LoggedSetDao,
    private val clock: Clock
) {
    fun observe(exerciseId: String): Flow<ExerciseGoal?> = goalDao.observe(exerciseId)
    fun observeAll(): Flow<List<ExerciseGoal>> = goalDao.observeAll()

    suspend fun get(exerciseId: String): ExerciseGoal? = goalDao.get(exerciseId)

    suspend fun setGoal(exerciseId: String, targetWeightLb: Double) {
        goalDao.upsert(ExerciseGoal(exerciseId = exerciseId, targetWeightLb = targetWeightLb, createdAt = clock.nowMs()))
    }

    suspend fun clearGoal(exerciseId: String) = goalDao.delete(exerciseId)

    /** One goal joined with the current best (heaviest set) logged for that exercise. */
    data class GoalProgress(
        val exerciseId: String,
        val name: String,
        val targetLb: Double,
        val currentBestLb: Double,
        val createdAt: Long
    ) {
        /** 0f–1f progress toward the target by heaviest set. */
        val fraction: Float get() = if (targetLb <= 0) 0f else (currentBestLb / targetLb).toFloat().coerceIn(0f, 1f)
        val achieved: Boolean get() = targetLb > 0 && currentBestLb >= targetLb
    }

    /** Every set goal with its progress, achieved-first then closest-first. Two queries, not N+1. */
    suspend fun goalsWithProgress(): List<GoalProgress> {
        val goals = goalDao.getAll()
        if (goals.isEmpty()) return emptyList()
        val bestByExercise = loggedSetDao.maxWeightPerExercise(goals.map { it.exerciseId })
            .associate { it.exerciseId to it.weightLb }
        return goals.map { g ->
            GoalProgress(
                exerciseId = g.exerciseId,
                // Falls back to the raw id only for a goal whose exercise was removed from the program;
                // such a goal stays clearable from the Goals screen's edit dialog.
                name = Program.exercise(g.exerciseId)?.name ?: g.exerciseId,
                targetLb = g.targetWeightLb,
                currentBestLb = bestByExercise[g.exerciseId] ?: 0.0,
                createdAt = g.createdAt
            )
        }.sortedWith(compareByDescending<GoalProgress> { it.achieved }.thenByDescending { it.fraction })
    }

    /** How many set goals have been reached — feeds the Goal Crusher trophy. */
    suspend fun achievedCount(): Int = goalsWithProgress().count { it.achieved }
}
