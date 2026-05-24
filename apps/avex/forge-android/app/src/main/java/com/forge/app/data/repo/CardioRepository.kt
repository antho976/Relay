package com.forge.app.data.repo

import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.entities.CardioEntry
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class CardioRepository @Inject constructor(
    private val cardioDao: CardioDao
) {
    fun observeRecent(limit: Int = 20): Flow<List<CardioEntry>> = cardioDao.observeRecent(limit)

    fun observeSince(sinceEpochMs: Long): Flow<List<CardioEntry>> = cardioDao.observeSince(sinceEpochMs)

    /** Total cardio minutes since [sinceEpochMs], excluding REST entries. */
    fun observeMinutesSince(sinceEpochMs: Long): Flow<Int?> = cardioDao.observeMinutesSince(sinceEpochMs)

    /** Total cardio distance (km) since [sinceEpochMs], excluding REST entries. */
    fun observeDistanceKmSince(sinceEpochMs: Long): Flow<Double?> = cardioDao.observeDistanceKmSince(sinceEpochMs)

    /** Cumulative km across all cardio entries (#79). */
    fun observeLifetimeDistanceKm(): Flow<Double?> = cardioDao.observeLifetimeDistanceKm()

    /** All run entries with non-zero distance — for pace trend (#78). */
    fun observeRunEntries(): Flow<List<CardioEntry>> = cardioDao.observeRunEntries()

    suspend fun add(entry: CardioEntry): Long = cardioDao.insert(entry)

    suspend fun update(entry: CardioEntry) = cardioDao.update(entry)

    suspend fun delete(entry: CardioEntry) = cardioDao.delete(entry)

    suspend fun get(id: Long): CardioEntry? = cardioDao.get(id)
}
