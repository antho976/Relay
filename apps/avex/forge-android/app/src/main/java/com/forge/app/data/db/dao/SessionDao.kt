package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.Session
import kotlinx.coroutines.flow.Flow

@Dao
interface SessionDao {

    @Insert
    suspend fun insert(session: Session): Long

    @Update
    suspend fun update(session: Session)

    @Delete
    suspend fun delete(session: Session)

    @Query("SELECT * FROM session WHERE id = :id")
    suspend fun get(id: Long): Session?

    /** Returns the in-progress session if one exists. App logic limits this to at most one. */
    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    suspend fun getActiveSession(): Session?

    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    fun observeActiveSession(): Flow<Session?>

    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY finished_at DESC LIMIT :limit")
    fun observeRecent(limit: Int = 10): Flow<List<Session>>

    @Query("""
        SELECT * FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL
        ORDER BY finished_at DESC LIMIT 1
    """)
    suspend fun lastFinishedForDay(dayKey: String): Session?

    /** Used by the "Showing Up" / "Through the Door" trophy rules. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL")
    fun observeFinishedCount(): Flow<Int>

    /** Used by the "Full Week" trophy (all 4 days trained). */
    @Query("SELECT DISTINCT day_key FROM session WHERE finished_at IS NOT NULL")
    suspend fun distinctDayKeysTrained(): List<String>

    /** For the deload banner — sessions completed since the user last marked deload. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL")
    suspend fun finishedCount(): Int

    /** Previous finished session for the same day (excludes current — used for session comparison #52). */
    @Query("""
        SELECT * FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL AND id != :excludeSessionId
        ORDER BY finished_at DESC LIMIT 1
    """)
    suspend fun previousFinishedForDay(dayKey: String, excludeSessionId: Long): Session?

    /** Best (highest) volume ever recorded for a given day, excluding the current session (#53). */
    @Query("""
        SELECT MAX(total_volume_lb) FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL AND id != :excludeSessionId
    """)
    suspend fun maxVolumeForDay(dayKey: String, excludeSessionId: Long): Double?

    /** Rolling-window queries for the Overview weekly stats strip. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL AND finished_at >= :sinceEpochMs")
    fun observeFinishedCountSince(sinceEpochMs: Long): Flow<Int>

    @Query("SELECT SUM(total_volume_lb) FROM session WHERE finished_at IS NOT NULL AND finished_at >= :sinceEpochMs")
    fun observeVolumeSince(sinceEpochMs: Long): Flow<Double?>

    /** Earliest finished session timestamp — used for the "first full month" milestone (#56). */
    @Query("SELECT MIN(started_at) FROM session WHERE finished_at IS NOT NULL")
    fun observeFirstFinishedSessionStartedAt(): Flow<Long?>

    /**
     * Nearest finished session whose start falls within [fromMs]–[toMs].
     * Orders by proximity to [targetMs] so the closest day to the anniversary is returned.
     * Used for the "On this day" memory card (#106).
     */
    @Query("""
        SELECT * FROM session
        WHERE finished_at IS NOT NULL AND started_at BETWEEN :fromMs AND :toMs
        ORDER BY ABS(started_at - :targetMs) ASC
        LIMIT 1
    """)
    suspend fun sessionNearDate(targetMs: Long, fromMs: Long, toMs: Long): Session?

    /** All finished sessions ordered oldest-first — used by trophy snapshot computations. */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY started_at ASC")
    suspend fun allFinished(): List<Session>

    /** Sessions finished within a calendar month [fromMs, toMs). Used by the monthly calendar (#54). */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL AND started_at >= :fromMs AND started_at < :toMs ORDER BY started_at ASC")
    suspend fun finishedInRange(fromMs: Long, toMs: Long): List<Session>

    /** Reactive version of [finishedInRange] — emits on any session change. */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL AND started_at >= :fromMs AND started_at < :toMs ORDER BY started_at ASC")
    fun observeFinishedInRange(fromMs: Long, toMs: Long): Flow<List<Session>>

    /** Deletes all sessions (CASCADE removes LoggedExercise, LoggedSet, MoodEntry). For reset (#119). */
    @Query("DELETE FROM session")
    suspend fun deleteAll()

    @Query("UPDATE session SET session_type = :type WHERE id = :id")
    suspend fun setSessionType(id: Long, type: String)

    @Query("UPDATE session SET is_untracked = :v WHERE id = :id")
    suspend fun setUntracked(id: Long, v: Boolean)

    @Query("UPDATE session SET journal = :text WHERE id = :id")
    suspend fun setJournal(id: Long, text: String)

    @Query("UPDATE session SET intensity = :intensity WHERE id = :id")
    suspend fun setIntensity(id: Long, intensity: String)

    /** All finished sessions ordered newest first — for session history screen (#62). */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY started_at DESC")
    fun observeAllFinishedSessions(): Flow<List<Session>>

    /** Sessions with mood for effort/difficulty trend (#95). */
    @Query("""
        SELECT s.started_at, m.mood FROM session s
        INNER JOIN mood_entry m ON m.session_id = s.id
        WHERE s.finished_at IS NOT NULL
        ORDER BY s.started_at ASC
    """)
    fun observeMoodOverTime(): kotlinx.coroutines.flow.Flow<List<MoodOverTime>>

    data class MoodOverTime(
        @androidx.room.ColumnInfo(name = "started_at") val startedAt: Long,
        @androidx.room.ColumnInfo(name = "mood") val mood: String
    )

    /** Aggregate stats for a window — for week/month comparisons (#34, #130). */
    @Query("""
        SELECT COUNT(*) AS session_count,
               SUM(total_volume_lb) AS total_volume,
               SUM(pr_count) AS total_prs,
               SUM(set_count) AS total_sets
        FROM session
        WHERE finished_at IS NOT NULL AND started_at >= :fromMs AND started_at < :toMs
    """)
    suspend fun aggregateInRange(fromMs: Long, toMs: Long): WindowAggregate

    data class WindowAggregate(
        @androidx.room.ColumnInfo(name = "session_count") val sessionCount: Int,
        @androidx.room.ColumnInfo(name = "total_volume") val totalVolume: Double?,
        @androidx.room.ColumnInfo(name = "total_prs") val totalPrs: Int,
        @androidx.room.ColumnInfo(name = "total_sets") val totalSets: Int
    )

    /** All finished sessions with volume + deload flag ordered oldest-first — for #126 volume/deload trend. */
    @Query("""
        SELECT id, day_key, started_at, total_volume_lb, deload_marked_here
        FROM session WHERE finished_at IS NOT NULL ORDER BY started_at ASC
    """)
    suspend fun allFinishedVolumeDeload(): List<SessionVolumeDeloadRow>

    data class SessionVolumeDeloadRow(
        @androidx.room.ColumnInfo(name = "id") val id: Long,
        @androidx.room.ColumnInfo(name = "day_key") val dayKey: String,
        @androidx.room.ColumnInfo(name = "started_at") val startedAt: Long,
        @androidx.room.ColumnInfo(name = "total_volume_lb") val totalVolumeLb: Double?,
        @androidx.room.ColumnInfo(name = "deload_marked_here") val deloadMarkedHere: Boolean
    )

    /** Avg and max volume per day_key across all finished sessions — for #132 best vs average. */
    @Query("""
        SELECT day_key, AVG(total_volume_lb) AS avg_vol, MAX(total_volume_lb) AS max_vol, COUNT(*) AS session_count
        FROM session WHERE finished_at IS NOT NULL AND total_volume_lb IS NOT NULL
        GROUP BY day_key
    """)
    suspend fun avgMaxVolumeByDayKey(): List<DayVolumeStats>

    /** Lifetime aggregate for #40 session metrics. */
    @Query("""
        SELECT SUM(total_volume_lb) AS total_volume, COUNT(*) AS session_count,
               AVG(set_count) AS avg_sets
        FROM session WHERE finished_at IS NOT NULL AND total_volume_lb IS NOT NULL
    """)
    suspend fun lifetimeAggregate(): LifetimeAggregate

    data class LifetimeAggregate(
        @androidx.room.ColumnInfo(name = "total_volume") val totalVolume: Double?,
        @androidx.room.ColumnInfo(name = "session_count") val sessionCount: Int,
        @androidx.room.ColumnInfo(name = "avg_sets") val avgSets: Double?
    )

    /** Per-day-type: avg duration, PR rate, set count — for #134. */
    @Query("""
        SELECT day_key, COUNT(*) AS session_count,
               AVG((finished_at - started_at) / 60000) AS avg_duration_min,
               AVG(CAST(pr_count AS FLOAT) / NULLIF(set_count, 0)) AS pr_rate,
               SUM(total_volume_lb) AS total_vol
        FROM session WHERE finished_at IS NOT NULL
        GROUP BY day_key
    """)
    suspend fun perDayTypeStats(): List<DayTypeStats>

    data class DayTypeStats(
        @androidx.room.ColumnInfo(name = "day_key") val dayKey: String,
        @androidx.room.ColumnInfo(name = "session_count") val sessionCount: Int,
        @androidx.room.ColumnInfo(name = "avg_duration_min") val avgDurationMin: Double?,
        @androidx.room.ColumnInfo(name = "pr_rate") val prRate: Double?,
        @androidx.room.ColumnInfo(name = "total_vol") val totalVol: Double?
    )

    data class DayVolumeStats(
        @androidx.room.ColumnInfo(name = "day_key") val dayKey: String,
        @androidx.room.ColumnInfo(name = "avg_vol") val avgVolume: Double,
        @androidx.room.ColumnInfo(name = "max_vol") val maxVolume: Double,
        @androidx.room.ColumnInfo(name = "session_count") val sessionCount: Int
    )

    /** All PR session start timestamps (for #85 day-of-week PR distribution). */
    @Query("""
        SELECT DISTINCT s.started_at FROM logged_exercise le
        INNER JOIN session s ON le.session_id = s.id
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL
    """)
    suspend fun prSessionStartTimes(): List<Long>
}
