package com.forge.app.data.repo

import androidx.room.withTransaction
import com.forge.app.core.time.Clock
import com.forge.app.core.time.deloadWeekEndMs
import com.forge.app.core.time.deloadWeekStartMs
import com.forge.app.data.db.dao.BodyweightDao
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
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.coach.LifeEvents
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.health.ActiveCalorieEstimator
import com.forge.app.domain.units.MAX_HOLD_SECONDS
import com.forge.app.domain.pr.PrDetector
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.program.Equipment
import com.forge.app.program.GenerationParams
import com.forge.app.program.Program
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Result of [WorkoutRepository.startOrResumeSession].
 *
 * @param created true when the row was inserted by this call (fresh start); false when an existing
 *   active session was resumed. Callers skip the warmup gate on resume (created == false).
 */
data class StartedSession(
    val session: Session,
    val created: Boolean
)

/**
 * Sane upper bound on a single logged set's reps. A 4-digit fat-finger (numeric-keyboard
 * autocomplete can drop a "9999") would otherwise inflate Epley e1RM by ×(reps/30) and volume by
 * ×reps — permanently poisoning every downstream stat, PR, and coach signal. 999 kills the typo
 * without rejecting any conceivable real set; the floor of 0 absorbs a stray negative.
 */
internal const val MAX_LOGGED_REPS = 999

/** Clamp a set's reps to a sane, non-poisoning range at the write boundary. Pure + testable. */
internal fun sanitizeReps(reps: Int): Int = reps.coerceIn(0, MAX_LOGGED_REPS)

/**
 * Sane upper bound on a single logged set's weight, in pounds.
 *
 * Reps and holds were clamped at the write boundary; weight was not, and the only guard on it was
 * the jump-confirm dialog, which is skipped outright on the FIRST set of an exercise (there is no
 * prior weight to compare against). So a fat-fingered 1000000000 on a brand-new lift went straight
 * into the row: the session's volume became 1e9, XpEngine granted five million XP, and every volume
 * chart's y-axis was flattened for good. Deleting the set repairs the charts; the XP and any
 * trophies it unlocked stay.
 *
 * 2000 lb clears the heaviest loaded machine anyone will put in a log — a leg press stacked past
 * anything a competitive lifter moves — while killing the digit-slip that poisons every aggregate.
 */
internal const val MAX_LOGGED_WEIGHT_LB = 2000.0

/** Clamp a set's weight to a sane, non-poisoning range at the write boundary. Pure + testable. */
internal fun sanitizeWeightLb(weightLb: Double?): Double? = weightLb?.coerceIn(0.0, MAX_LOGGED_WEIGHT_LB)

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
    private val bodyweightDao: BodyweightDao,
    private val sessionHrSampleDao: com.forge.app.data.db.dao.SessionHrSampleDao,
    private val health: HealthConnectManager,
    private val clock: Clock,
    private val settingsRepo: SettingsRepository,
    private val programRepository: ProgramRepository,
    private val wearHrIngest: com.forge.app.service.wear.WearHrIngest,
    private val database: com.forge.app.data.db.ForgeDatabase,
    @dagger.hilt.android.qualifiers.ApplicationContext private val appContext: android.content.Context
) {

    /**
     * Redraw the home-screen widget after a change to what it claims.
     *
     * The widget renders the active session, the streak and the week's dots — all session-derived —
     * yet the only refresh was a program regeneration plus the system's periodic pass, whose floor
     * is 30 minutes and which Doze defers indefinitely. So "WORKOUT IN PROGRESS · Tap to resume"
     * could sit on the home screen for an hour after the workout ended, and the streak stayed a
     * workout behind all day. These four moments are the ones that change the answer.
     */
    private suspend fun refreshWidget() = com.forge.app.widget.refreshForgeWidgets(appContext)

    /**
     * The window an applied deload governs: it pauses rotation, tags the sessions logged inside it,
     * and ends by restoring full volume. Monday-anchored (see [deloadWeekStartMs]) so a deload can
     * never tag a session belonging to the next block.
     */
    private fun deloadWindow(appliedMs: Long): LongRange =
        deloadWeekStartMs(appliedMs) until deloadWeekEndMs(appliedMs)

    // ─── Session lifecycle ─────────────────────────────────────────────────────

    fun observeActiveSession(): Flow<Session?> = sessionDao.observeActiveSession()

    /** Every set in a session, reactive off Room invalidation — the live day screen watches this
     *  so a set written by another surface (the wrist) shows up without a manual refresh. */
    fun observeSetsForSession(sessionId: Long): Flow<List<LoggedSet>> =
        loggedSetDao.observeAllForSession(sessionId)

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
            // Don't resume a "zombie" session whose day the program no longer has — resuming it would
            // load the wrong day's plan (Program.day() falls back to the first day) and log under a stale
            // key. Resolve it here too (not just at Overview-open, E8) so every start path upholds the
            // invariant, then fall through to a fresh start. Gate on isLoaded so a not-yet-loaded program
            // (seed keys only, on a cold start) can't mis-classify a perfectly valid live session.
            val isZombie = Program.isLoaded && active.dayKey !in Program.dayKeys
            if (!isZombie) {
                return StartedSession(session = active, created = false)
            }
            resolveOrphanSession(Program.dayKeys.toSet())
        }
        // Tag sessions started inside the deload week so the deload actually shows up in history and
        // feeds DeloadAdvisor's repeat-suppression (the marker was previously never written — #18).
        val deloadStart = settingsRepo.deloadWeekStartMs.first()
        val inDeloadWeek = deloadStart > 0 && clock.nowMs() in deloadWindow(deloadStart)
        // First session after a real break: tag it FIRST_BACK (Coach v3 B1). The enum has existed
        // since #109 with nothing to write it; this is its writer. The tag keeps the return week out
        // of stall and fatigue reads (A1's filter), so easing back in never reads as a plateau.
        val sessionType = if (isFirstBack()) SessionType.FIRST_BACK.key else SessionType.NORMAL.key
        val session = Session(
            dayKey = dayKey, startedAt = clock.nowMs(), finishedAt = null, deloadMarkedHere = inDeloadWeek,
            sessionType = sessionType
        )
        val id = sessionDao.insert(session)
        refreshWidget()
        return StartedSession(session.copy(id = id), created = true)
    }

    /**
     * Is this the first session back after a layoff? True only for the session that RESUMES
     * training — once a return session exists, [LifeEvents] reports the ramp rather than the gap,
     * so the tag lands on exactly one session (B1).
     */
    private suspend fun isFirstBack(): Boolean {
        val finished = sessionDao.allFinished().filter { !it.isUntracked }
        val layoff = LifeEvents.layoff(finished, clock.nowMs())
        return layoff?.away == true
    }

    /**
     * Create a fresh freestyle ("go with the flow") session and return its id. Unlike
     * [startOrResumeSession] this never resumes an existing active session — a freestyle log is a
     * self-contained, log-after-the-fact workout that the caller fills and finishes in one go. Keyed
     * by [Program.FREESTYLE_DAY_KEY], which resolves to "Open workout" on display surfaces.
     *
     * [startedAt] is when the user opened the logger; since a freestyle log opens no active sitting
     * segment, [finishSession] falls back to wall-clock (finish − [startedAt]) for the duration — so
     * passing the open time records the real time spent logging instead of ~0.
     */
    /**
     * Run [block] in one database transaction — for callers that compose several of this
     * repository's writes into a single unit of work (the freestyle logger writes a session, its
     * exercises and every set). Without it, a failure or a cancellation part-way through leaves a
     * torn session in history.
     */
    suspend fun <T> inTransaction(block: suspend () -> T): T = database.withTransaction(block)

    suspend fun createFreestyleSession(startedAt: Long = clock.nowMs()): Long {
        val session = Session(
            dayKey = Program.FREESTYLE_DAY_KEY, startedAt = startedAt, finishedAt = null
        )
        return sessionDao.insert(session)
    }

    /**
     * "Log again today" (GYMAP-36): duplicate a past finished session as a fresh freestyle session dated
     * now — a full-fidelity copy so a repeated workout can be re-logged without re-entering it. Every
     * non-skipped logged exercise and each of its sets is copied verbatim (weight, reps, set type, RPE,
     * timed hold, drop/AMRAP/failure/assist markers), preserving order; only per-session EVALUATIONS are
     * dropped (difficulty rating, hit-target, PR flag, and the day-specific exercise note) since those
     * are re-earned on the real day, not repeated. Keyed freestyle ("Open workout") like every other
     * re-log-a-past-workout path (GYMAP-48), so slotId — a program-slot link meaningless in a freestyle
     * log — is dropped too.
     *
     * Stamped finished with denormalised volume/setCount but WITHOUT [finishSession]'s side effects (no
     * program rotation, no Health Connect calorie mirror) — a re-log is data entry, not a live finish.
     * prCount is 0 by construction: a copy of existing history can only ever TIE an all-time max it is
     * duplicating, never beat it, so no copied set is a PR. Returns the new session id, or null when the
     * source is missing or logged nothing to copy (the caller then has nothing to re-log).
     */
    suspend fun reLogSession(sourceSessionId: Long, startedAt: Long = clock.nowMs()): Long? =
        database.withTransaction {
            val source = sessionDao.get(sourceSessionId) ?: return@withTransaction null
            val sourceExercises = loggedExerciseDao.forSession(sourceSessionId).filterNot { it.skipped }
            if (sourceExercises.isEmpty()) return@withTransaction null

            val newSession = Session(dayKey = Program.FREESTYLE_DAY_KEY, startedAt = startedAt, finishedAt = null)
            val newSessionId = sessionDao.insert(newSession)
            sourceExercises.forEach { le ->
                val newLeId = loggedExerciseDao.insert(
                    LoggedExercise(
                        sessionId = newSessionId,
                        exerciseId = le.exerciseId,
                        orderIndex = le.orderIndex,
                        // Keep the performed name/unit override + superset grouping so the copy reads
                        // identically; drop slotId + every per-session evaluation (see kdoc).
                        swappedName = le.swappedName,
                        swappedUnit = le.swappedUnit,
                        supersetGroup = le.supersetGroup
                    )
                )
                val copies = loggedSetDao.forLoggedExercise(le.id)
                    .map { it.copy(id = 0, loggedExerciseId = newLeId, completedAt = startedAt) }
                if (copies.isNotEmpty()) loggedSetDao.insertAll(copies)
            }

            val newSets = loggedSetDao.allForSession(newSessionId)
            sessionDao.update(
                newSession.copy(
                    id = newSessionId,
                    finishedAt = clock.nowMs(),
                    totalVolumeLb = VolumeCalculator.sessionVolumeLb(newSets),
                    setCount = newSets.size,
                    // Inherit the original's active time so the re-log shows a realistic duration
                    // (a duplicated workout took about as long) instead of ~0.
                    activeSeconds = source.activeSeconds
                )
            )
            newSessionId
        }

    /**
     * The sets from the most recent OTHER time this exercise was performed — powers the freestyle
     * logger's "copy last time" panel. Empty when it has never been logged before. excludeSessionId is
     * -1 because a freestyle log has no persisted session id while it's being filled in.
     */
    suspend fun lastPerformanceSets(exerciseId: String): List<LoggedSet> {
        val last = loggedExerciseDao.lastLoggedBefore(exerciseId, -1L) ?: return emptyList()
        return loggedSetDao.forLoggedExercise(last.id).sortedBy { it.setIndex }
    }

    /**
     * Freestyle logging persists sets directly via [logSet], bypassing the live day screen's PR pass,
     * so flag [LoggedExercise.wasPr] here the same way the day screen does: compare each just-logged
     * set for [loggedExerciseId] against the all-time rep-max frontier for [exerciseId] (excluding this
     * entry) via [PrDetector], running it forward so later sets also beat earlier ones. Persists wasPr
     * and returns whether this exercise set a PR — so the caller can tally the session's prCount and
     * the lift feeds the lifetime PR count / PRs list like any other workout.
     */
    suspend fun flagPrForLoggedExercise(loggedExerciseId: Long, exerciseId: String): Boolean {
        // The frontier (per-rep max weight, excluding this entry) substitutes exactly for full history
        // in PrDetector, which reads only weightLb/reps/isAssisted — same wrapping the day screen uses.
        val running = repMaxFrontierForExercise(exerciseId, loggedExerciseId).map { row ->
            LoggedSet(loggedExerciseId = 0L, setIndex = 0, weightText = "", weightLb = row.weightLb, reps = row.reps, completedAt = 0L)
        }.toMutableList()
        var wasPr = false
        setsFor(loggedExerciseId).sortedBy { it.setIndex }.forEach { s ->
            if (!s.isAssisted && PrDetector.isPr(running, s.weightLb, s.reps, s.durationSeconds)) wasPr = true
            running.add(s)
        }
        // One column, not a whole row rebuilt from a read taken before this pass ran: a rating, a
        // note or a skip landing while the recalculation was in flight used to be overwritten by
        // that stale snapshot. Only written when true, as before — this pass never clears the flag.
        if (wasPr) loggedExerciseDao.setWasPr(loggedExerciseId, true)
        return wasPr
    }

    /**
     * Marks a session finished and stamps the denormalised volume + PR count so
     * later list views don't need to re-join exercises and sets. Closes the open sitting
     * segment and stamps total ACTIVE seconds (summed across sittings); returns it so the
     * caller can show the real duration. Falls back to wall-clock when no segments exist.
     */
    /**
     * Close the session and stamp its denormalised totals.
     *
     * The totals are derived HERE, from the database, rather than taken as parameters. Callers used
     * to compute them from `_state.value.exercises` — the day screen's in-memory list — and those
     * three columns drive history, Stats and the Profile's lifetime figures permanently. Any set
     * missing from that list at the moment Finish was tapped (logged on the watch, lost to one of
     * the refresh races, or written by a concurrent coroutine) was erased from the user's totals
     * even though its row was sitting in `logged_set` the whole time.
     *
     * [resolveOrphanSession] already derived them this way. Now there is one way to do it, and no
     * way for a caller to pass a number that disagrees with the rows.
     */
    suspend fun finishSession(sessionId: Long): Int {
        val now = clock.nowMs()
        // EVERY local change a finish makes, in one transaction.
        //
        // The winner decision alone was already atomic, and that fixed the double-finish. It left a
        // worse failure behind it: `finished_at` was stamped by its own statement, and the segment
        // close and the derived totals ran after it as separate writes. A crash, a cancelled
        // coroutine or a throw anywhere in between left a session that is FINISHED and has no
        // active seconds, no volume, no PR count, no set count and an open segment still running —
        // and a retry took the `== 0` branch and returned early, because `finished_at` was no
        // longer null. The session was permanently wrong and nothing could repair it.
        //
        // Committing the stamp together with everything derived from it means the two states a
        // crash can leave are "not finished" (retry converges) and "finished and complete".
        val outcome = database.withTransaction {
            // ONE writer finishes a session, and SQLite decides which. A read, a `finishedAt == null`
            // check and a write are three steps with two gaps in them, and a double-tapped FINISH, a
            // finish racing the orphan-recovery pass, or a wrist command arriving as the phone
            // finishes could both pass the check: the session was stamped twice, counted twice
            // toward the program rotation, and mirrored to Health Connect twice. The conditional
            // UPDATE collapses the check and the write into one statement, and its affected-row
            // count names the winner.
            if (sessionDao.finishIfUnfinished(sessionId, now) == 0) {
                // Already finished — report the duration that was stamped and do nothing else.
                FinishOutcome.AlreadyFinished(sessionDao.get(sessionId)?.activeSeconds ?: 0)
            } else {
                sessionSegmentDao.closeOpen(sessionId, now)
                // Soft-fail instead of crashing if the row vanished (e.g. a concurrent program
                // regenerate discarded the active session mid-finish).
                val session = sessionDao.get(sessionId)
                if (session == null) FinishOutcome.Gone else {
                    val segMs = closedSegmentMs(sessionId)
                    val activeSeconds = if (segMs > 0) (segMs / 1000L).toInt()
                        else ((now - session.startedAt) / 1000L).toInt().coerceAtLeast(0)
                    val sets = loggedSetDao.allForSession(sessionId)
                    val prCount = loggedExerciseDao.forSession(sessionId).count { it.wasPr }
                    // Targeted columns rather than a full-row write built from a snapshot taken
                    // before the finish. The whole-entity write carried is_untracked, session_type,
                    // intensity, tags and journal from that older read, so marking a session
                    // untracked (or tagging it, or writing a journal entry) from the finish sheet
                    // was silently reverted by the finish itself.
                    sessionDao.setFinishTotals(
                        id = sessionId,
                        totalVolumeLb = VolumeCalculator.sessionVolumeLb(sets),
                        prCount = prCount,
                        setCount = sets.size,
                        activeSeconds = activeSeconds
                    )
                    FinishOutcome.Won(session, activeSeconds)
                }
            }
        }

        if (outcome !is FinishOutcome.Won) {
            return if (outcome is FinishOutcome.AlreadyFinished) outcome.activeSeconds else 0
        }

        // Everything past the commit is a side effect on state this transaction does not own:
        // DataStore (the rotation counter), Health Connect, and the widget. None of it belongs
        // inside a Room transaction — `maybeRotateProgram` writes DataStore and can regenerate a
        // whole program, which would hold the database lock for the duration.
        //
        // Each is attempted independently. They used to run in sequence with no guard, so a throw
        // in the rotation (a DataStore read failure, a generation error) skipped the Health Connect
        // mirrors AND the widget refresh, and the finish itself then reported a failure to a caller
        // whose local data was already correctly committed.
        finishSideEffect { maybeRotateProgram() }
        finishSideEffect { writeFinishMirrors(outcome.session, endMs = now, activeSeconds = outcome.activeSeconds) }
        finishSideEffect { refreshWidget() }
        return outcome.activeSeconds
    }

    /** What the finish transaction decided, so the side effects below it know whether to run. */
    private sealed interface FinishOutcome {
        /** This call stamped the session; it owns the follow-up work. */
        data class Won(val session: Session, val activeSeconds: Int) : FinishOutcome
        /** Someone else already finished it — report their duration and do nothing. */
        data class AlreadyFinished(val activeSeconds: Int) : FinishOutcome
        /** The row disappeared mid-finish. */
        object Gone : FinishOutcome
    }

    /**
     * Run one post-commit side effect without letting it take the others down.
     *
     * Cancellation is rethrown: swallowing it would let a cancelled scope keep working, and the
     * caller is entitled to stop us.
     */
    private suspend inline fun finishSideEffect(block: () -> Unit) {
        try {
            block()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (_: Exception) {
            // Local state is already committed and correct; a failed mirror or widget refresh is
            // not a reason to report the finish as failed.
        }
    }

    /**
     * Every Health Connect mirror a finished gym session gets — ONE helper so the normal finish and
     * the orphan-recovery finish can't drift (a recovered session is a real session; it mirrors too).
     * Each write is independently gated on its own opt-in pref + granted permission and fail-soft,
     * so a miss can never affect the local finish.
     */
    private suspend fun writeFinishMirrors(session: Session, endMs: Long, activeSeconds: Int) {
        maybeWriteActiveCalories(session, finishedAtMs = endMs, activeSeconds = activeSeconds)
        maybeWriteSessionRecord(session, endMs = endMs)
        maybeWriteHrSeries(session, endMs = endMs)
    }

    /**
     * HC-4: mirror this finished session's estimated active calories to Health Connect — gated on the
     * opt-in pref AND the granted write permission (the cheap pref is checked first, so a disconnected
     * user never touches HC), and fail-soft so a miss can't affect the finish. Skips silently when
     * there's no logged bodyweight (the MET estimate needs it) or the session had no active time.
     */
    private suspend fun maybeWriteActiveCalories(session: Session, finishedAtMs: Long, activeSeconds: Int) {
        if (!settingsRepo.hcWriteCalories.first()) return
        if (!health.canWriteActiveCalories()) return
        // The watch's MEASURED burn (streamed with the HR batches, W3) beats the MET estimate —
        // "cardio kcal estimates return only with real watch burn data" is the settled rule.
        val watchKcal = wearHrIngest.watchKcal(session.id)
        val kcal = watchKcal ?: run {
            val weightLb = bodyweightDao.latest()?.weightLb ?: return
            // Pass fractional minutes (/ 60.0) so a sub-minute remainder isn't truncated away — Int
            // division dropped up to ~1 min per session and skipped sessions under a full minute entirely.
            ActiveCalorieEstimator.estimate(activeSeconds / 60.0, weightLb, session.intensity) ?: return
        }
        health.writeActiveCalories(
            kcal = kcal,
            startMs = session.startedAt,
            endMs = finishedAtMs,
            // Same key shape as the session and HR mirrors above, so a re-finish updates the
            // record rather than adding another.
            clientRecordId = "avex-session-kcal-${session.id}",
            clientRecordVersion = finishedAtMs
        )
    }

    /**
     * W3: attach the watch's HR trace to the finished session in Health Connect — same opt-in as
     * the session write (one "workout sessions" concept), fail-soft, upsert-keyed per session.
     */
    private suspend fun maybeWriteHrSeries(session: Session, endMs: Long) {
        if (!settingsRepo.hcWriteSessions.first()) return
        val samples = sessionHrSampleDao.forSession(session.id)
        if (samples.isEmpty()) return
        health.writeHrSeries(
            clientRecordId = "avex-session-hr-${session.id}",
            clientRecordVersion = endMs,
            startMs = session.startedAt,
            endMs = endMs,
            samples = samples.map { com.forge.app.domain.health.HrPoint(timeMs = it.atMs, bpm = it.bpm) }
        )
    }

    /** The watch's HR trace for one session (W3) — session detail's graph + HRR read. */
    suspend fun hrSamplesForSession(sessionId: Long): List<com.forge.app.data.db.entities.SessionHrSample> =
        sessionHrSampleDao.forSession(sessionId)

    /**
     * W0: mirror this finished gym session to Health Connect as a strength-training
     * [androidx.health.connect.client.records.ExerciseSessionRecord], titled with its day name —
     * this is what makes it appear in Samsung Health / Google Fit. Keyed on a stable
     * clientRecordId per session so a re-finish (orphan recovery after a crash) UPDATES the HC
     * record instead of duplicating it; the version stamp is the finish time, which only grows.
     */
    private suspend fun maybeWriteSessionRecord(session: Session, endMs: Long) {
        if (!settingsRepo.hcWriteSessions.first()) return
        if (!health.canWriteExerciseSessions()) return
        health.writeExerciseSession(
            clientRecordId = "avex-session-${session.id}",
            clientRecordVersion = endMs,
            exerciseType = com.forge.app.data.health.HcExerciseTypes.STRENGTH,
            title = Program.dayDisplayName(session.dayKey),
            startMs = session.startedAt,
            endMs = endMs
        )
    }

    /** Lifetime count of PR sets across all logged exercises — feeds the PR-milestone notification. */
    suspend fun lifetimePrCount(): Int = loggedExerciseDao.prCount()

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
        // A freestyle user has no plan to rotate — and rerollAll() would silently generate one that
        // stays hidden behind the freestyle home (the manual Settings paths flip freestyle off, but
        // this background path can't ask). Skip entirely.
        if (settingsRepo.freestyleMode.first()) return

        val deloadStart = settingsRepo.deloadWeekStartMs.first()
        val deloadRange = if (deloadStart > 0) deloadWindow(deloadStart) else null
        // A deload WEEK has to end. applyDeloadWeek regenerates at reduced volume and stamps the
        // start, but nothing ever restored full volume: the only exits were a manual Settings
        // regenerate or auto-rotation, and rotationCadence defaults to "never". So for a default
        // user the recovery week quietly became their permanent program — and once the marker
        // aged past the window, nothing on screen said they were deloading either.
        //
        // This sits ABOVE the rotation gate deliberately: that gate is precisely the one most
        // users never pass. A clock that moved backwards lands before the window and matches
        // neither branch, leaving the program alone.
        if (deloadRange != null && clock.nowMs() > deloadRange.last) {
            programRepository.restoreAfterDeload()
            return
        }
        if (settingsRepo.rotationCadence.first() != "every_n") return
        // Pause auto-rotation inside the deload week — a rotation regenerates a full-volume program
        // and would silently wipe the recovery week mid-deload (seam fix #18). The counter is left
        // untouched, so rotation resumes on the next finish after the deload ends.
        if (deloadRange != null && clock.nowMs() in deloadRange) return
        // Read-increment-compare-write in ONE DataStore edit: two finishes racing used to lose an
        // increment, or both trip the limit and start two full program generations at once.
        val n = settingsRepo.rotationEveryN.first()
        if (!settingsRepo.countSessionTowardRotation(n)) return
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
        refreshWidget()
    }

    /** What [resolveOrphanSession] did, so the UI can surface it once. */
    data class OrphanResolution(val finishedToHistory: Boolean)

    /**
     * A "zombie" active session is one in progress on a day key the current program no longer has — a
     * force-stop, or a program regenerate that bypassed the workout-discard guard, can leave one behind.
     * It would otherwise drive a misleading resume banner or silently block a fresh start. Resolve it
     * NON-DESTRUCTIVELY: an empty session is discarded (nothing to lose); one with real logged sets is
     * finished to history (the work is preserved and still attributes to its exercises' all-time PRs).
     * No rotation side-effects (unlike [finishSession]). Returns what happened, or null if no orphan.
     */
    suspend fun resolveOrphanSession(validDayKeys: Set<String>): OrphanResolution? {
        // Empty = the program isn't loaded yet; never treat a live session as orphan on no information.
        if (validDayKeys.isEmpty()) return null
        val active = sessionDao.getActiveSession() ?: return null
        if (active.dayKey in validDayKeys) return null
        val sets = loggedSetDao.allForSession(active.id)
        if (sets.isEmpty()) {
            discardSession(active.id)
            return OrphanResolution(finishedToHistory = false)
        }
        val now = clock.nowMs()
        // This session was abandoned earlier (force-stop / regenerate), not finished just now: close any
        // open sitting at the LAST logged-set activity rather than `now` (which could be days later), then
        // stamp the real active time exactly as [finishSession] does — otherwise activeSeconds stays 0 and
        // [Session.durationMinutes] falls back to wall-clock, showing a multi-day "workout" in history.
        closeDanglingSegments(active.id)
        val activeSeconds = (closedSegmentMs(active.id) / 1000L).toInt().coerceAtLeast(0)
        // Mirror finishSession's denormalised stamps so history/recap/trophies read this session correctly:
        // volume through the shared calculator (no divergence if its formula changes), and prCount from the
        // per-exercise PR flags (the sets also count toward all-time PRs via the frontier queries).
        val prCount = loggedExerciseDao.forSession(active.id).count { it.wasPr }
        sessionDao.update(
            active.copy(
                finishedAt = now,
                totalVolumeLb = VolumeCalculator.sessionVolumeLb(sets),
                setCount = sets.size,
                prCount = prCount,
                activeSeconds = activeSeconds
            )
        )
        // Mirror to Health Connect like a normal finish (a recovered session is a real session) — but
        // end the HC record at the LAST logged activity, not `now`: resolution can run days after the
        // session was abandoned, and `now` would write a multi-day workout into Samsung Health.
        val hcEndMs = sets.maxOfOrNull { it.completedAt } ?: (active.startedAt + activeSeconds * 1000L)
        writeFinishMirrors(active, endMs = hcEndMs, activeSeconds = activeSeconds)
        refreshWidget()
        return OrphanResolution(finishedToHistory = true)
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

    /** This session's existing row for a program slot, or null. See [LoggedExerciseDao.forSessionSlot]. */
    suspend fun loggedExerciseForSlot(sessionId: Long, slotId: String): Long? =
        loggedExerciseDao.forSessionSlot(sessionId, slotId)?.id

    /**
     * The row for one program slot in one session, creating it if it does not exist yet — as ONE
     * transaction, so two writers cannot both find nothing and both insert.
     *
     * Every caller previously did the read and the insert as separate steps: the day screen's
     * `ensureLoggedExercise`, the wrist's `SetLogUseCase.logFromWatch`, the swap paths. The wrist
     * writes through this same process, so "the phone logs a set while a wrist command is in
     * flight" is two coroutines in one Room instance, and the gap between the read and the insert
     * is where the slot forks: two logged_exercise rows for one slot, each holding half the sets.
     * The day screen renders the first, so the other half of the workout is simply not there — and
     * it is not lost either, which is worse, because it still counts toward volume and progression.
     *
     * Room serialises transactions on its single writer connection, so the read and the insert are
     * indivisible with respect to every other writer in the process.
     */
    suspend fun ensureLoggedExerciseForSlot(
        sessionId: Long,
        slotId: String,
        exerciseId: String,
        orderIndex: Int,
        swappedName: String? = null,
        swappedUnit: String? = null
    ): Long = com.forge.app.data.db.SessionWrites.ensureSlotRow(
        db = database,
        sessionId = sessionId,
        slotId = slotId,
        exerciseId = exerciseId,
        orderIndex = orderIndex,
        swappedName = swappedName,
        swappedUnit = swappedUnit
    )

    suspend fun updateExercise(loggedExercise: LoggedExercise) =
        loggedExerciseDao.update(loggedExercise)

    // Targeted single-column writes rather than read-modify-write of the whole row: see the DAO's
    // note — a note commit racing a SKIP tap used to silently un-skip the exercise.
    suspend fun setRating(loggedExerciseId: Long, rating: EffortRating) =
        loggedExerciseDao.setDifficulty(loggedExerciseId, rating)

    suspend fun setSkipped(loggedExerciseId: Long, skipped: Boolean) =
        loggedExerciseDao.setSkipped(loggedExerciseId, skipped)

    suspend fun setNote(loggedExerciseId: Long, note: String?) =
        loggedExerciseDao.setNote(loggedExerciseId, note)

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
        // The same transaction the forward path (setSessionSwap) already uses, for the same reason
        // and in the opposite direction. Read, count, write was three steps: a set inserted by the
        // wrist after the count came back zero was silently re-attributed when the stale row was
        // written back under the slot's exercise id — the set stayed in the database, under a lift
        // the user never performed, and the one they did perform lost it.
        database.withTransaction {
            val ex = loggedExerciseDao.get(loggedExerciseId) ?: return@withTransaction
            if (loggedSetDao.countForLoggedExercise(loggedExerciseId) > 0) return@withTransaction
            loggedExerciseDao.update(
                ex.copy(exerciseId = ex.effectiveSlotId, slotId = null, swappedName = null, swappedUnit = null)
            )
        }
    }

    suspend fun lastLoggedExerciseBefore(exerciseId: String, excludeSessionId: Long): LoggedExercise? =
        loggedExerciseDao.lastLoggedBefore(exerciseId, excludeSessionId)

    // ─── Sets ──────────────────────────────────────────────────────────────────

    suspend fun setsFor(loggedExerciseId: Long): List<LoggedSet> =
        loggedSetDao.forLoggedExercise(loggedExerciseId)

    /**
     * Insert one performed set.
     *
     * The set's index is resolved HERE, from the highest index already stored, rather than taken
     * from the caller's live row count. Every caller passed a count, and a count is wrong the moment
     * a set is deleted from the middle: indices 0 and 2 with a count of 2 wrote a second index 2,
     * and the wrist's prefill (`maxByOrNull { setIndex }`) then picked arbitrarily between the two,
     * so the target weight it showed flipped between different sets. One funnel, one rule.
     *
     * The allocation and the insert are ONE transaction, because a funnel is not the same as an
     * atom: `MAX(set_index) + 1` followed by a separate insert has a gap in it, and two writers
     * reaching that gap together both read the same maximum and both write it. This app has more
     * than one writer whenever a watch is paired — the wrist's commands are handled by
     * WearSyncService inside this same process, against this same Room instance — and one lifter
     * double-tapping LOG SET is enough on its own. Room serialises transactions on its single
     * writer connection, so the read and the insert are indivisible with respect to both.
     */
    suspend fun logSet(
        loggedExerciseId: Long,
        weightText: String,
        weightLb: Double?,
        reps: Int,
        durationSeconds: Int? = null
    ): Long = com.forge.app.data.db.SessionWrites.insertSetWithNextIndex(
        database,
        run {
            val safeLb = sanitizeWeightLb(weightLb)
            LoggedSet(
                loggedExerciseId = loggedExerciseId,
                // Replaced inside the transaction; see SessionWrites.insertSetWithNextIndex.
                setIndex = 0,
                // A clamped weight takes the text with it. Leaving the typed "1000000000" beside a
                // stored 2000 would show the user a number the database does not hold, and every
                // later edit would re-parse the text back to the absurd value.
                weightText = if (safeLb != null && weightLb != null && safeLb != weightLb)
                    com.forge.app.domain.units.weightInputValue(safeLb, useKg = false) else weightText,
                weightLb = safeLb,
                reps = sanitizeReps(reps),
                completedAt = clock.nowMs(),
                // Timed holds (GYMAP-51): a held duration in whole seconds. null for a normal rep
                // set; when present, `reps` is not a meaningful count and the set is skipped by
                // weight×reps stats.
                durationSeconds = durationSeconds?.coerceIn(0, MAX_HOLD_SECONDS)
            )
        }
    )

    suspend fun deleteSet(set: LoggedSet) = loggedSetDao.delete(set)

    suspend fun updateSet(set: LoggedSet) =
        loggedSetDao.update(set.copy(reps = sanitizeReps(set.reps), weightLb = sanitizeWeightLb(set.weightLb)))

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

    /** Longest hold ever for a timed-hold exercise (seconds), or null if none — the "best 0:45" hint (GYMAP-51). */
    suspend fun bestHoldSecondsForExercise(exerciseId: String): Int? =
        loggedSetDao.bestHoldSecondsForExercise(exerciseId)

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

    /** One session's completed rests, oldest first — the HRR read's windows (W3). */
    suspend fun restEventsForSession(sessionId: Long): List<RestEvent> = restEventDao.forSession(sessionId)

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
