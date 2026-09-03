package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.projections.HeatmapTimestamp
import com.forge.app.data.db.projections.RecentPrRow
import com.forge.app.data.db.types.EffortRating
import kotlinx.coroutines.flow.Flow

@Dao
interface LoggedExerciseDao {

    @Insert
    suspend fun insert(loggedExercise: LoggedExercise): Long

    @Update
    suspend fun update(loggedExercise: LoggedExercise)

    @Delete
    suspend fun delete(loggedExercise: LoggedExercise)

    @Query("SELECT * FROM logged_exercise WHERE id = :id")
    suspend fun get(id: Long): LoggedExercise?

    @Query("SELECT * FROM logged_exercise WHERE session_id = :sessionId ORDER BY order_index")
    suspend fun forSession(sessionId: Long): List<LoggedExercise>

    /**
     * Every row for a BATCH of sessions, in one query (P-01).
     *
     * The JSON export asked per session, so a year of training was a thousand round trips through
     * Room for a file the user is waiting on. Chunk the ids to stay under SQLite's variable limit
     * and group the result once — see `BackupRepository`.
     */
    @Query("SELECT * FROM logged_exercise WHERE session_id IN (:sessionIds) ORDER BY session_id, order_index")
    suspend fun forSessions(sessionIds: List<Long>): List<LoggedExercise>

    /**
     * This session's row for one program SLOT, matching [LoggedExercise.effectiveSlotId]
     * (`slot_id` when swapped, else `exercise_id`).
     *
     * The day screen needs a source of truth for "does this slot already have a row" that is not
     * the UI state, which only refreshes after a write completes. There is no unique index on
     * (session_id, slot) to lean on, so this read is what makes creation idempotent.
     */
    @Query("""
        SELECT * FROM logged_exercise
        WHERE session_id = :sessionId AND COALESCE(slot_id, exercise_id) = :slotId
        ORDER BY id LIMIT 1
    """)
    suspend fun forSessionSlot(sessionId: Long, slotId: String): LoggedExercise?

    /** Reactive [forSession] — drives the watch's /session/live mirror (W1) off Room invalidation. */
    @Query("SELECT * FROM logged_exercise WHERE session_id = :sessionId ORDER BY order_index")
    fun observeForSession(sessionId: Long): Flow<List<LoggedExercise>>

    /**
     * The most recently logged instance of this exercise in any *other* session. Used
     * to pre-fill the weight input on the day screen from the user's last performance.
     *
     * Ordered by the session's `started_at`, NOT by `logged_exercise.id`. The row id is an
     * insertion counter that only tracks chronology while every session is created live: the
     * importer writes BACKDATED sessions with fresh, highest-yet ids, so after backfilling years of
     * history "last time" resolved to the newest row the importer happened to write — and that
     * stale performance fed straight into the suggested working weight.
     *
     * Three predicates decide what qualifies as "last time", and all three are about the same
     * thing: the prescribed load for the next session is a SUGGESTION, and untracked sessions are
     * excluded from suggestions by contract ([com.forge.app.data.db.entities.Session.isUntracked]).
     *
     *  - `is_untracked = 0` — a session at a friend's gym on unfamiliar equipment must not become
     *    the anchor for the next prescribed weight.
     *  - `finished_at IS NOT NULL` — a session still in progress is not a past performance.
     *  - `EXISTS (a set)` — a swap or a skip creates a logged_exercise row eagerly, with no sets
     *    under it. A newer EMPTY row outranked the real work below it, so the card read "First
     *    time" for a lift with years of history and the wrist prefilled from nothing.
     */
    @Query("""
        SELECT le.* FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.exercise_id = :exerciseId AND le.session_id != :excludeSessionId
          AND s.finished_at IS NOT NULL
          AND s.is_untracked = 0
          AND EXISTS (SELECT 1 FROM logged_set ls WHERE ls.logged_exercise_id = le.id)
        ORDER BY s.started_at DESC, le.id DESC LIMIT 1
    """)
    suspend fun lastLoggedBefore(exerciseId: String, excludeSessionId: Long): LoggedExercise?

    /**
     * Trophy and lifetime counts.
     *
     * All of these join `session` for the same reason: they are permanent progression counters, and
     * `Session.isUntracked` promises that an untracked session is "excluded from streak, trophies,
     * suggestions". Counting the bare `logged_exercise` table meant a workout the user explicitly
     * marked as not counting still unlocked trophies, raised the lifetime PR total and inflated the
     * ratings histogram — on the very screens that hide untracked rows, so the number and the list
     * it was supposedly counting disagreed with each other. The `finished_at` half keeps the LIVE
     * session out for the same reason every maximum in LoggedSetDao does.
     */
    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    fun observePrCount(): Flow<Int>

    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun prCount(): Int

    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.difficulty = :rating AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun countWithRating(rating: EffortRating): Int

    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.hit_full_target = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun fullTargetCount(): Int

    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.swapped_name IS NOT NULL AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun swapCount(): Int

    /** Total logged exercises ever — the "totalSessions" stat in the prototype counts these, not Sessions. */
    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun totalLogged(): Int

    @Query("""
        SELECT COUNT(*) FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    fun observeTotalLogged(): Flow<Int>

    /** For the frequency heatmap. One row per LoggedExercise; aggregated to per-day counts in Kotlin. */
    @Query("""
        SELECT s.started_at FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.started_at >= :sinceEpochMs
    """)
    fun observeHeatmapTimestamps(sinceEpochMs: Long): Flow<List<HeatmapTimestamp>>

    /** PR timeline — 30 most recent PR-marked exercises, joined to session date. */
    @Query("""
        SELECT le.exercise_id, le.swapped_name, s.started_at, le.id AS logged_exercise_id
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
        ORDER BY s.started_at DESC
        LIMIT 30
    """)
    fun observeRecentPrs(): Flow<List<RecentPrRow>>

    /** All PRs with session date — for the PRs subtab (#39). */
    @Query("""
        SELECT le.exercise_id, le.swapped_name, s.started_at, le.id AS logged_exercise_id
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
        ORDER BY s.started_at DESC
    """)
    fun observeAllPrs(): Flow<List<RecentPrRow>>

    /**
     * Exercise frequency in past N weeks — distinct sessions containing each exercise (#73).
     * Excludes skipped exercises (you didn't actually do them) and untracked sessions, matching
     * the Stats history policy (also consumed by the monthly/yearly Recap).
     */
    @Query("""
        SELECT le.exercise_id, COUNT(DISTINCT le.session_id) AS session_count
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.started_at >= :sinceMs
          AND s.is_untracked = 0 AND le.skipped = 0
        GROUP BY le.exercise_id
        ORDER BY session_count DESC
    """)
    suspend fun frequencySince(sinceMs: Long): List<ExerciseFreqRow>

    data class ExerciseFreqRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "session_count") val sessionCount: Int
    )

    /**
     * The same population as [frequencySince] (finished, tracked, non-skipped, started on or after
     * [sinceMs]) but as raw (session, exercise id, swapped name) rows, for a caller that has to
     * resolve each row's DISPLAY identity before counting. The Recap's "most trained" needs that:
     * a custom move keeps its name only in `swapped_name`, and a slot that was swapped late keeps
     * its original id under a different name, so grouping on `exercise_id` alone named the wrong
     * movement (a humanized slug) or merged two into one bucket.
     */
    @Query("""
        SELECT le.session_id AS session_id, le.exercise_id AS exercise_id, le.swapped_name AS swapped_name
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.started_at >= :sinceMs
          AND s.is_untracked = 0 AND le.skipped = 0
        ORDER BY s.started_at ASC, le.id ASC
    """)
    suspend fun sessionExerciseRowsSince(sinceMs: Long): List<SessionExerciseRow>

    /** All PR dates per exercise ordered chronologically — used to compute time-to-next-PR (#74). */
    @Query("""
        SELECT le.exercise_id, s.started_at AS session_date
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
        ORDER BY le.exercise_id, s.started_at ASC
    """)
    suspend fun prDatesPerExercise(): List<ExercisePrDate>

    data class ExercisePrDate(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "session_date") val sessionDate: Long
    )

    /** Effort ratings with session dates — used for the weekly effort distribution chart (#75). */
    @Query("""
        SELECT le.difficulty, s.started_at AS session_date
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.started_at >= :sinceMs AND le.difficulty IS NOT NULL
          AND s.is_untracked = 0 AND le.skipped = 0
    """)
    suspend fun effortRatingsSince(sinceMs: Long): List<EffortWithDate>

    data class EffortWithDate(
        @androidx.room.ColumnInfo(name = "difficulty") val difficulty: String?,
        @androidx.room.ColumnInfo(name = "session_date") val sessionDate: Long
    )

    /**
     * Every logged exercise across finished, tracked sessions — the adaptation engine's
     * snapshot fan-out (one query instead of per-exercise loops). Untracked sessions are
     * excluded here so they never feed a suggestion (#110).
     */
    @Query("""
        SELECT le.* FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND s.is_untracked = 0
        ORDER BY s.started_at ASC, le.order_index ASC
    """)
    suspend fun allForFinishedSessions(): List<LoggedExercise>

    /** Set superset group for an exercise (#38). */
    /**
     * Single-column writes, so two of these racing can't clobber each other.
     *
     * They used to be SELECT-then-`update(row.copy(...))` in the repository: each built a whole row
     * from its own pre-read snapshot, so a note debounce firing while the user tapped SKIP wrote
     * `skipped = false` straight back over the skip. The exercise silently un-skipped itself and
     * counted against the session's honesty percentage. [setSupersetGroup] below already had the
     * right shape.
     */
    @Query("UPDATE logged_exercise SET difficulty = :rating WHERE id = :id")
    suspend fun setDifficulty(id: Long, rating: EffortRating?)

    @Query("UPDATE logged_exercise SET skipped = :skipped WHERE id = :id")
    suspend fun setSkipped(id: Long, skipped: Boolean)

    @Query("UPDATE logged_exercise SET note = :note WHERE id = :id")
    suspend fun setNote(id: Long, note: String?)

    @Query("UPDATE logged_exercise SET superset_group = :group WHERE id = :id")
    suspend fun setSupersetGroup(id: Long, group: String?)

    /**
     * The PR flag, on its own.
     *
     * The asynchronous PR recalculation used to write a whole LoggedExercise built from its own
     * earlier read, purely to set this one boolean — the exact shape the note-vs-skip race above
     * was fixed for. A rating, a note or a skip landing while the recalculation was in flight was
     * overwritten by the stale snapshot it started from.
     */
    @Query("UPDATE logged_exercise SET was_pr = :wasPr WHERE id = :id")
    suspend fun setWasPr(id: Long, wasPr: Boolean)

    /** Full-text search across all exercise notes (#60). */
    @Query("""
        SELECT le.id, le.exercise_id, le.swapped_name, le.note, s.started_at AS session_started_at
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.note IS NOT NULL AND le.note != '' AND le.note LIKE '%' || :query || '%' ESCAPE '\'
        ORDER BY s.started_at DESC
        LIMIT 100
    """)
    suspend fun searchNotes(query: String): List<NoteSearchResult>

    data class NoteSearchResult(
        @androidx.room.ColumnInfo(name = "id") val id: Long,
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "swapped_name") val swappedName: String?,
        @androidx.room.ColumnInfo(name = "note") val note: String?,
        @androidx.room.ColumnInfo(name = "session_started_at") val sessionStartedAt: Long
    )

    /**
     * (session, exercise) pairs across every finished session — lets the History screen search by
     * exercise name. Skipped entries are excluded (you didn't do them); the caller resolves each id
     * to a display name via [com.forge.app.program.Program.exerciseDisplayName].
     */
    @Query("""
        SELECT le.session_id AS session_id, le.exercise_id AS exercise_id, le.swapped_name AS swapped_name
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL AND le.skipped = 0
    """)
    fun observeSessionExerciseIds(): Flow<List<SessionExerciseRow>>

    data class SessionExerciseRow(
        @androidx.room.ColumnInfo(name = "session_id") val sessionId: Long,
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "swapped_name") val swappedName: String?
    )
}
