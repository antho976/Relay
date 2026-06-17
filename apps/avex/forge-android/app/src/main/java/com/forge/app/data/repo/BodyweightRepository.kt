package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.health.BodyweightSync
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class BodyweightRepository @Inject constructor(
    private val dao: BodyweightDao,
    private val clock: Clock,
    private val health: HealthConnectManager,
    private val settings: SettingsRepository
) {
    fun observeRecent(limit: Int = 90): Flow<List<BodyweightEntry>> = dao.observeRecent(limit)

    suspend fun latestWeightLb(): Double? = dao.latest()?.weightLb

    suspend fun log(weightLb: Double) {
        val now = clock.nowMs()
        dao.upsert(BodyweightEntry(dateKey = LocalDate.now().toString(), weightLb = weightLb, recordedAt = now))
        // Mirror the weigh-in to Health Connect when the user has opted in AND granted write access.
        // Gated on both so onboarding (neither set) never writes, and a mirror failure can't break
        // the local save above — the DB stays the single source of truth.
        if (settings.hcWriteBodyweight.first() && health.canWriteWeight()) {
            health.writeWeight(weightLb, now)
        }
    }

    suspend fun delete(id: Long) = dao.delete(id)

    /** Whether an "Import from Health Connect" affordance should be offered (read permission granted). */
    suspend fun canImportFromHealthConnect(): Boolean = health.canReadWeight()

    /**
     * Pull the latest bodyweight from Health Connect and record it locally — but only when it's
     * newer than our newest entry (see [BodyweightSync.shouldImport]), so a typed weigh-in is never
     * overwritten and re-importing is idempotent. Returns the imported value, or null when there
     * was nothing newer to import (or HC is unavailable / not granted).
     */
    suspend fun importLatestFromHealthConnect(): Double? {
        val hc = health.latestWeight(clock.nowMs()) ?: return null
        val localLatestMs = dao.latest()?.recordedAt
        if (!BodyweightSync.shouldImport(hc.timeMs, hc.weightLb, localLatestMs)) return null
        val dateKey = Instant.ofEpochMilli(hc.timeMs).atZone(ZoneId.systemDefault()).toLocalDate().toString()
        dao.upsert(BodyweightEntry(dateKey = dateKey, weightLb = hc.weightLb, recordedAt = hc.timeMs))
        return hc.weightLb
    }
}
