package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LeanMassDao
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.domain.health.LeanMassSync
import kotlinx.coroutines.flow.Flow
import java.time.Instant
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Lean-body-mass log (W6) — skeletal-muscle trend from the watch's BIA measurements, the sibling
 * of [BodyFatRepository] minus everything a watch-authored metric doesn't need: no manual log, no
 * write-back. The DB is the local source of truth; Health Connect is the only inlet.
 */
@Singleton
class LeanMassRepository @Inject constructor(
    private val dao: LeanMassDao,
    private val clock: Clock,
    private val health: HealthConnectManager
) {
    fun observeRecent(limit: Int = 90): Flow<List<LeanMassEntry>> = dao.observeRecent(limit)

    suspend fun latestLb(): Double? = dao.latest()?.weightLb

    /** Whether an "Import from Health Connect" affordance should be offered (read granted). */
    suspend fun canImportFromHealthConnect(): Boolean = health.canReadLeanMass()

    /**
     * Pull the latest lean-mass reading from Health Connect and record it locally — only when it's
     * newer than our newest entry ([LeanMassSync.shouldImport]), so re-importing is idempotent.
     * Returns the imported value in lb, or null when there was nothing newer.
     */
    suspend fun importLatestFromHealthConnect(): Double? {
        val hc = health.latestLeanMass(clock.nowMs()) ?: return null
        val localLatestMs = dao.latest()?.recordedAt
        if (!LeanMassSync.shouldImport(hc.timeMs, hc.weightLb, localLatestMs)) return null
        val dateKey = Instant.ofEpochMilli(hc.timeMs).atZone(ZoneId.systemDefault()).toLocalDate().toString()
        dao.upsert(LeanMassEntry(dateKey = dateKey, weightLb = hc.weightLb, recordedAt = hc.timeMs))
        return hc.weightLb
    }
}
