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
import java.time.LocalTime
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

    /**
     * Record a weigh-in for [date] (defaults to today) with an optional [note] (GYMAP-54). One entry
     * per day: the `date_key` upsert replaces that day's row, so re-saving edits it and backdating
     * fills a past day. `recorded_at` is stamped on the chosen day (now for today, else that day at
     * the current time-of-day) so a backdated entry sorts onto the right day in the trend rather than
     * jumping to "now".
     */
    suspend fun log(weightLb: Double, date: LocalDate = LocalDate.now(), note: String? = null) {
        val now = clock.nowMs()
        val today = LocalDate.now()
        val recordedAt =
            if (date == today) now
            else date.atTime(LocalTime.now()).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
        dao.upsert(
            BodyweightEntry(
                dateKey = date.toString(),
                weightLb = weightLb,
                recordedAt = recordedAt,
                note = note?.trim()?.takeIf { it.isNotBlank() }
            )
        )
        // Mirror ONLY a same-day weigh-in to Health Connect, and only when the user has opted in AND
        // granted write access. Gated on all three so onboarding (neither set) never writes, a mirror
        // failure can't break the local save above (the DB stays the single source of truth), and a
        // backdated value never lands in HC at the wrong instant (HC keeps its own history).
        if (date == today && settings.hcWriteBodyweight.first() && health.canWriteWeight()) {
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

    /**
     * One-time bulk backfill of the ENTIRE Health Connect weight history into the local log (GYMAP-63,
     * on first connect). Reduces HC's readings to one-per-day and inserts ONLY days we don't already
     * have (see [BodyweightSync.historyToImport]) so a typed weigh-in is never overwritten and re-running
     * is idempotent.
     *
     * Returns the number of new days imported on a SUCCESSFUL read (0 when HC had nothing new), or
     * **null** when the read couldn't happen (unavailable / not granted / a transient provider error).
     * The caller latches the "history imported" flag only on a non-null result, so a momentary failure
     * right after the grant never permanently skips the backfill.
     */
    suspend fun importHistoryFromHealthConnect(): Int? {
        if (!health.canReadWeight()) return null
        val zone = ZoneId.systemDefault()
        val readings = health.readWeightHistory(0L, clock.nowMs()) ?: return null
        val dated = readings.map {
            BodyweightSync.DatedWeight(
                dateKey = Instant.ofEpochMilli(it.timeMs).atZone(zone).toLocalDate().toString(),
                weightLb = it.weightLb,
                recordedAtMs = it.timeMs
            )
        }
        val existing = dao.all().map { it.dateKey }.toSet()
        val toImport = BodyweightSync.historyToImport(dated, existing)
        toImport.forEach {
            dao.upsert(BodyweightEntry(dateKey = it.dateKey, weightLb = it.weightLb, recordedAt = it.recordedAtMs))
        }
        return toImport.size
    }
}
