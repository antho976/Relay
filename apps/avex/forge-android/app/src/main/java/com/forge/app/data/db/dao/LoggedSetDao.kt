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

    /** Bulk insert — used by the workout importer, which writes a whole exercise's sets at once. */
    @Insert
    suspend fun insertAll(sets: List<LoggedSet>)

    @Update
    suspend fun update(set: LoggedSet)

    @Delete
    suspend fun delete(set: LoggedSet)

    @Query("SELECT * FROM logged_set WHERE id = :id")
    suspend fun get(id: Long): LoggedSet?

    @Query("SELECT * FROM logged_set WHERE logged_exercise_id = :loggedExerciseId ORDER BY set_index")
    suspend fun forLoggedExercise(loggedExerciseId: Long): List<LoggedSet>

    /** Number of sets logged under one exercise entry — guards swap re-attribution (#11). */
    @Query("SELECT COUNT(*) FROM logged_set WHERE logged_exercise_id = :loggedExerciseId")
    suspend fun countForLoggedExercise(loggedExerciseId: Long): Int

    /**
     * The highest set_index under one exercise entry, or null when it has no sets yet.
     *
     * The next index has to come from this rather than from the row COUNT. Deleting a set from the
     * middle leaves indices 0 and 2 with a count of 2, so the next set was written as a second
     * index 2 — and the wrist's prefill resolves `maxByOrNull { setIndex }` against that tie, so
     * the target weight it showed could flip between two different sets.
     */
    @Query("SELECT MAX(set_index) FROM logged_set WHERE logged_exercise_id = :loggedExerciseId")
    suspend fun maxSetIndex(loggedExerciseId: Long): Int?

    /**
     * Per-rep-count max weight across every prior non-assisted weighted set of an exercise —
     * the Pareto frontier PR detection compares against. [com.forge.app.domain.pr.PrDetector.isPr]
     * only ever asks "max prior weight at >= N reps", so this aggregate gives identical answers
     * to scanning the full set history while staying bounded by distinct rep counts (a few dozen
     * rows forever, vs every set ever logged). Assisted sets are excluded, matching isPr's filter.
     *
     * Untracked sessions are excluded too: `Session.isUntracked` promises a session is kept out of
     * stats and trophies, so a one-off max-out at a friend's gym must not raise the bar a later
     * tracked lift has to clear — that silently suppressed real PRs with nothing on screen to
     * explain why (every PR surface hides untracked rows).
     */
    @Query("""
        SELECT s.reps AS reps, MAX(s.weight_lb) AS weight_lb
        FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id = :exerciseId
          AND s.logged_exercise_id != :excludeLoggedExerciseId
          AND sess.is_untracked = 0
          AND s.weight_lb IS NOT NULL
          AND s.is_assisted = 0
          AND s.duration_seconds IS NULL
        GROUP BY s.reps
    """)
    suspend fun repMaxFrontierForExercise(exerciseId: String, excludeLoggedExerciseId: Long): List<RepMaxRow>

    data class RepMaxRow(
        @androidx.room.ColumnInfo(name = "reps") val reps: Int,
        @androidx.room.ColumnInfo(name = "weight_lb") val weightLb: Double
    )

    /**
     * Per-rep-count max weight for a SET of exercises across every PRIOR session's non-assisted
     * weighted sets — "prior" = a session that STARTED before [beforeStartedAt]. The session-detail
     * page flags an e1RM as a new best by comparing against this, so an old session reflects what was
     * a best AT THE TIME (the started_at filter), not all-time. One query covers the whole session's
     * exercises (vs one query per exercise). Returns nothing for an empty [exerciseIds].
     */
    @Query("""
        SELECT le.exercise_id AS exercise_id, s.reps AS reps, MAX(s.weight_lb) AS weight_lb
        FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id IN (:exerciseIds)
          AND sess.started_at < :beforeStartedAt
          AND sess.is_untracked = 0
          AND s.weight_lb IS NOT NULL
          AND s.is_assisted = 0
          AND s.duration_seconds IS NULL
        GROUP BY le.exercise_id, s.reps
    """)
    suspend fun repMaxFrontierBeforeSession(
        exerciseIds: List<String>,
        beforeStartedAt: Long
    ): List<ExerciseRepMaxRow>

    data class ExerciseRepMaxRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
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

    /**
     * The single best set ever for an exercise: heaviest weight, most reps at that weight.
     * Timed-hold sets (`duration_seconds` set, reps not meaningful) are excluded here and in every
     * weight×reps / e1RM / volume aggregate below — a weighted plank must not read as a strength best
     * (GYMAP-51). Read/display queries (forLoggedExercise, allForSession) keep timed sets.
     *
     * Assisted and untracked sets are excluded here and in every max-weight query below, matching
     * [repMaxFrontierForExercise] and isPr: a set the PR engine refuses to recognise must not become
     * the personal best, fill a goal bar or unlock a trophy. The app used to contradict itself on
     * two adjacent screens — "PB 40 lb" on a lift with no PR ever recorded.
     *
     * The `finished_at IS NOT NULL` join predicate keeps the LIVE session out of every maximum here
     * and below, matching [maxSessionVolume] and [topLift]. A mid-workout typo (2255 for 225) used
     * to be visible to the trophy pass the moment the exercise was completed, and
     * `UnlockedTrophyDao.unlock` is IGNORE-on-conflict — so deleting the bad set seconds later left
     * the trophies unlocked forever. Both trophy passes run AFTER `finishSession` stamps
     * `finished_at`, so the session that just ended still counts.
     */
    @Query("""
        SELECT s.* FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id = :exerciseId AND s.weight_lb IS NOT NULL AND s.duration_seconds IS NULL
          AND s.is_assisted = 0 AND sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
        ORDER BY s.weight_lb DESC, s.reps DESC
        LIMIT 1
    """)
    suspend fun personalBestSet(exerciseId: String): LoggedSet?

    /**
     * Longest hold ever for a timed-hold exercise (GYMAP-51) — the counterpart to [maxWeightForExercise]
     * for holds, driving the "best 0:45" prefill/ghost hint. Null if the exercise has no timed set.
     */
    @Query("""
        SELECT MAX(s.duration_seconds) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        WHERE le.exercise_id = :exerciseId AND s.duration_seconds IS NOT NULL
    """)
    suspend fun bestHoldSecondsForExercise(exerciseId: String): Int?

    /** Max numeric weight ever lifted for the given exercise, unassisted and tracked. Null if all
     *  logs are non-numeric (e.g. BW). */
    @Query("""
        SELECT MAX(s.weight_lb) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id = :exerciseId AND s.duration_seconds IS NULL
          AND s.is_assisted = 0 AND sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
    """)
    suspend fun maxWeightForExercise(exerciseId: String): Double?

    /** Max weight across any of the given exercise ids — for trophies like "Bench Club" that span bench variants. */
    @Query("""
        SELECT MAX(s.weight_lb) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id IN (:exerciseIds) AND s.duration_seconds IS NULL
          AND s.is_assisted = 0 AND sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
    """)
    suspend fun maxWeightAcrossExercises(exerciseIds: List<String>): Double?

    /**
     * Heaviest set PER exercise for the given ids in ONE query — the Goals screen's progress join,
     * instead of N separate [maxWeightAcrossExercises] calls. Exercises with no weighted set are
     * simply absent from the result (the caller defaults them to 0).
     */
    @Query("""
        SELECT le.exercise_id AS exercise_id, MAX(s.weight_lb) AS weight_lb
        FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE le.exercise_id IN (:exerciseIds) AND s.weight_lb IS NOT NULL AND s.duration_seconds IS NULL
          AND s.is_assisted = 0 AND sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
        GROUP BY le.exercise_id
    """)
    suspend fun maxWeightPerExercise(exerciseIds: List<String>): List<ExerciseMaxRow>

    data class ExerciseMaxRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "weight_lb") val weightLb: Double
    )

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
          AND ls.duration_seconds IS NULL
        ORDER BY s.started_at ASC
    """)
    fun observeAllFinishedSetsWithSession(): Flow<List<SetWithExerciseAndSession>>

    /**
     * Best estimated 1-rep-max (lb, Epley) across working sets (non-assisted, non-skipped, tracked)
     * since [sinceMs] — computed in SQL so the profile standing engine doesn't load every set into
     * memory just to take a max. Mirrors [com.forge.app.domain.adapt.E1rm.epley]: reps ≤ 1 → the raw
     * weight, else weight·(1 + reps/30). Null when no qualifying weighted set exists (pre-baseline).
     */
    @Query("""
        SELECT MAX(CASE WHEN ls.reps <= 1 THEN ls.weight_lb ELSE ls.weight_lb * (1 + ls.reps / 30.0) END)
        FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.started_at >= :sinceMs
          AND s.is_untracked = 0 AND le.skipped = 0 AND ls.is_assisted = 0
          AND ls.weight_lb IS NOT NULL AND ls.weight_lb > 0
          AND ls.duration_seconds IS NULL
    """)
    suspend fun bestE1rmLbSince(sinceMs: Long): Double?

    /** Max reps in any single logged set of a tracked session. */
    @Query("""
        SELECT MAX(s.reps) FROM logged_set s
        INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
        INNER JOIN session sess ON le.session_id = sess.id
        WHERE sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
    """)
    suspend fun maxRepsAnySet(): Int?

    /** Max reps summed across one exercise's sets (per logged exercise) — the "Rep Machine" trophy (#105). */
    @Query("""
        SELECT MAX(total_reps) FROM (
            SELECT SUM(s.reps) AS total_reps
            FROM logged_set s
            INNER JOIN logged_exercise le ON s.logged_exercise_id = le.id
            INNER JOIN session sess ON le.session_id = sess.id
            WHERE sess.is_untracked = 0 AND sess.finished_at IS NOT NULL
            GROUP BY s.logged_exercise_id
        )
    """)
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
          AND ls.duration_seconds IS NULL
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

    /** Reactive per-session sets — drives the watch's /session/live mirror (W1): every set log or
     *  undo re-emits, so the wrist's ticks track Room, not any ViewModel. */
    @Query("""
        SELECT ls.* FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        WHERE le.session_id = :sessionId
        ORDER BY ls.completed_at
    """)
    fun observeAllForSession(sessionId: Long): Flow<List<LoggedSet>>

    /**
     * Every set across finished, tracked sessions — pairs with
     * [LoggedExerciseDao.allForFinishedSessions] for the adaptation-engine snapshot.
     */
    @Query("""
        SELECT ls.* FROM logged_set ls
        INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.is_untracked = 0
          AND ls.duration_seconds IS NULL
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
            WHERE ss.finished_at IS NOT NULL AND ss.is_untracked = 0 AND s.duration_seconds IS NULL
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
        WHERE ss.finished_at IS NOT NULL AND s.weight_lb IS NOT NULL AND s.duration_seconds IS NULL
          AND s.is_assisted = 0 AND ss.is_untracked = 0
        ORDER BY s.weight_lb DESC
        LIMIT 1
    """)
    suspend fun topLift(): TopLiftRow?

    data class TopLiftRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "weight_lb") val weightLb: Double
    )
}
