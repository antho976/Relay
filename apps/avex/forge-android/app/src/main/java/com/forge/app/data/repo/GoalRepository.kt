package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.entities.ExerciseGoal
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.program.Program
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class GoalRepository @Inject constructor(
    private val goalDao: ExerciseGoalDao,
    private val loggedSetDao: LoggedSetDao,
    private val settingsRepository: com.forge.app.data.prefs.SettingsRepository,
    private val clock: Clock
) {
    fun observe(exerciseId: String): Flow<ExerciseGoal?> = goalDao.observe(exerciseId)
    fun observeAll(): Flow<List<ExerciseGoal>> = goalDao.observeAll()

    suspend fun get(exerciseId: String): ExerciseGoal? = goalDao.get(exerciseId)

    suspend fun setGoal(exerciseId: String, targetWeightLb: Double) {
        goalDao.upsert(ExerciseGoal(exerciseId = exerciseId, targetWeightLb = targetWeightLb, createdAt = clock.nowMs()))
    }

    /**
     * Delete the lift target for [exerciseId], and the Home pin that pointed at it (L-06).
     *
     * The cleanup lives HERE, not in one ViewModel, because there are two ways to clear a lift
     * goal — the Goals screen and the workout screen's goal setter — and only the first knew to do
     * it. The pin left behind by the second is invisible on Home (Home resolves keys against live
     * goals) but still counts as one of the three slots, so the next pin evicts a live one to make
     * room and Home quietly shows two goals in three slots.
     *
     * @return the position the pin held, or -1 when the goal was not pinned — so a caller offering
     *   Undo can put it back where the user had it.
     */
    suspend fun clearGoal(exerciseId: String): Int {
        goalDao.delete(exerciseId)
        return settingsRepository.removeGoalPin(liftPinKey(exerciseId))
    }

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
                // Resolves through the active program, the seed split, then the library; a goal whose
                // exercise was removed entirely still shows a readable (humanized) name, never a raw id,
                // and stays clearable from the Goals screen's edit dialog (C3).
                name = Program.exerciseDisplayName(g.exerciseId),
                targetLb = g.targetWeightLb,
                currentBestLb = bestByExercise[g.exerciseId] ?: 0.0,
                createdAt = g.createdAt
            )
        }.sortedWith(compareByDescending<GoalProgress> { it.achieved }.thenByDescending { it.fraction })
    }

    /** How many set goals have been reached — feeds the Goal Crusher trophy. */
    suspend fun achievedCount(): Int = goalsWithProgress().count { it.achieved }
}
