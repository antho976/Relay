package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.db.entities.UnlockedTrophy
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyExercises
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Persists which trophies have fired and runs the unlock evaluator. The catalogue itself
 * is static in [com.forge.app.program.Trophies]; rule evaluation is delegated to the
 * pure [TrophyEvaluator] so this class only handles I/O + the diff.
 */
@Singleton
class TrophyRepository @Inject constructor(
    private val unlockedDao: UnlockedTrophyDao,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val clock: Clock
) {
    fun observeAll(): Flow<List<UnlockedTrophy>> = unlockedDao.observeAll()

    fun observeUnlockedIds(): Flow<List<String>> = unlockedDao.observeUnlockedIds()

    suspend fun unlockedIds(): Set<String> = unlockedDao.unlockedIds().toSet()

    /** Records the unlock; IGNORE conflict policy means re-firing is a safe no-op. */
    suspend fun unlock(trophyId: String) {
        unlockedDao.unlock(UnlockedTrophy(trophyId = trophyId, unlockedAt = clock.nowMs()))
    }

    suspend fun unlockMany(trophyIds: Collection<String>) {
        val now = clock.nowMs()
        trophyIds.forEach { id ->
            unlockedDao.unlock(UnlockedTrophy(trophyId = id, unlockedAt = now))
        }
    }

    /**
     * Reads every counter / max the trophy rules look at. Each underlying query is a
     * single-row aggregate, so the fan-out is cheap. Called from [evaluateAndUnlockNew]
     * after `finishSession` and from the Trophies screen for progress hints.
     */
    suspend fun snapshot(): TrophyStatsSnapshot = TrophyStatsSnapshot(
        totalLoggedExercises = loggedExerciseDao.totalLogged(),
        totalPrs = loggedExerciseDao.prCount(),
        brutalRatings = loggedExerciseDao.countWithRating(EffortRating.BRUTAL),
        swapsUsed = loggedExerciseDao.swapCount(),
        fullTargetHits = loggedExerciseDao.fullTargetCount(),
        finishedSessions = sessionDao.finishedCount(),
        distinctDayKeysTrained = sessionDao.distinctDayKeysTrained().size,
        maxBenchLb = loggedSetDao.maxWeightAcrossExercises(TrophyExercises.BENCH_EXERCISE_IDS) ?: 0.0,
        maxSquatLb = loggedSetDao.maxWeightAcrossExercises(TrophyExercises.SQUAT_EXERCISE_IDS) ?: 0.0,
        maxSessionVolumeLb = loggedSetDao.maxSessionVolume() ?: 0.0
    )

    /**
     * Computes the current stats, runs every trophy rule, and persists any rows that
     * weren't already unlocked. Returns the newly-fired [Trophy] objects in catalogue
     * order so the caller (DayViewModel) can attach them to the session summary.
     */
    suspend fun evaluateAndUnlockNew(): List<Trophy> {
        val snapshot = snapshot()
        val satisfied = TrophyEvaluator.unlockedByRule(snapshot)
        val already = unlockedIds()
        val newlyUnlockedIds = satisfied - already
        if (newlyUnlockedIds.isEmpty()) return emptyList()
        unlockMany(newlyUnlockedIds)
        return Trophies.all.filter { it.id in newlyUnlockedIds }
    }
}
