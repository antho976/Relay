package com.forge.app.data.repo

import androidx.room.withTransaction
import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.RestEventDao
import com.forge.app.data.db.dao.SessionBreakDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.SuggestionOutcomeDao
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.RestEvent
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.SessionBreak
import com.forge.app.data.db.entities.SessionSegment
import com.forge.app.data.db.entities.SuggestionOutcome
import com.forge.app.data.db.types.EffortRating
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.Equipment
import com.forge.app.program.GenerationParams
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Result of [WorkoutRepository.startOrResumeSession].
 *
 * @param created the row was inserted by this call — no segments or logged work can exist yet.
 * @param hasLoggedWork the resumed session already has logged exercises (mid-workout resume).
 */
data class StartedSession(
    val session: Session,
    val created: Boolean,
    val hasLoggedWork: Boolean
)

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
    private val sessionBreakDao: SessionBreakDao,
    private val restEventDao: RestEventDao,
    private val suggestionOutcomeDao: SuggestionOutcomeDao,
    private val sessionSegmentDao: com.forge.app.data.db.dao.SessionSegmentDao,
    private val clock: Clock,
    private val settingsRepo: SettingsRepository,
    private val programRepository: ProgramRepository,
    private val database: com.forge.app.data.db.ForgeDatabase
) {

    private companion object {
        /** A deload "week" — how long the applied deload pauses rotation / tags sessions (#18). */
        const val DELOAD_WEEK_MS = 7L * 24 * 60 * 60 * 1000
    }

    // ─── Session lifecycle ─────────────────────────────────────────────────────

    fun observeActiveSession(): Flow<Session?> = sessionDao.observeActiveSession()

    /** The current in-progress session, if any. Does NOT create one (unlike [startOrResumeSession]). */
    suspend fun activeSession(): Session? = sessionDao.getActiveSession()

    /**
     * Starts a new session, OR resumes the currently-active one if there already
     * is one. App invariant: at most one active session at a time. Returns the full
     * row plus created/has-work flags so the caller doesn't re-query what this
     * method already knows (session-open is the screen's latency-critical path).
     */
    suspend fun startOrResumeSession(dayKey: String): StartedSession {
        sessionDao.getActiveSession()?.let { active ->
            return StartedSession(
                session = active,
                created = false,
                hasLoggedWork = loggedExerciseDao.forSession(active.id).isNotEmpty()
            )
        }
        // Tag sessions started inside the deload week so the deload actually shows up in history and
        // feeds DeloadAdvisor's repeat-suppression (the marker was previously never written — #18).
        val deloadStart = settingsRepo.deloadWeekStartMs.first()
        val inDeloadWeek = deloadStart > 0 && clock.nowMs() - deloadStart in 0 until DELOAD_WEEK_MS
        val session = Session(
            dayKey = dayKey, startedAt = clock.nowMs(), finishedAt = null, deloadMarkedHere = inDeloadWeek
        )
        val id = sessionDao.insert(session)
        return StartedSession(session.copy(id = id), created = true, hasLoggedWork = false)
    }

    /**
     * Marks a session finished and stamps the denormalised volume + PR count so
     * later list views don't need to re-join exercises and sets. Closes the open sitting
     * segment and stamps total ACTIVE seconds (summed across sittings); returns it so the
     * caller can show the real duration. Falls back to wall-clock when no segments exist.
     */
    suspend fun finishSession(sessionId: Long, totalVolumeLb: Double, prCount: Int, setCount: Int): Int {
        val now = clock.nowMs()
        sessionSegmentDao.closeOpen(sessionId, now)
        val session = sessionDao.get(sessionId) ?: error("Session $sessionId not found")
        val segMs = closedSegmentMs(sessionId)
        val activeSeconds = if (segMs > 0) (segMs / 1000L).toInt()
            else ((now - session.startedAt) / 1000L).toInt().coerceAtLeast(0)
        sessionDao.update(
            session.copy(
                finishedAt = now,
                totalVolumeLb = totalVolumeLb,
                prCount = prCount,
                setCount = setCount,
                activeSeconds = activeSeconds
            )
        )
        maybeRotateProgram()
        return activeSeconds
    }

    // ─── Session segments (per-sitting active timing) ──────────────────────────

    /** The session row, for callers that need its real start time / fields. */
    suspend fun session(sessionId: Long): Session? = sessionDao.get(sessionId)

    /** Open a new active sitting for this session. */
    suspend fun openSessionSegment(sessionId: Long, startedAt: Long) =
        sessionSegmentDao.insert(SessionSegment(sessionId = sessionId, startedAt = startedAt))

    /** Close every open segment of a session at [endedAt] (explicit leave / finish). */
    suspend fun closeOpenSegments(sessionId: Long, endedAt: Long) =
        sessionSegmentDao.closeOpen(sessionId, endedAt)

    /**
     * Close segments left open by process death (app killed mid-sitting): end each at the last
     * logged-set activity within it, so the time the app was simply gone isn't counted.
     */
    suspend fun closeDanglingSegments(sessionId: Long) {
        val open = sessionSegmentDao.openForSession(sessionId)
        if (open.isEmpty()) return
        val sets = allSetsForSession(sessionId)
        open.forEach { seg ->
            val lastActivity = sets.filter { it.completedAt >= seg.startedAt }.maxOfOrNull { it.completedAt }
            sessionSegmentDao.close(seg.id, lastActivity ?: seg.startedAt)
        }
    }

    /** Sum of CLOSED segment durations (ms) — the "prior sittings" active time. */
    suspend fun closedSegmentMs(sessionId: Long): Long =
        sessionSegmentDao.forSession(sessionId)
            .mapNotNull { seg -> seg.endedAt?.let { (it - seg.startedAt).coerceAtLeast(0) } }
            .sum()

    /** All segments of a session, oldest first — the export breakdown. */
    suspend fun sessionSegments(sessionId: Long) = sessionSegmentDao.forSession(sessionId)

    /** Rotation (program-unlock Phase 3): re-roll the program every N finished sessions, if enabled. */
    private suspend fun maybeRotateProgram() {
        if (settingsRepo.rotationCadence.first() != "every_n") return
        // Pause auto-rotation inside the deload week — a rotation regenerates a full-volume program
        // and would silently wipe the recovery week mid-deload (seam fix #18). The counter is left
        // untouched, so rotation resumes on the next finish after the deload ends.
        val deloadStart = settingsRepo.deloadWeekStartMs.first()
        if (deloadStart > 0 && clock.nowMs() - deloadStart in 0 until DELOAD_WEEK_MS) return
        val n = settingsRepo.rotationEveryN.first().coerceAtLeast(1)
        val next = settingsRepo.rotationCounter.first() + 1
        if (next < n) {
            settingsRepo.setRotationCounter(next)
            return
        }
        settingsRepo.setRotationCounter(0)
        // Use the user's full saved generation profile (goal/experience/emphasis/problem-areas/
        // priority-muscles/pinned), not a near-empty GenerationParams that dropped them all.
        programRepository.rerollAll()
    }

    /** Persists the comma-separated tag list for a finished session (#107). */
    suspend fun setSessionTags(sessionId: Long, tags: List<String>) {
        val session = sessionDao.get(sessionId) ?: return
        sessionDao.update(session.copy(tags = tags.joinToString(",")))
    }

    suspend fun setDifficultyTag(setId: Long, tag: String?) = loggedSetDao.setDifficultyTag(setId, tag)
    suspend fun setAmrap(setId: Long, v: Boolean) = loggedSetDao.setAmrap(setId, v)
    suspend fun setAssisted(setId: Long, v: Boolean) = loggedSetDao.setAssisted(setId, v)
    suspend fun setToFailure(setId: Long, v: Boolean) = loggedSetDao.setToFailure(setId, v)
    suspend fun setSetType(setId: Long, type: String?) = loggedSetDao.setSetType(setId, type)
    suspend fun setDropAnnotation(setId: Long, annotation: String?) = loggedSetDao.setDropAnnotation(setId, annotation)
    suspend fun setRpe(setId: Long, rpe: Double?) = loggedSetDao.setRpe(setId, rpe)

    suspend fun setSessionType(sessionId: Long, type: String) = sessionDao.setSessionType(sessionId, type)
    suspend fun setUntracked(sessionId: Long, v: Boolean) = sessionDao.setUntracked(sessionId, v)
    suspend fun setJournal(sessionId: Long, text: String) = sessionDao.setJournal(sessionId, text)
    suspend fun setIntensity(sessionId: Long, intensity: String) = sessionDao.setIntensity(sessionId, intensity)

    suspend fun previousSessionForDay(dayKey: String, excludeSessionId: Long): Session? =
        sessionDao.previousFinishedForDay(dayKey, excludeSessionId)

    suspend fun bestPreviousVolumeForDay(dayKey: String, excludeSessionId: Long): Double? =
        sessionDao.maxVolumeForDay(dayKey, excludeSessionId)

    suspend fun discardSession(sessionId: Long) {
        val session = sessionDao.get(sessionId) ?: return
        sessionDao.delete(session) // CASCADE removes LoggedExercises and their LoggedSets
    }

    // ─── Logged exercises ──────────────────────────────────────────────────────

    /**
     * One-shot read of a session's logged exercises. Preferred over a Flow + single
     * collect — the Flow variant spins up an invalidation observer just to grab a
     * single value.
     */
    suspend fun loggedExercisesForSession(sessionId: Long): List<LoggedExercise> =
        loggedExerciseDao.forSession(sessionId)

    suspend fun addExerciseToSession(
        sessionId: Long,
        exerciseId: String,
        orderIndex: Int,
        swappedName: String? = null,
        swappedUnit: String? = null,
        /** Program slot this entry fills when [exerciseId] is a swapped exercise (#11). Null = not swapped. */
        slotId: String? = null
    ): Long = loggedExerciseDao.insert(
        LoggedExercise(
            sessionId = sessionId,
            exerciseId = exerciseId,
            orderIndex = orderIndex,
            swappedName = swappedName,
            swappedUnit = swappedUnit,
            slotId = slotId
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

    /**
     * Apply a session swap to a logged exercise, preserving every other column (superset group, etc.).
     * Re-keys `exercise_id` to the swapped exercise so PRs/stats attribute to the real exercise (#11),
     * stashing the original slot in `slot_id` so the day screen still maps it to its plan slot.
     *
     * Re-keying happens ONLY while the entry has no logged sets: once any set exists, `exercise_id`
     * records what was actually performed and must never change — re-keying it would silently
     * re-attribute those sets to the swapped exercise (false PRs on it, lost history on the original).
     * After sets exist a swap is a name/unit relabel only.
     */
    suspend fun setSessionSwap(loggedExerciseId: Long, swappedName: String?, swappedUnit: String?, swapExerciseId: String) {
        // Atomic check-then-write (SM-2): wrap the set-count read and the re-key in one transaction so a
        // concurrent logSet can't insert a set between them — which would re-key a row that now has real
        // sets and silently mis-attribute them to the swapped exercise.
        database.withTransaction {
            val ex = loggedExerciseDao.get(loggedExerciseId) ?: return@withTransaction
            if (loggedSetDao.countForLoggedExercise(loggedExerciseId) > 0) {
                // Sets exist — a swap is a name/unit relabel only; exercise_id must never change.
                loggedExerciseDao.update(ex.copy(swappedName = swappedName, swappedUnit = swappedUnit))
                return@withTransaction
            }
            val slot = ex.effectiveSlotId
            loggedExerciseDao.update(
                ex.copy(
                    exerciseId = swapExerciseId,
                    // Keep the slot link only while this entry actually differs from its slot — swapping
                    // back to the original exercise clears it (slot == exercise again).
                    slotId = slot.takeIf { it != swapExerciseId },
                    swappedName = swappedName,
                    swappedUnit = swappedUnit
                )
            )
        }
    }

    /**
     * Revert a swapped entry back to its plan slot — but ONLY while it has no logged sets (#11). A swap
     * eagerly creates a logged row, so clearing the swap must un-attribute that still-empty row back to
     * the slot, or the card stays stuck on the swapped exercise. If sets exist, the entry records real
     * performed work and is left untouched.
     */
    suspend fun revertSwapToSlotIfEmpty(loggedExerciseId: Long) {
        val ex = loggedExerciseDao.get(loggedExerciseId) ?: return
        if (loggedSetDao.countForLoggedExercise(loggedExerciseId) > 0) return
        loggedExerciseDao.update(
            ex.copy(exerciseId = ex.effectiveSlotId, slotId = null, swappedName = null, swappedUnit = null)
        )
    }

    suspend fun lastLoggedExerciseBefore(exerciseId: String, excludeSessionId: Long): LoggedExercise? =
        loggedExerciseDao.lastLoggedBefore(exerciseId, excludeSessionId)

    // ─── Sets ──────────────────────────────────────────────────────────────────

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

    suspend fun updateSet(set: LoggedSet) = loggedSetDao.update(set)

    /**
     * Bounded all-time-history reads — replace loading every set ever logged for an
     * exercise on each card build. [excludeLoggedExerciseId] null = exclude nothing.
     */
    suspend fun repMaxFrontierForExercise(exerciseId: String, excludeLoggedExerciseId: Long?) =
        loggedSetDao.repMaxFrontierForExercise(exerciseId, excludeLoggedExerciseId ?: -1L)

    suspend fun hasHistoryForExercise(exerciseId: String, excludeLoggedExerciseId: Long?): Boolean =
        loggedSetDao.hasHistoryForExercise(exerciseId, excludeLoggedExerciseId ?: -1L)

    /** The single best set ever for an exercise (heaviest, most reps at that weight). */
    suspend fun personalBestSet(exerciseId: String): LoggedSet? =
        loggedSetDao.personalBestSet(exerciseId)

    /** Per-session aggregates for one exercise (last N finished sessions, newest first). */
    suspend fun sessionAggregatesForExercise(exerciseId: String, limit: Int = 8) =
        loggedSetDao.sessionAggregatesForExercise(exerciseId, limit)

    /** All sets in a session ordered by completedAt — used to derive actual rest intervals (#82). */
    suspend fun allSetsForSession(sessionId: Long): List<LoggedSet> =
        loggedSetDao.allForSession(sessionId)

    suspend fun maxWeightForExercise(exerciseId: String): Double? =
        loggedSetDao.maxWeightForExercise(exerciseId)

    /** Set or clear the superset group for a logged exercise (#38). */
    suspend fun setSupersetGroup(loggedExerciseId: Long, group: String?) =
        loggedExerciseDao.setSupersetGroup(loggedExerciseId, group)

    // ─── Session breaks (#139) ────────────────────────────────────────────────

    suspend fun logBreak(sessionId: Long, type: String) =
        sessionBreakDao.insert(SessionBreak(sessionId = sessionId, type = type, loggedAt = clock.nowMs()))

    // ─── Realized rest (adaptation engine System 2) ───────────────────────────

    suspend fun logRestEvent(
        sessionId: Long,
        exerciseId: String,
        setIndex: Int,
        plannedSeconds: Int,
        realizedSeconds: Int,
        endedBy: String,
        secondsAdded: Int
    ) = restEventDao.insert(
        RestEvent(
            sessionId = sessionId,
            exerciseId = exerciseId,
            setIndex = setIndex,
            plannedSeconds = plannedSeconds,
            realizedSeconds = realizedSeconds,
            endedBy = endedBy,
            secondsAdded = secondsAdded,
            loggedAt = clock.nowMs()
        )
    )

    /** Recent realized-rest history — RestAdvisor's tuning input. */
    suspend fun recentRestEvents(limit: Int = 200): List<RestEvent> = restEventDao.recent(limit)

    // ─── Suggestion outcomes (auto-coach Phase 2 calibration) ─────────────────

    /** First set logged while a weight suggestion was showing: record suggestion vs reality. */
    suspend fun recordSuggestionOutcome(
        exerciseId: String,
        unitCode: String,
        suggestedLb: Double,
        takenLb: Double,
        reps: Int,
        rangeText: String
    ) = suggestionOutcomeDao.insert(
        SuggestionOutcome(
            exerciseId = exerciseId, unit = unitCode, suggestedLb = suggestedLb,
            takenLb = takenLb, reps = reps, rangeText = rangeText, loggedAt = clock.nowMs()
        )
    )

    /** Recent suggestion outcomes — SuggestionCalibrator's input. */
    suspend fun recentSuggestionOutcomes(limit: Int = 500): List<SuggestionOutcome> =
        suggestionOutcomeDao.recent(limit)

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
