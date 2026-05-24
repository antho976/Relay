package com.forge.app.data.repo

import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.entities.RestDayEntry
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class RestDayRepository @Inject constructor(private val dao: RestDayDao) {
    suspend fun markRestDay(dateKey: String, type: String) =
        dao.upsert(RestDayEntry(dateKey = dateKey, type = type))
    suspend fun clearRestDay(dateKey: String) = dao.delete(dateKey)
    suspend fun get(dateKey: String): RestDayEntry? = dao.get(dateKey)
}
