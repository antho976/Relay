package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import com.forge.app.data.db.projections.SetWithExerciseId
import kotlinx.coroutines.flow.Flow

@Dao
interface LoggedSetDao {

    @Insert
    suspend fun insert(set: LoggedSet): Long

    @Update
    suspend fun update(set: LoggedSet)

    @Delete
    suspend fun delete(set: LoggedSet)

    @Query("SELECT * FROM logged_set WHERE logged_exercise_id = :loggedExerciseId ORDER BY set_index")
    fun observeForLoggedExercise(loggedExerciseId: Long): Flow<List<LoggedSet>>

    @Query("SELECT * FROM logged_set WHERE logged_exercise_id = :loggedExerciseId ORDER BY set_index")
    suspend fun forLoggedExercise(loggedExerciseId: Long): List<LoggedSet>

    /**
     * Every set ever performed for the given static exercise id (across all sessions).
     * Used for PR detection — feed this into [com.forge.app.domain.pr.PrDetector] to check
     * whether a proposed (weight, reps) tuple has been beaten before.
     */
    @Query("""
        SELECT s.* FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id = :exerciseId
        ORDER BY s.completed_at DESC
    """)
    suspend fun historyForExercise(exerciseId: String): List<LoggedSet>

    /** Max numeric weight ever lifted for the given exercise. Null if all logs are non-numeric (e.g. BW). */
    @Query("""
        SELECT MAX(s.weight_lb) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id = :exerciseId
    """)
    suspend fun maxWeightForExercise(exerciseId: String): Double?

    /** Max weight across any of the given exercise ids — for trophies like "Bench Club" that span bench variants. */
    @Query("""
        SELECT MAX(s.weight_lb) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id IN (:exerciseIds)
    """)
    suspend fun maxWeightAcrossExercises(exerciseIds: List<String>): Double?

    /** All sets in a window, joined to the exercise id — feeds weekly volume by muscle. */
    @Query("""
        SELECT ls.weight_lb, ls.reps, le.exercise_id
        FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE ls.completed_at >= :sinceEpochMs AND s.finished_at IS NOT NULL
    """)
    fun observeSetsSinceWithExerciseId(sinceEpochMs: Long): Flow<List<SetWithExerciseId>>

    /** Every set across every finished session, joined to exercise + session date. Used by strength curves. */
    @Query("""
        SELECT ls.weight_lb, ls.reps, le.exercise_id, s.started_at
        FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL
        ORDER BY s.started_at ASC
    """)
    fun observeAllFinishedSetsWithSession(): Flow<List<SetWithExerciseAndSession>>

    /** Max reps in any single logged set — feeds the "Rep Machine" trophy (#105). */
    @Query("SELECT MAX(reps) FROM logged_set")
    suspend fun maxRepsAnySet(): Int?

    /** Toggle per-set difficulty tag (#68). */
    @Query("UPDATE logged_set SET difficulty_tag = :tag WHERE id = :id")
    suspend fun setDifficultyTag(id: Long, tag: String?)

    /** Toggle AMRAP marker (#140). */
    @Query("UPDATE logged_set SET is_amrap = :v WHERE id = :id")
    suspend fun setAmrap(id: Long, v: Boolean)

    /** Toggle assisted marker (#141). */
    @Query("UPDATE logged_set SET is_assisted = :v WHERE id = :id")
    suspend fun setAssisted(id: Long, v: Boolean)

    /** Toggle failure marker (#18). */
    @Query("UPDATE logged_set SET to_failure = :v WHERE id = :id")
    suspend fun setToFailure(id: Long, v: Boolean)

    /** Set advanced set type (#142). */
    @Query("UPDATE logged_set SET set_type = :type WHERE id = :id")
    suspend fun setSetType(id: Long, type: String?)

    /** Set drop annotation (#143): "weightLb2/reps2" format. */
    @Query("UPDATE logged_set SET drop_annotation = :annotation WHERE id = :id")
    suspend fun setDropAnnotation(id: Long, annotation: String?)

    /** All sets in a session ordered by completedAt — used for actual rest-time computation (#82). */
    @Query("""
        SELECT ls.* FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        WHERE le.session_id = :sessionId
        ORDER BY ls.completed_at ASC
    """)
    suspend fun allForSession(sessionId: Long): List<LoggedSet>

    /** Peak single-session total volume — feeds the "Volume King" / "Volume Beast" trophies. */
    @Query("""
        SELECT MAX(session_total) FROM (
            SELECT le.session_id, SUM(IFNULL(s.weight_lb, 0) * s.reps) AS session_total
            FROM logged_set s
            INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
            INNER JOIN session ss ON le.session_id = ss.id
            WHERE ss.finished_at IS NOT NULL
            GROUP BY le.session_id
        )
    """)
    suspend fun maxSessionVolume(): Double?
}
