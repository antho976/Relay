package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyFatDao
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.health.BodyFatSync
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Body-fat-% log (GYMAP-62) — the sibling of [BodyweightRepository]. Readings come from a smart
 * scale via Health Connect or from manual entry; the DB is the single source of truth, and a
 * same-day manual entry can mirror back to Health Connect when the user opts in.
 */
@Singleton
class BodyFatRepository @Inject constructor(
    private val dao: BodyFatDao,
    private val clock: Clock,
    private val health: HealthConnectManager,
    private val settings: SettingsRepository
) {
    fun observeRecent(limit: Int = 90): Flow<List<BodyFatEntry>> = dao.observeRecent(limit)

    suspend fun latestPercent(): Double? = dao.latest()?.percent

    /** Today, from the injected clock — the default for a reading with no explicit date. */
    private fun today(zone: ZoneId = ZoneId.systemDefault()): LocalDate =
        Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()

    /**
     * Record a body-fat reading (percent) for [date] (defaults to today). One entry per day: the
     * `date_key` upsert replaces that day's row, so re-saving edits it and backdating fills a past
     * day. `recorded_at` is stamped on the chosen day so a backdated entry sorts onto the right day.
     */
    suspend fun log(percent: Double, date: LocalDate = today()) {
        // One reading of the injected clock decides the default date, the "is this today"
        // comparison and the time-of-day a backdated stamp inherits. This read `LocalDate.now()`
        // and `LocalTime.now()` — the system clock — alongside `clock.nowMs()`, so the three could
        // disagree: with the clocks a day apart, logging TODAY took the backdate branch, stamped
        // `recorded_at` from the wrong instant and silently skipped the Health Connect mirror
        // below, which only runs for a same-day entry. [BodyweightRepository.record] already fixed
        // exactly this; the sibling never got it.
        val now = clock.nowMs()
        val zone = ZoneId.systemDefault()
        val nowLocal = Instant.ofEpochMilli(now).atZone(zone)
        val today = nowLocal.toLocalDate()
        val recordedAt =
            if (date == today) now
            else date.atTime(nowLocal.toLocalTime()).atZone(zone).toInstant().toEpochMilli()
        dao.upsert(BodyFatEntry(dateKey = date.toString(), percent = percent, recordedAt = recordedAt))
        // Mirror ONLY a same-day entry to Health Connect, and only when opted in AND write access is
        // granted — same three-gate rule as bodyweight, so a mirror failure can't break the local
        // save and a backdated value never lands in HC at the wrong instant.
        if (date == today && settings.hcWriteBodyFat.first() && health.canWriteBodyFat()) {
            health.writeBodyFat(percent, now)
        }
    }

    suspend fun delete(id: Long) = dao.delete(id)

    /** Whether an "Import from Health Connect" affordance should be offered (read permission granted). */
    suspend fun canImportFromHealthConnect(): Boolean = health.canReadBodyFat()

    /**
     * Pull the latest body fat from Health Connect and record it locally — but only when it's newer
     * than our newest entry (see [BodyFatSync.shouldImport]), so a typed figure is never overwritten
     * and re-importing is idempotent. Returns the imported percent, or null when there was nothing
     * newer (or HC is unavailable / not granted).
     */
    suspend fun importLatestFromHealthConnect(): Double? {
        val hc = health.latestBodyFat(clock.nowMs()) ?: return null
        val localLatestMs = dao.latest()?.recordedAt
        if (!BodyFatSync.shouldImport(hc.timeMs, hc.percent, localLatestMs)) return null
        val dateKey = Instant.ofEpochMilli(hc.timeMs).atZone(ZoneId.systemDefault()).toLocalDate().toString()
        dao.upsert(BodyFatEntry(dateKey = dateKey, percent = hc.percent, recordedAt = hc.timeMs))
        return hc.percent
    }
}
