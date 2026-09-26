package com.forge.app.data.repo

import com.forge.app.data.db.dao.WarmupRoutineDao
import com.forge.app.data.db.entities.WarmupRoutineItem
import com.forge.app.program.Program
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages user-defined warmup routines per day (#144).
 * Falls back to the static [Program] warmup if no custom items exist.
 */
@Singleton
class WarmupRepository @Inject constructor(private val dao: WarmupRoutineDao) {

    fun observeForDay(dayKey: String): Flow<List<WarmupRoutineItem>> = dao.observeForDay(dayKey)

    /** Returns custom warmup items for the day, or null if none defined (fall back to static plan). */
    suspend fun customWarmupForDay(dayKey: String): List<String>? {
        val items = dao.forDay(dayKey)
        return if (items.isEmpty()) null
        else items.map { it.label }
    }
}
