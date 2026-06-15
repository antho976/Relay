package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.projections.ExerciseSessionAggregate
import com.forge.app.data.db.projections.SetWithExerciseAndSession
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
    suspend fun forLoggedExercise(loggedExerciseId: Long): List<LoggedSet>

    /** Number of sets logged under one exercise entry — guards swap re-attribution (#11). */
    @Query("SELECT COUNT(*) FROM logged_set WHERE logged_exercise_id = :loggedExerciseId")
    suspend fun countForLoggedExercise(loggedExerciseId: Long): Int

    /**
     * Per-rep-count max weight across every prior non-assisted weighted set of an exercise —
     * the Pareto frontier PR detection compares against. [com.forge.app.domain.pr.PrDetector.isPr]
     * only ever asks "max prior weight at >= N reps", so this aggregate gives identical answers
     * to scanning the full set history while staying bounded by distinct rep counts (a few dozen
     * rows forever, vs every set ever logged). Assisted sets are excluded, matching isPr's filter.
     */
    @Query("""
        SELECT s.reps AS reps, MAX(s.weight_lb) AS weight_lb
        FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id = :exerciseId
          AND s.logged_exercise_id != :excludeLoggedExerciseId
          AND s.weight_lb IS NOT NULL
          AND s.is_assisted = 0
        GROUP BY s.reps
    """)
    suspend fun repMaxFrontierForExercise(exerciseId: String, excludeLoggedExerciseId: Long): List<RepMaxRow>

    data class RepMaxRow(
        @androidx.room.ColumnInfo(name = "reps") val reps: Int,
        @androidx.room.ColumnInfo(name = "weight_lb") val weightLb: Double
    )

    /**
     * True when the exercise has ANY prior set (incl. bodyweight/assisted) outside
     * [excludeLoggedExerciseId] — the "first-ever time" gate for PR flagging, which the
     * weighted-only frontier can't answer on its own.
     */
    @Query("""
        SELECT EXISTS(
            SELECT 1 FROM logged_set s
            INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
            WHERE le.exercise_id = :exerciseId AND s.logged_exercise_id != :excludeLoggedExerciseId
        )
    """)
    suspend fun hasHistoryForExercise(exerciseId: String, excludeLoggedExerciseId: Long): Boolean

    /** The single best set ever for an exercise: heaviest weight, most reps at that weight. */
    @Query("""
        SELECT s.* FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id = :exerciseId AND s.weight_lb IS NOT NULL
        ORDER BY s.weight_lb DESC, s.reps DESC
        LIMIT 1
    """)
    suspend fun personalBestSet(exerciseId: String): LoggedSet?

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

    /**
     * Every set across every finished session, joined to exercise + session date. Used by strength
     * curves, PRs, hall-of-fame, e1RM lifts, pattern radar, rep-range/RPE, and the consistency/
     * session counts. Filters to TRACKED, non-skipped, unassisted working sets so these surfaces
     * match the adaptation engine's population (which excludes untracked sessions, skipped
     * exercises, and assisted reps) — they previously diverged on the same screen.
     */
    @Query("""
        SELECT ls.weight_lb, ls.reps, ls.rpe, le.exercise_id, s.started_at, le.id AS logged_exercise_id
        FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL
          AND s.is_untracked = 0 AND le.skipped = 0 AND ls.is_assisted = 0
        ORDER BY s.started_at ASC
    """)
    fun observeAllFinishedSetsWithSession(): Flow<List<SetWithExerciseAndSession>>

    /** Max reps in any single logged set. */
    @Query("SELECT MAX(reps) FROM logged_set")
    suspend fun maxRepsAnySet(): Int?

    /** Max reps summed across one exercise's sets (per logged exercise) — the "Rep Machine" trophy (#105). */
    @Query("SELECT MAX(total_reps) FROM (SELECT SUM(reps) AS total_reps FROM logged_set GROUP BY logged_exercise_id)")
    suspend fun maxRepsSummedPerExercise(): Int?

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

    /** Set per-set RPE (1.0–10.0 in 0.5 steps, or null to clear). */
    @Query("UPDATE logged_set SET rpe = :rpe WHERE id = :id")
    suspend fun setRpe(id: Long, rpe: Double?)

    /**
     * Per-session aggregates for one exercise across finished sessions, newest first.
     * Used by the day-screen last-session strip + sparkline. Excludes the current
     * in-progress session by relying on session.finished_at IS NOT NULL.
     */
    @Query("""
        SELECT s.started_at AS started_at,
               s.finished_at AS finished_at,
               COALESCE(SUM(IFNULL(ls.weight_lb, 0) * ls.reps), 0) AS volume_lb,
               MAX(ls.weight_lb) AS top_weight_lb
        FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.exercise_id = :exerciseId AND s.finished_at IS NOT NULL
        GROUP BY s.id
        ORDER BY s.started_at DESC
        LIMIT :limit
    """)
    suspend fun sessionAggregatesForExercise(exerciseId: String, limit: Int = 8): List<ExerciseSessionAggregate>

    /** All sets in a session ordered by completedAt — used for actual rest-time computation (#82). */
    @Query("""
        SELECT ls.* FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        WHERE le.session_id = :sessionId
        ORDER BY ls.completed_at ASC
    """)
    suspend fun allForSession(sessionId: Long): List<LoggedSet>

    /**
     * Every set across finished, tracked sessions — pairs with
     * [LoggedExerciseDao.allForFinishedSessions] for the adaptation-engine snapshot.
     */
    @Query("""
        SELECT ls.* FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.is_untracked = 0
        ORDER BY ls.completed_at ASC
    """)
    suspend fun allForFinishedSessions(): List<LoggedSet>

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

    /** The single heaviest set ever logged, with its exercise id — the profile "top lift" signature. */
    @Query("""
        SELECT le.exercise_id AS exercise_id, s.weight_lb AS weight_lb
        FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session ss ON le.session_id = ss.id
        WHERE ss.finished_at IS NOT NULL AND s.weight_lb IS NOT NULL
        ORDER BY s.weight_lb DESC
        LIMIT 1
    """)
    suspend fun topLift(): TopLiftRow?

    data class TopLiftRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "weight_lb") val weightLb: Double
    )
}
