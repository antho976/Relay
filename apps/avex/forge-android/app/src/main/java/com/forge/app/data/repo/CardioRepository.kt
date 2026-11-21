package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.health.HcExerciseTypes
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.health.HcRecordKeys
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class CardioRepository @Inject constructor(
    private val cardioDao: CardioDao,
    private val health: HealthConnectManager,
    private val settingsRepo: SettingsRepository,
    private val clock: Clock
) {
    /** Newest real cardio sessions; logged rest days are excluded before the limit applies. */
    fun observeRecent(limit: Int = 20): Flow<List<CardioEntry>> = cardioDao.observeRecent(limit)

    /** Full cardio history, newest-first (the log list no longer caps at 20). */
    fun observeAll(): Flow<List<CardioEntry>> = cardioDao.observeAll()

    /** Non-rest cardio logged within [fromMs, toMs) — a bounded query for a single day's detail, so
     *  callers don't load the full history just to filter one day out of it. */
    suspend fun entriesInRange(fromMs: Long, toMs: Long): List<CardioEntry> = cardioDao.between(fromMs, toMs)

    /** Total cardio minutes since [sinceEpochMs], excluding REST entries. */
    fun observeMinutesSince(sinceEpochMs: Long): Flow<Int?> = cardioDao.observeMinutesSince(sinceEpochMs)

    suspend fun add(entry: CardioEntry): Long {
        val id = cardioDao.insert(entry)
        mirrorToHealthConnect(entry.copy(id = if (entry.id != 0L) entry.id else id))
        return id
    }

    suspend fun update(entry: CardioEntry) {
        cardioDao.update(entry)
        mirrorToHealthConnect(entry)
    }

    suspend fun delete(entry: CardioEntry) {
        cardioDao.delete(entry)
        // Take the HC mirror with it (fail-soft; a never-mirrored id is a no-op).
        health.deleteExerciseSession(clientRecordId(entry.id))
    }

    suspend fun get(id: Long): CardioEntry? = cardioDao.get(id)

    /**
     * W0: mirror this cardio entry to Health Connect as an exercise session with its REAL type, so
     * a logged run appears as a run in Samsung Health / Google Fit. Gated on the session write
     * opt-in + granted permission, fail-soft, and keyed on a stable clientRecordId per entry so an
     * EDIT updates the HC record (version = write time, which only grows) and a delete removes it.
     * Rest days are not workouts: an entry edited INTO rest deletes any mirror it had.
     */
    private suspend fun mirrorToHealthConnect(entry: CardioEntry) {
        if (!settingsRepo.hcWriteSessions.first()) return
        if (!health.canWriteExerciseSessions()) return
        val type = CardioType.entries.firstOrNull { it.code == entry.type }
        if (type == CardioType.REST) {
            health.deleteExerciseSession(clientRecordId(entry.id))
            return
        }
        val startMs = entry.date
        val endMs = entry.date + entry.durationMin.coerceAtLeast(0) * 60_000L
        // A run imported from the watch ("recorded with your watch, import?") already lives in
        // Health Connect under its own provider; writing an Avex-origin twin made Samsung Health /
        // Google Fit list it twice. A mirror this entry wrote BEFORE the watch session synced is
        // dropped too (no-op when never mirrored), else HC keeps a stale Avex twin.
        if (health.hasOverlappingWatchSession(startMs, endMs)) {
            health.deleteExerciseSession(clientRecordId(entry.id))
            return
        }
        health.writeExerciseSession(
            clientRecordId = clientRecordId(entry.id),
            clientRecordVersion = clock.nowMs(),
            exerciseType = HcExerciseTypes.forCardioCode(entry.type),
            // Built-in activities carry their label; a custom activity's name lives in DataStore
            // (not resolvable here), so it mirrors under the honest generic.
            title = type?.displayName ?: "Cardio",
            startMs = startMs,
            endMs = endMs
        )
    }

    // One scheme for writers and deleters (M-02): the reset path derives the same key from the ids
    // it captures before the wipe, so it can address exactly what this wrote.
    private fun clientRecordId(entryId: Long) = HcRecordKeys.cardio(entryId)
}
