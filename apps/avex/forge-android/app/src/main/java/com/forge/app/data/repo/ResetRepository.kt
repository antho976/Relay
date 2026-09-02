package com.forge.app.data.repo

import androidx.room.withTransaction
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.AdviceEventDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.CoachDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.RestEventDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.SuggestionOutcomeDao
import com.forge.app.data.db.dao.TrophyNearMissDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.health.HcRecordKeys
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles the destructive reset operations in the Settings screen (#119).
 *
 * Every reset that removes rows Avex has mirrored to Health Connect takes the mirrors with them
 * (M-02): "Deletes ALL data" used to clear the local tables while Samsung Health / Fit kept every
 * Avex-written session, calorie, heart-rate, weight and body-fat record. The mirrors are keyed on
 * local row ids ([HcRecordKeys]), so each reset captures the keys BEFORE its wipe; the local
 * delete then runs first and never waits on the provider, and the external cleanup that follows
 * is best-effort and fail-soft (no provider, no grant or a provider error cannot fail the reset).
 */
@Singleton
class ResetRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val trophyDao: UnlockedTrophyDao,
    private val cardioDao: CardioDao,
    private val moodDao: MoodDao,
    private val suggestionOutcomeDao: SuggestionOutcomeDao,
    private val adviceEventDao: AdviceEventDao,
    private val restEventDao: RestEventDao,
    private val restDayDao: RestDayDao,
    private val coachDao: CoachDao,
    private val nearMissDao: TrophyNearMissDao,
    private val settingsRepo: SettingsRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository,
    private val health: HealthConnectManager,
    private val clock: com.forge.app.core.time.Clock,
    private val db: ForgeDatabase
) {
    /**
     * "Deletes all sessions, sets, and exercises logged. Cannot be undone." — and the tables that
     * were *derived from* those sessions have to go with them.
     *
     * `DELETE FROM session` only reaches what declares a foreign key on `session.id`. Everything the
     * engine learned from that history — `coach_decision` / `coach_pass` (which bias EVERY future
     * program generation through `CoachGenBias`), `suggestion_outcome` (the weight-chip step
     * calibrator), `advice_event`, `trophy_near_miss`, `rest_day_entry` — has no FK and used to
     * survive. `mood_entry` survived too, with `session_id` SET NULL, and kept feeding readiness.
     * The user asked for a clean slate and got a "fresh" program silently shaped by the data they
     * had just erased.
     *
     * One transaction so a reset is all-or-nothing rather than half a wipe. The session ids are
     * read first: they are the only handle on the Health Connect mirrors, and the wipe destroys them.
     */
    suspend fun resetSessions() {
        val sessionIds = sessionDao.allIds()
        db.withTransaction {
            sessionDao.deleteAll()
            moodDao.deleteAll()
            suggestionOutcomeDao.deleteAll()
            adviceEventDao.deleteAll()
            restEventDao.deleteAll()
            restDayDao.deleteAll()
            coachDao.deleteAllDecisions()
            coachDao.deleteAllPasses()
            nearMissDao.deleteAll()
        }
        health.deleteSessionMirrors(sessionIds)
    }

    suspend fun resetTrophies() = trophyDao.deleteAll()

    /** Cardio entries plus the exercise sessions Avex mirrored for them (ids captured before the wipe). */
    suspend fun resetCardio() {
        val entryIds = cardioDao.allIds()
        cardioDao.deleteAll()
        health.deleteExerciseSessions(entryIds.map(HcRecordKeys::cardio))
    }

    /** Restore default preferences but keep onboarding/identity so the user isn't re-onboarded. */
    suspend fun resetAppSettings() = settingsRepo.resetSettingsOnly()

    /**
     * True clean slate: wipe EVERY database table (via [ForgeDatabase.clearAllTables], so no table
     * is ever forgotten as the schema grows) plus all preferences. Clearing the onboarding flag
     * routes the user back through onboarding, which regenerates the program.
     */
    suspend fun factoryReset() {
        withContext(Dispatchers.IO) { db.clearAllTables() }
        photoRepo.deleteAll() // progress photos live as files outside the DB — clear them too (#138).
        avatarRepo.clear()    // the avatar is an app-private file too.
        // Every record Avex ever wrote to Health Connect, of every type, by time range rather than
        // by key: the tables are already gone, and a range delete is scoped by the provider to
        // this app's own records, so nothing of another app's is touched. Before the preference
        // wipe so the reset's external half runs while the grants and opt-ins still describe it.
        health.deleteAllAvexRecords(clock.nowMs())
        settingsRepo.resetAll()
    }
}
