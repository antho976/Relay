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
    fun observeForSession(sessionId: Long): Flow<List<LoggedExercise>>

    @Query("SELECT * FROM logged_exercise WHERE session_id = :sessionId ORDER BY order_index")
    suspend fun forSession(sessionId: Long): List<LoggedExercise>

    /**
     * The most recently logged instance of this exercise in any *other* session. Used
     * to pre-fill the weight input on the day screen from the user's last performance.
     */
    @Query("""
        SELECT * FROM logged_exercise
        WHERE exercise_id = :exerciseId AND session_id != :excludeSessionId
        ORDER BY id DESC LIMIT 1
    """)
    suspend fun lastLoggedBefore(exerciseId: String, excludeSessionId: Long): LoggedExercise?

    /** Trophy counts. */
    @Query("SELECT COUNT(*) FROM logged_exercise WHERE was_pr = 1")
    fun observePrCount(): Flow<Int>

    @Query("SELECT COUNT(*) FROM logged_exercise WHERE was_pr = 1")
    suspend fun prCount(): Int

    @Query("SELECT COUNT(*) FROM logged_exercise WHERE difficulty = :rating")
    fun observeCountWithRating(rating: EffortRating): Flow<Int>

    @Query("SELECT COUNT(*) FROM logged_exercise WHERE difficulty = :rating")
    suspend fun countWithRating(rating: EffortRating): Int

    @Query("SELECT COUNT(*) FROM logged_exercise WHERE hit_full_target = 1")
    suspend fun fullTargetCount(): Int

    @Query("SELECT COUNT(*) FROM logged_exercise WHERE swapped_name IS NOT NULL")
    suspend fun swapCount(): Int

    /** Total logged exercises ever — the "totalSessions" stat in the prototype counts these, not Sessions. */
    @Query("SELECT COUNT(*) FROM logged_exercise")
    suspend fun totalLogged(): Int

    @Query("SELECT COUNT(*) FROM logged_exercise")
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
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL
        ORDER BY s.started_at DESC
        LIMIT 30
    """)
    fun observeRecentPrs(): Flow<List<RecentPrRow>>

    /** Counts per exercise_id across all finished sessions — used to pick top N for strength curves. */
    @Query("""
        SELECT le.exercise_id, COUNT(*) AS cnt
        FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE s.finished_at IS NOT NULL
        GROUP BY le.exercise_id
        ORDER BY cnt DESC
    """)
    suspend fun exerciseFrequencyOrdered(): List<ExerciseFrequencyRow>

    data class ExerciseFrequencyRow(
        @androidx.room.ColumnInfo(name = "exercise_id") val exerciseId: String,
        @androidx.room.ColumnInfo(name = "cnt") val count: Int
    )
}
