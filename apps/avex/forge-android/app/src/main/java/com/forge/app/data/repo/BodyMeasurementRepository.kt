package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyMeasurementDao
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.domain.measurement.BodyMeasurementType
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Body-measurement history (GYMAP-52). Local-only, mirroring [BodyweightRepository] but scoped per
 * measurement type. Values are stored canonically in cm; unit conversion happens at the display edge
 * ([com.forge.app.domain.units.LengthFormatter]).
 */
@Singleton
class BodyMeasurementRepository @Inject constructor(
    private val dao: BodyMeasurementDao,
    private val clock: Clock
) {
    /** All readings, newest first — callers group by [BodyMeasurementEntry.type]. */
    fun observeAll(): Flow<List<BodyMeasurementEntry>> = dao.observeAll()

    /** Record today's value for one measurement type (replaces an existing same-day entry). */
    suspend fun log(type: BodyMeasurementType, valueCm: Double) {
        dao.upsert(
            BodyMeasurementEntry(
                type = type.key,
                dateKey = LocalDate.now().toString(),
                valueCm = valueCm,
                recordedAt = clock.nowMs()
            )
        )
    }

    suspend fun latest(type: BodyMeasurementType): BodyMeasurementEntry? = dao.latest(type.key)

    suspend fun delete(id: Long) = dao.delete(id)
}
