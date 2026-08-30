package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CoachGoalDao
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.dao.ExtendedGoalDao
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.coach.GoalPortfolio
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The Goal Portfolio's data layer (Coach v3 A2).
 *
 * **Additive by design.** `exercise_goal` and `extended_goal` keep working exactly as they do — the
 * Goals screen, the cardio hub and the `goal_crusher` / `goals_5` trophies all still read them.
 * This repository never migrates or deletes those rows; [promotionCandidates] simply offers to
 * mirror one into a coach goal, and the user decides. Nothing the portfolio does can regress a
 * trophy or move someone's existing goals.
 */
@Singleton
class CoachGoalRepository @Inject constructor(
    private val coachGoalDao: CoachGoalDao,
    private val exerciseGoalDao: ExerciseGoalDao,
    private val extendedGoalDao: ExtendedGoalDao,
    private val adaptationRepository: AdaptationRepository,
    private val clock: Clock
) {

    fun observeActive(): Flow<List<CoachGoal>> = coachGoalDao.observeActive()

    suspend fun active(): List<CoachGoal> = coachGoalDao.active()

    suspend fun all(): List<CoachGoal> = coachGoalDao.all()

    /**
     * Add a goal. New goals land at the END of the priority order — the coach never silently
     * demotes what the user already said mattered most.
     */
    suspend fun add(
        kind: CoachGoalKind,
        targetKey: String = "",
        targetValue: Double? = null,
        source: String = SOURCE_USER,
        note: String = ""
    ): Long {
        val nextPriority = (coachGoalDao.active().maxOfOrNull { it.priority } ?: -1) + 1
        return coachGoalDao.insert(
            CoachGoal(
                kind = kind.code,
                targetKey = targetKey,
                targetValue = targetValue,
                priority = nextPriority,
                createdAt = clock.nowMs(),
                source = source,
                note = note
            )
        )
    }

    suspend fun archive(id: Long) = coachGoalDao.markArchived(id, clock.nowMs())

    suspend fun complete(id: Long) = coachGoalDao.markCompleted(id, clock.nowMs())

    suspend fun delete(id: Long) = coachGoalDao.delete(id)

    /** Re-rank: the list's order becomes the priority order, index 0 = the block's focus goal. */
    suspend fun reorder(idsInOrder: List<Long>) {
        idsInOrder.forEachIndexed { index, id -> coachGoalDao.setPriority(id, index) }
    }

    /** Every active goal with its live reading, trajectory and ETA. */
    suspend fun states(): List<GoalPortfolio.GoalState> =
        GoalPortfolio.evaluate(coachGoalDao.active(), adaptationRepository.snapshotOrEmpty())

    /** Conflicting pairs among the active goals, each with the coach's sequencing proposal. */
    suspend fun conflicts(): List<GoalPortfolio.GoalConflict> =
        // With the snapshot, so a bodyweight goal's direction comes from its target against the
        // athlete's actual weight rather than only from whatever the note happens to say.
        GoalPortfolio.conflicts(coachGoalDao.active(), adaptationRepository.snapshotOrEmpty())

    /**
     * Goals that reached their target since the last check. The lifecycle moment the plan asks
     * for: celebrate, archive, propose a successor. Marks them completed so the moment fires once.
     */
    suspend fun harvestCompleted(): List<GoalPortfolio.GoalState> {
        val reached = states().filter { it.reachedNow }
        reached.forEach { coachGoalDao.markCompleted(it.goal.id, clock.nowMs()) }
        return reached
    }

    /**
     * Existing app goals that could become coach goals, as [kind, targetKey, targetValue] triples
     * the UI can offer one tap at a time. Reads the legacy tables; writes nothing.
     */
    suspend fun promotionCandidates(): List<Candidate> {
        val existing = coachGoalDao.all()
        fun alreadyTracked(kind: CoachGoalKind, key: String) =
            existing.any { it.kind == kind.code && it.targetKey == key && it.isActive }

        val out = mutableListOf<Candidate>()
        exerciseGoalDao.getAll().forEach { g ->
            if (!alreadyTracked(CoachGoalKind.LIFT_1RM, g.exerciseId)) {
                out += Candidate(CoachGoalKind.LIFT_1RM, g.exerciseId, g.targetWeightLb)
            }
        }
        extendedGoalDao.getAll().filter { it.completedAt == null }.forEach { g ->
            val kind = when {
                g.goalType == "1rm" -> CoachGoalKind.LIFT_1RM
                g.goalType == "frequency" -> CoachGoalKind.CONSISTENCY
                g.goalType.startsWith("cardio_minutes") -> CoachGoalKind.CONDITIONING
                g.goalType.startsWith("bodyweight") -> CoachGoalKind.BODYWEIGHT
                else -> null
            } ?: return@forEach
            val key = g.exerciseId.orEmpty()
            if (!alreadyTracked(kind, key)) out += Candidate(kind, key, g.targetValue)
        }
        return out
    }

    data class Candidate(val kind: CoachGoalKind, val targetKey: String, val targetValue: Double?)

    companion object {
        const val SOURCE_USER = "user"
        const val SOURCE_COACH = "coach"
    }
}
