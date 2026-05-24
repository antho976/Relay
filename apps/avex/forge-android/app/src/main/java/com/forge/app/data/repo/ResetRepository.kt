package com.forge.app.data.repo

import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.prefs.SettingsRepository
import javax.inject.Inject
import javax.inject.Singleton

/** Handles the destructive reset operations in the Settings screen (#119). */
@Singleton
class ResetRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val trophyDao: UnlockedTrophyDao,
    private val cardioDao: CardioDao,
    private val settingsRepo: SettingsRepository
) {
    suspend fun resetSessions() = sessionDao.deleteAll()
    suspend fun resetTrophies() = trophyDao.deleteAll()
    suspend fun resetCardio() = cardioDao.deleteAll()
    suspend fun resetAppSettings() = settingsRepo.resetAll()

    suspend fun factoryReset() {
        sessionDao.deleteAll()
        trophyDao.deleteAll()
        cardioDao.deleteAll()
        settingsRepo.resetAll()
    }
}
