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

    @Query("SELECT * FROM session WHERE draft_id = :draftId LIMIT 1")
    suspend fun forDraft(draftId: String): Session?

    /**
     * Stamp [finishedAt] on a session that is still open, and report whether THIS call was the one
     * that did it (1) or found it already finished (0).
     *
     * The finish path used to read the row, check `finishedAt == null`, and then write a whole
     * Session built from that read. Two callers — a double-tapped FINISH, a finish racing the
     * orphan-recovery pass, a wrist command arriving as the phone finishes — could both pass the
     * check and both proceed to stamp the session, rotate the program, refresh state and mirror the
     * workout to Health Connect. The full-row write had a second failure mode of its own: it
     * carried every column from a snapshot taken before the check, so an is_untracked, session_type
     * or intensity change made in between was silently reverted.
     *
     * Both are the same fix. The winner is decided by SQLite, and only the winner does the rest.
     */
    @Query("UPDATE session SET finished_at = :finishedAt WHERE id = :id AND finished_at IS NULL")
    suspend fun finishIfUnfinished(id: Long, finishedAt: Long): Int

    /** The finish summary, written after [finishIfUnfinished] has named a winner. */
    @Query("""
        UPDATE session
        SET total_volume_lb = :totalVolumeLb, pr_count = :prCount,
            set_count = :setCount, active_seconds = :activeSeconds
        WHERE id = :id
    """)
    suspend fun setFinishTotals(
        id: Long,
        totalVolumeLb: Double,
        prCount: Int,
        setCount: Int,
        activeSeconds: Int
    )

    /** Returns the in-progress session if one exists. App logic limits this to at most one. */
    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    suspend fun getActiveSession(): Session?

    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    fun observeActiveSession(): Flow<Session?>

    /**
     * Recent finished sessions, INCLUSIVE of untracked ones — for history lists, which is what an
     * untracked session still belongs in. Anything that feeds a schedule, a comparison or a
     * progression signal wants [observeRecentTracked] instead.
     */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY finished_at DESC LIMIT :limit")
    fun observeRecent(limit: Int = 10): Flow<List<Session>>

    /**
     * The same window, tracked only.
     *
     * The inclusive flow was driving the lit week dots, the best-session tile, today's trained keys
     * and next-up resolution — while the workout COUNT and volume beside them were already
     * tracked-only. So a week with one untracked session lit three dots above the words "2
     * workouts", and a session the user had explicitly excluded from their record still consumed
     * today's schedule slot and decided what came next.
     *
     * A DAO variant rather than an in-memory filter, so the window is 120 TRACKED rows: filtering
     * after the fact would silently shorten it for anyone with a run of untracked sessions.
     */
    @Query("""
        SELECT * FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
        ORDER BY finished_at DESC LIMIT :limit
    """)
    fun observeRecentTracked(limit: Int = 10): Flow<List<Session>>

    /**
     * Every finished session's finish instant, newest first — the column alone, no rows.
     *
     * The day-streak walk used to read it out of `observeRecent(120)`, which terminates the walk
     * when the WINDOW runs out rather than when a real rest gap appears: anyone training twice a
     * day fills 120 rows inside about two months, and their streak silently stopped growing there
     * while the Profile's trophy streak (which reads all sessions) kept counting past it. Unbounded
     * is affordable here because it is one Long per session, not a row.
     */
    @Query("""
        SELECT finished_at FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
        ORDER BY finished_at DESC
    """)
    fun observeFinishedAts(): Flow<List<Long>>

    /** One-shot [observeFinishedAts], for the streak read that doesn't want a subscription. */
    @Query("""
        SELECT finished_at FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
        ORDER BY finished_at DESC
    """)
    suspend fun finishedAts(): List<Long>

    /** Sessions that count toward progression — the Stats/Overview "workouts logged" figure. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL AND is_untracked = 0")
    fun observeFinishedCount(): Flow<Int>

    /** Used by the "Full Week" trophy (all 4 days trained). */
    @Query("SELECT DISTINCT day_key FROM session WHERE finished_at IS NOT NULL AND is_untracked = 0")
    suspend fun distinctDayKeysTrained(): List<String>

    /**
     * EVERY finished session, untracked included — the "does this install hold any training data"
     * question, which backup warnings and the demo-data gate ask. Deliberately NOT the progression
     * count: someone whose only history is untracked still has history worth backing up, and must
     * not have demo data seeded over it.
     */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL")
    suspend fun finishedCount(): Int

    /** [finishedCount] restricted to sessions that count toward progression. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL AND is_untracked = 0")
    suspend fun trackedFinishedCount(): Int

    /**
     * Day key of the most recently finished TRACKED session — the "next up" anchor for the training
     * reminder and the widget.
     *
     * This was the last inclusive query left feeding `resolveNextUp`, and it sat beside two that
     * already filtered: `finishedDayKeysSince` (today's trained keys) and `finishedAtsSince` (the
     * week dots) both exclude untracked work. So one excluded session moved the rotation on for the
     * reminder and the widget while every other input to the same decision ignored it — the two
     * surfaces disagreed with the app about what came next.
     *
     * The widget's separate "has this user ever trained" check stays inclusive: that one is asking
     * whether any data exists, not what to do next.
     */
    @Query("""
        SELECT day_key FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
        ORDER BY finished_at DESC LIMIT 1
    """)
    suspend fun lastFinishedDayKey(): String?

    /** Whether ANY finished session exists, untracked included — the widget's zero-state check. */
    @Query("SELECT EXISTS(SELECT 1 FROM session WHERE finished_at IS NOT NULL)")
    suspend fun hasAnyFinishedSession(): Boolean

    /**
     * Day keys of sessions finished since [sinceMs] — the widget's "trained today" set, which feeds
     * `WeeklySchedule.resolveNextUp`. Tracked only, matching what DirectiveRepository already
     * filters for in Kotlin: an untracked session is excluded from suggestions by contract, and the
     * widget and the app disagreeing about what is next up is worse than either answer.
     */
    @Query("SELECT day_key FROM session WHERE finished_at >= :sinceMs AND is_untracked = 0")
    suspend fun finishedDayKeysSince(sinceMs: Long): List<String>

    /** Finish instants since [sinceMs] — the widget's Mon–Sun dot row, without loading entities. */
    @Query("SELECT finished_at FROM session WHERE finished_at >= :sinceMs AND is_untracked = 0")
    suspend fun finishedAtsSince(sinceMs: Long): List<Long>

    /** Previous same-day session, with a stable id tie-break for equal finish timestamps. */
    @Query("""
        SELECT * FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL AND is_untracked = 0
          AND id != :excludeSessionId
          AND (finished_at < :beforeFinishedAt OR (finished_at = :beforeFinishedAt AND id < :excludeSessionId))
        ORDER BY finished_at DESC, id DESC LIMIT 1
    """)
    suspend fun previousFinishedForDay(
        dayKey: String, excludeSessionId: Long, beforeFinishedAt: Long = Long.MAX_VALUE
    ): Session?

    /** Best (highest) volume ever recorded for a given day, excluding the current session (#53). */
    @Query("""
        SELECT MAX(total_volume_lb) FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL AND is_untracked = 0
          AND id != :excludeSessionId
    """)
    suspend fun maxVolumeForDay(dayKey: String, excludeSessionId: Long): Double?

    /**
     * Rolling-window queries for the Overview weekly stats strip. Window on started_at
     * (the day you trained) to stay consistent with aggregateInRange / finishedInRange /
     * the week-comparison strip; only finished sessions are counted. The AI-export query
     * below intentionally windows on finished_at instead — see its doc.
     */
    @Query("""
        SELECT COUNT(*) FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0 AND started_at >= :sinceEpochMs
    """)
    fun observeFinishedCountSince(sinceEpochMs: Long): Flow<Int>

    @Query("""
        SELECT SUM(total_volume_lb) FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0 AND started_at >= :sinceEpochMs
    """)
    fun observeVolumeSince(sinceEpochMs: Long): Flow<Double?>

    /**
     * Earliest finished TRACKED session — the "first full month" milestone (#56).
     *
     * A milestone is an achievement, and an untracked session is one the user asked not to count.
     * It also decided when the month started, so an excluded workout could award the milestone a
     * month early.
     */
    @Query("SELECT MIN(started_at) FROM session WHERE finished_at IS NOT NULL AND is_untracked = 0")
    fun observeFirstFinishedSessionStartedAt(): Flow<Long?>

    /**
     * Nearest finished TRACKED session whose start falls within [fromMs]–[toMs].
     * Orders by proximity to [targetMs] so the closest day to the anniversary is returned.
     * Used for the "On this day" memory card (#106).
     *
     * Tracked only, because this is not the history screen: the memory is delivered as an
     * engagement hook on the Overview and inside the weekly recap notification, alongside numbers
     * that already exclude untracked work. A session someone deliberately kept out of their record
     * resurfacing a year later as "on this day" is the same category error as counting it.
     */
    @Query("""
        SELECT * FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
          AND started_at BETWEEN :fromMs AND :toMs
        ORDER BY ABS(started_at - :targetMs) ASC
        LIMIT 1
    """)
    suspend fun sessionNearDate(targetMs: Long, fromMs: Long, toMs: Long): Session?

    /** All finished sessions ordered oldest-first — used by trophy snapshot computations. */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY started_at ASC")
    suspend fun allFinished(): List<Session>

    /**
     * Sessions finished within [fromMs, toMs), INCLUSIVE of untracked ones. Used by the monthly
     * calendar (#54), which is a history surface. Signals want [finishedInRangeTracked].
     */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL AND started_at >= :fromMs AND started_at < :toMs ORDER BY started_at ASC")
    suspend fun finishedInRange(fromMs: Long, toMs: Long): List<Session>

    /**
     * The same range, tracked only.
     *
     * The training reminder read the inclusive one, so a workout the user excluded from their
     * record still suppressed that day's nudge and fed the schedule resolution behind it. The
     * year-in-review read it too and filtered in memory afterwards, which is the same query stated
     * twice — once here and once at the call site.
     */
    @Query("""
        SELECT * FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
          AND started_at >= :fromMs AND started_at < :toMs
        ORDER BY started_at ASC
    """)
    suspend fun finishedInRangeTracked(fromMs: Long, toMs: Long): List<Session>

    /** Recovery is anchored to completion, including sessions that started before midnight. */
    @Query("SELECT * FROM session WHERE finished_at >= :sinceMs AND is_untracked = 0")
    suspend fun finishedForRecoverySince(sinceMs: Long): List<Session>

    /**
     * Sessions whose *finish* time falls in [fromMs, toMs) — used by the weekly AI export so a
     * session that started before the window boundary but finished inside it is still included.
     */
    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL AND finished_at >= :fromMs AND finished_at < :toMs ORDER BY started_at ASC")
    suspend fun finishedByFinishTimeInRange(fromMs: Long, toMs: Long): List<Session>

    /**
     * Every session starting inside [fromMs, toMs), with the instant each one occupies — the whole
     * nudge window for one imported workout, in a single query.
     *
     * The importer walked this window a slot at a time and only ever forwards from wherever the
     * previous workout in the same file had stopped, so re-importing the same file with its
     * workouts in a different order could start the search PAST the slot a workout already
     * occupied and insert it a second time. Handing back the window lets the guard check every
     * candidate regardless of order, and costs one query instead of up to sixty.
     */
    @Query("SELECT id, started_at FROM session WHERE started_at >= :fromMs AND started_at < :toMs")
    suspend fun startRefsInRange(fromMs: Long, toMs: Long): List<SessionStartRef>

    /** Every session id. The reset captures these BEFORE [deleteAll] so the Health Connect mirrors
     *  keyed on them can still be addressed once the rows are gone (M-02). */
    @Query("SELECT id FROM session")
    suspend fun allIds(): List<Long>

    /** Deletes all sessions (CASCADE removes LoggedExercise → LoggedSet, rest events, segments, breaks
     *  and HR samples; MoodEntry is only SET NULL — see [MoodDao.deleteAll]). For reset (#119). */
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

    /** Aggregate stats for a window — for week/month comparisons (#34, #130). */
    @Query("""
        SELECT COUNT(*) AS session_count,
               SUM(total_volume_lb) AS total_volume,
               SUM(pr_count) AS total_prs,
               SUM(set_count) AS total_sets
        FROM session
        WHERE finished_at IS NOT NULL AND is_untracked = 0
          AND started_at >= :fromMs AND started_at < :toMs
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
        FROM session WHERE finished_at IS NOT NULL AND is_untracked = 0 ORDER BY started_at ASC
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
        FROM session WHERE finished_at IS NOT NULL AND total_volume_lb IS NOT NULL AND is_untracked = 0
        GROUP BY day_key
    """)
    suspend fun avgMaxVolumeByDayKey(): List<DayVolumeStats>

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
        WHERE le.was_pr = 1 AND s.finished_at IS NOT NULL AND s.is_untracked = 0
    """)
    suspend fun prSessionStartTimes(): List<Long>

    /**
     * One string that changes whenever any FINISHED session, or any exercise or set logged under one,
     * changes — and not when only the workout in progress does.
     *
     * Computed inside SQLite as id-weighted aggregates: a scan, but with nothing materialised, where
     * the engine used to load all three tables in full only to compare them with the last load.
     * Weighting every column by the row id means an edit, or two values trading places, still moves
     * the total. Epoch columns are reduced modulo a prime so the products stay inside a double's
     * exact range.
     */
    @Query("""
        SELECT
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * (
            (finished_at % 1000003) * 3 + (started_at % 1000003) * 5 + is_untracked * 7
            + pr_count * 11 + set_count * 13 + IFNULL(total_volume_lb, 0) * 17 + deload_marked_here * 19
            + active_seconds * 23 + LENGTH(day_key) * 29 + IFNULL(LENGTH(tags), 0) * 31
            + IFNULL(LENGTH(session_type), 0) * 37 + IFNULL(LENGTH(intensity), 0) * 41
            + IFNULL(LENGTH(journal), 0) * 43))
            FROM session WHERE finished_at IS NOT NULL), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(le.id * (
            le.session_id * 3 + le.order_index * 5 + LENGTH(le.exercise_id) * 7
            + IFNULL(LENGTH(le.swapped_name), 0) * 11 + IFNULL(LENGTH(le.swapped_unit), 0) * 13
            + IFNULL(LENGTH(le.difficulty), 0) * 17 + le.hit_full_target * 19 + le.was_pr * 23
            + le.skipped * 29 + IFNULL(LENGTH(le.superset_group), 0) * 31
            + IFNULL(LENGTH(le.slot_id), 0) * 37 + IFNULL(LENGTH(le.note), 0) * 41))
            FROM logged_exercise le INNER JOIN session s ON le.session_id = s.id
            WHERE s.finished_at IS NOT NULL), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(ls.id * (
            ls.logged_exercise_id * 3 + ls.set_index * 5 + IFNULL(ls.weight_lb, 0) * 7 + ls.reps * 11
            + (ls.completed_at % 1000003) * 13 + ls.is_amrap * 17 + ls.is_assisted * 19
            + ls.to_failure * 23 + IFNULL(ls.rpe, 0) * 29 + IFNULL(ls.duration_seconds, 0) * 31
            + IFNULL(LENGTH(ls.set_type), 0) * 37 + IFNULL(LENGTH(ls.difficulty_tag), 0) * 41
            + IFNULL(LENGTH(ls.weight_text), 0) * 43 + IFNULL(LENGTH(ls.drop_annotation), 0) * 47))
            FROM logged_set ls
            INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
            INNER JOIN session s ON le.session_id = s.id
            WHERE s.finished_at IS NOT NULL), '')
        AS fingerprint
    """)
    suspend fun finishedHistoryFingerprint(): String

    /** [finishedHistoryFingerprint], re-read whenever one of its three tables is written. */
    @Query("""
        SELECT
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * (
            (finished_at % 1000003) * 3 + (started_at % 1000003) * 5 + is_untracked * 7
            + pr_count * 11 + set_count * 13 + IFNULL(total_volume_lb, 0) * 17 + deload_marked_here * 19
            + active_seconds * 23 + LENGTH(day_key) * 29 + IFNULL(LENGTH(tags), 0) * 31
            + IFNULL(LENGTH(session_type), 0) * 37 + IFNULL(LENGTH(intensity), 0) * 41
            + IFNULL(LENGTH(journal), 0) * 43))
            FROM session WHERE finished_at IS NOT NULL), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(le.id * (
            le.session_id * 3 + le.order_index * 5 + LENGTH(le.exercise_id) * 7
            + IFNULL(LENGTH(le.swapped_name), 0) * 11 + IFNULL(LENGTH(le.swapped_unit), 0) * 13
            + IFNULL(LENGTH(le.difficulty), 0) * 17 + le.hit_full_target * 19 + le.was_pr * 23
            + le.skipped * 29 + IFNULL(LENGTH(le.superset_group), 0) * 31
            + IFNULL(LENGTH(le.slot_id), 0) * 37 + IFNULL(LENGTH(le.note), 0) * 41))
            FROM logged_exercise le INNER JOIN session s ON le.session_id = s.id
            WHERE s.finished_at IS NOT NULL), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(ls.id * (
            ls.logged_exercise_id * 3 + ls.set_index * 5 + IFNULL(ls.weight_lb, 0) * 7 + ls.reps * 11
            + (ls.completed_at % 1000003) * 13 + ls.is_amrap * 17 + ls.is_assisted * 19
            + ls.to_failure * 23 + IFNULL(ls.rpe, 0) * 29 + IFNULL(ls.duration_seconds, 0) * 31
            + IFNULL(LENGTH(ls.set_type), 0) * 37 + IFNULL(LENGTH(ls.difficulty_tag), 0) * 41
            + IFNULL(LENGTH(ls.weight_text), 0) * 43 + IFNULL(LENGTH(ls.drop_annotation), 0) * 47))
            FROM logged_set ls
            INNER JOIN logged_exercise le ON ls.logged_exercise_id = le.id
            INNER JOIN session s ON le.session_id = s.id
            WHERE s.finished_at IS NOT NULL), '')
        AS fingerprint
    """)
    fun observeFinishedHistoryFingerprint(): Flow<String>

    /**
     * The same, for every other table the adaptation snapshot reads: moods, cardio, bodyweight,
     * check-ins, injuries, vacations and the two customization tables. The larger ones are folded
     * like [finishedHistoryFingerprint]; the small ones are concatenated whole.
     */
    @Query("""
        SELECT
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * (IFNULL(session_id, 0) * 3 + (recorded_at % 1000003) * 5))
            || ':' || IFNULL(group_concat(quote(mood) || quote(day_key), ','), '') FROM mood_entry), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * ((date % 1000003) * 3 + duration_min * 5
            + IFNULL(distance_km, 0) * 7 + IFNULL(interval_count, 0) * 11 + IFNULL(incline_pct, 0) * 13
            + IFNULL(laps, 0) * 17 + IFNULL(elevation_m, 0) * 19 + IFNULL(LENGTH(note), 0) * 23))
            || ':' || IFNULL(group_concat(quote(type) || quote(effort) || quote(rest_reason)
                || quote(hr_zone) || quote(conditions), ','), '') FROM cardio_entry), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * (weight_lb * 3 + (recorded_at % 1000003) * 5
            + LENGTH(date_key) * 7 + IFNULL(LENGTH(note), 0) * 11)) FROM bodyweight_entry), '')
        || '|' ||
        IFNULL((SELECT COUNT(*) || ':' || TOTAL(id * (IFNULL(sleep_quality, 0) * 3 + IFNULL(soreness, 0) * 5
            + IFNULL(stress, 0) * 7 + IFNULL(motivation, 0) * 11 + sick * 13 + skipped * 17
            + (recorded_at % 1000003) * 19))
            || ':' || IFNULL(group_concat(quote(date_key) || quote(sore_muscles), ','), '') FROM checkin_entry), '')
        || '|' ||
        IFNULL((SELECT group_concat(quote(id) || quote(scope) || quote(target_key) || quote(note)
            || quote(started_at) || quote(cleared_at), ',') FROM injury_restriction), '')
        || '|' ||
        IFNULL((SELECT group_concat(quote(id) || quote(start_date) || quote(end_date), ',')
            FROM vacation_period), '')
        || '|' ||
        IFNULL((SELECT group_concat(quote(day_key) || quote(exercise_id) || quote(custom_name)
            || quote(custom_muscle) || quote(rep_range_override) || quote(sets_override)
            || quote(order_override) || quote(removed) || quote(source), ',') FROM program_customization), '')
        || '|' ||
        IFNULL((SELECT group_concat(quote(exercise_id) || quote(swapped_name) || quote(swapped_unit)
            || quote(rest_timer_override_seconds) || quote(pinned_note) || quote(source)
            || quote(swapped_exercise_id), ',') FROM exercise_customization), '')
        AS fingerprint
    """)
    suspend fun engineSideTablesFingerprint(): String
}

/** A session's id and the instant it starts on — [SessionDao.startRefsInRange]. */
data class SessionStartRef(
    val id: Long,
    @androidx.room.ColumnInfo(name = "started_at") val startedAt: Long
)
