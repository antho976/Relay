package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.entities.ExerciseGoal
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class GoalRepository @Inject constructor(
    private val goalDao: ExerciseGoalDao,
    private val clock: Clock
) {
    fun observe(exerciseId: String): Flow<ExerciseGoal?> = goalDao.observe(exerciseId)

    suspend fun get(exerciseId: String): ExerciseGoal? = goalDao.get(exerciseId)

    suspend fun setGoal(exerciseId: String, targetWeightLb: Double) {
        goalDao.upsert(ExerciseGoal(exerciseId = exerciseId, targetWeightLb = targetWeightLb, createdAt = clock.nowMs()))
    }

    suspend fun clearGoal(exerciseId: String) = goalDao.delete(exerciseId)
}
