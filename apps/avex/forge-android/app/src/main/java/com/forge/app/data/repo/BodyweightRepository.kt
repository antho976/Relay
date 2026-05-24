package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.entities.BodyweightEntry
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class BodyweightRepository @Inject constructor(
    private val dao: BodyweightDao,
    private val clock: Clock
) {
    fun observeRecent(limit: Int = 90): Flow<List<BodyweightEntry>> = dao.observeRecent(limit)

    suspend fun latestWeightLb(): Double? = dao.latest()?.weightLb

    suspend fun log(weightLb: Double) {
        val dateKey = LocalDate.now().toString()
        dao.upsert(BodyweightEntry(dateKey = dateKey, weightLb = weightLb, recordedAt = clock.nowMs()))
    }

    suspend fun delete(id: Long) = dao.delete(id)
}
