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
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/** Handles the destructive reset operations in the Settings screen (#119). */
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
     * One transaction so a reset is all-or-nothing rather than half a wipe.
     */
    suspend fun resetSessions() = db.withTransaction {
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

    suspend fun resetTrophies() = trophyDao.deleteAll()
    suspend fun resetCardio() = cardioDao.deleteAll()

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
        settingsRepo.resetAll()
    }
}
