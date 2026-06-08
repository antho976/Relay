package com.forge.app.data.repo

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.SessionDao
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
    private val settingsRepo: SettingsRepository,
    private val db: ForgeDatabase
) {
    suspend fun resetSessions() = sessionDao.deleteAll()
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
        settingsRepo.resetAll()
    }
}
