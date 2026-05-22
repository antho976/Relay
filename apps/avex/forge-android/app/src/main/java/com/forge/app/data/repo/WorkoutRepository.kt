package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The aggregate the day-screen ViewModel talks to. Wraps sessions, logged exercises,
 * sets, and mood entries — anything that's part of one workout's lifecycle.
 *
 * Stats / cross-session aggregates (frequency heatmap, weekly volume, strength curves)
 * live in a separate StatsRepository introduced in Phase 5. Keeping them out of here
 * stops this class from sprawling.
 */
@Singleton
class WorkoutRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val moodDao: MoodDao,
    private val clock: Clock
) {

    // ─── Session lifecycle ─────────────────────────────────────────────────────

    fun observeActiveSession(): Flow<Session?> = sessionDao.observeActiveSession()

    /**
     * Starts a new session, OR returns the id of the currently-active one if there
     * already is one. App invariant: at most one active session at a time.
     */
    suspend fun startOrResumeSession(dayKey: String): Long {
        sessionDao.getActiveSession()?.let { return it.id }
        return sessionDao.insert(
            Session(dayKey = dayKey, startedAt = clock.nowMs(), finishedAt = null)
        )
    }

    /**
     * Marks a session finished and stamps the denormalised volume + PR count so
     * later list views don't need to re-join exercises and sets.
     */
    suspend fun finishSession(sessionId: Long, totalVolumeLb: Double, prCount: Int) {
        val session = sessionDao.get(sessionId) ?: error("Session $sessionId not found")
        sessionDao.update(
            session.copy(
                finishedAt = clock.nowMs(),
                totalVolumeLb = totalVolumeLb,
                prCount = prCount
            )
        )
    }

    suspend fun discardSession(sessionId: Long) {
        val session = sessionDao.get(sessionId) ?: return
        sessionDao.delete(session) // CASCADE removes LoggedExercises and their LoggedSets
    }

    // ─── Logged exercises ──────────────────────────────────────────────────────

    fun observeExercisesForSession(sessionId: Long): Flow<List<LoggedExercise>> =
        loggedExerciseDao.observeForSession(sessionId)

    suspend fun addExerciseToSession(
        sessionId: Long,
        exerciseId: String,
        orderIndex: Int,
        swappedName: String? = null,
        swappedUnit: String? = null
    ): Long = loggedExerciseDao.insert(
        LoggedExercise(
            sessionId = sessionId,
            exerciseId = exerciseId,
            orderIndex = orderIndex,
            swappedName = swappedName,
            swappedUnit = swappedUnit
        )
    )

    suspend fun updateExercise(loggedExercise: LoggedExercise) =
        loggedExerciseDao.update(loggedExercise)

    suspend fun setRating(loggedExerciseId: Long, rating: EffortRating) {
        val ex = loggedExerciseDao.get(loggedExerciseId) ?: return
        loggedExerciseDao.update(ex.copy(difficulty = rating))
    }

    suspend fun setSkipped(loggedExerciseId: Long, skipped: Boolean) {
        val ex = loggedExerciseDao.get(loggedExerciseId) ?: return
        loggedExerciseDao.update(ex.copy(skipped = skipped))
    }

    suspend fun setNote(loggedExerciseId: Long, note: String?) {
        val ex = loggedExerciseDao.get(loggedExerciseId) ?: return
        loggedExerciseDao.update(ex.copy(note = note))
    }

    suspend fun lastLoggedExerciseBefore(exerciseId: String, excludeSessionId: Long): LoggedExercise? =
        loggedExerciseDao.lastLoggedBefore(exerciseId, excludeSessionId)

    // ─── Sets ──────────────────────────────────────────────────────────────────

    fun observeSets(loggedExerciseId: Long): Flow<List<LoggedSet>> =
        loggedSetDao.observeForLoggedExercise(loggedExerciseId)

    suspend fun setsFor(loggedExerciseId: Long): List<LoggedSet> =
        loggedSetDao.forLoggedExercise(loggedExerciseId)

    suspend fun logSet(
        loggedExerciseId: Long,
        setIndex: Int,
        weightText: String,
        weightLb: Double?,
        reps: Int
    ): Long = loggedSetDao.insert(
        LoggedSet(
            loggedExerciseId = loggedExerciseId,
            setIndex = setIndex,
            weightText = weightText,
            weightLb = weightLb,
            reps = reps,
            completedAt = clock.nowMs()
        )
    )

    suspend fun deleteSet(set: LoggedSet) = loggedSetDao.delete(set)

    /** All sets for this static exercise across every session. Feeds PR detection. */
    suspend fun historyForExercise(exerciseId: String): List<LoggedSet> =
        loggedSetDao.historyForExercise(exerciseId)

    suspend fun maxWeightForExercise(exerciseId: String): Double? =
        loggedSetDao.maxWeightForExercise(exerciseId)

    // ─── Mood ──────────────────────────────────────────────────────────────────

    suspend fun recordMood(sessionId: Long?, dayKey: String, mood: String) {
        moodDao.insert(
            MoodEntry(
                sessionId = sessionId,
                dayKey = dayKey,
                mood = mood,
                recordedAt = clock.nowMs()
            )
        )
    }
}
