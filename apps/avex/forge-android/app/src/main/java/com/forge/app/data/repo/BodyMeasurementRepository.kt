package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyMeasurementDao
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.domain.measurement.BodyMeasurementType
import kotlinx.coroutines.flow.Flow
import java.time.Instant
import java.time.ZoneId
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

    /**
     * Record today's value for one measurement type (replaces an existing same-day entry).
     *
     * Both stamps come from ONE reading of the injected clock. They used to come from two different
     * sources — `LocalDate.now()` and `clock.nowMs()` — which is microseconds apart in production
     * but arbitrarily far apart under a FakeClock or any future backfill path. The entity's contract
     * ("one entry per type per day, upserted by date") is enforced on `date_key` while every chart
     * sorts by `recorded_at`, so the two disagreeing is a row filed under a day it didn't happen on.
     */
    suspend fun log(type: BodyMeasurementType, valueCm: Double, zone: ZoneId = ZoneId.systemDefault()) {
        val now = clock.nowMs()
        dao.upsert(
            BodyMeasurementEntry(
                type = type.key,
                dateKey = Instant.ofEpochMilli(now).atZone(zone).toLocalDate().toString(),
                valueCm = valueCm,
                recordedAt = now
            )
        )
    }

    suspend fun latest(type: BodyMeasurementType): BodyMeasurementEntry? = dao.latest(type.key)

    suspend fun delete(id: Long) = dao.delete(id)
}
