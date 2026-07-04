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

    /** Full cardio history, newest-first (the log list no longer caps at 20). */
    fun observeAll(): Flow<List<CardioEntry>> = cardioDao.observeAll()

    fun observeSince(sinceEpochMs: Long): Flow<List<CardioEntry>> = cardioDao.observeSince(sinceEpochMs)

    /** Non-rest cardio logged within [fromMs, toMs) — a bounded query for a single day's detail, so
     *  callers don't load the full history just to filter one day out of it. */
    suspend fun entriesInRange(fromMs: Long, toMs: Long): List<CardioEntry> = cardioDao.between(fromMs, toMs)

    /** Total cardio minutes since [sinceEpochMs], excluding REST entries. */
    fun observeMinutesSince(sinceEpochMs: Long): Flow<Int?> = cardioDao.observeMinutesSince(sinceEpochMs)

    /** Total cardio distance (km) since [sinceEpochMs], excluding REST entries. */
    fun observeDistanceKmSince(sinceEpochMs: Long): Flow<Double?> = cardioDao.observeDistanceKmSince(sinceEpochMs)

    suspend fun add(entry: CardioEntry): Long = cardioDao.insert(entry)

    suspend fun update(entry: CardioEntry) = cardioDao.update(entry)

    suspend fun delete(entry: CardioEntry) = cardioDao.delete(entry)

    suspend fun get(id: Long): CardioEntry? = cardioDao.get(id)
}
