package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.health.BodyweightSync
import com.forge.app.domain.health.HcRecordKeys
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

    /** Today, from the injected clock — the default for a weigh-in with no explicit date. */
    private fun today(zone: ZoneId = ZoneId.systemDefault()): LocalDate =
        Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()

    suspend fun latestWeightLb(): Double? = dao.latest()?.weightLb

    /**
     * Record a weigh-in for [date] (defaults to today) with an optional [note] (GYMAP-54). One entry
     * per day: the `date_key` upsert replaces that day's row, so re-saving edits it and backdating
     * fills a past day. `recorded_at` is stamped on the chosen day (now for today, else that day at
     * the current time-of-day) so a backdated entry sorts onto the right day in the trend rather than
     * jumping to "now".
     */
    suspend fun log(weightLb: Double, date: LocalDate = today(), note: String?) =
        record(weightLb, date, note, keepExistingNote = false)

    /**
     * Record a weigh-in from a surface that has no note field — the daily check-in and onboarding.
     *
     * Distinct from [log] because `note = null` is ambiguous on its own: from the editor it means
     * "the user cleared the note", but from here it only means "this screen has nothing to say
     * about the note". Treating the second as the first is how a check-in silently erased a note
     * typed hours earlier, so the two intents get two entry points rather than one nullable.
     */
    suspend fun logWeightOnly(weightLb: Double, date: LocalDate = today()) =
        record(weightLb, date, note = null, keepExistingNote = true)

    private suspend fun record(
        weightLb: Double,
        date: LocalDate,
        note: String?,
        keepExistingNote: Boolean
    ) {
        // One reading of the injected clock decides both "is this today" and the time-of-day the
        // backdated stamp inherits, so the default date, the comparison and the timestamp can never
        // come from three different instants (or, under a FakeClock, from three different days).
        val now = clock.nowMs()
        val zone = ZoneId.systemDefault()
        val nowLocal = Instant.ofEpochMilli(now).atZone(zone)
        val todayLocal = nowLocal.toLocalDate()
        val recordedAt =
            if (date == todayLocal) now
            else date.atTime(nowLocal.toLocalTime()).atZone(zone).toInstant().toEpochMilli()
        val dateKey = date.toString()
        // dao.upsert is INSERT OR REPLACE, which DELETES the conflicting row rather than updating
        // it — so the day's existing note and id only survive if we carry them forward ourselves.
        val existing = dao.byDateKey(dateKey)
        dao.upsert(
            BodyweightEntry(
                id = existing?.id ?: 0,
                dateKey = dateKey,
                weightLb = weightLb,
                recordedAt = recordedAt,
                note = if (keepExistingNote) existing?.note
                       else note?.trim()?.takeIf { it.isNotBlank() }
            )
        )
        // Mirror ONLY a same-day weigh-in to Health Connect, and only when the user has opted in AND
        // granted write access. Gated on all three so onboarding (neither set) never writes, a mirror
        // failure can't break the local save above (the DB stays the single source of truth), and a
        // backdated value never lands in HC at the wrong instant (HC keeps its own history).
        if (date == todayLocal && settings.hcWriteBodyweight.first() && health.canWriteWeight()) {
            // Keyed on the day, not the row id: the upsert above hands a re-saved day a fresh id,
            // while the date_key is the one identity the entry keeps. So a second weigh-in today
            // UPDATES the mirror instead of adding a duplicate, and [delete] can find it (M-02).
            health.writeWeight(
                weightLb, now,
                clientRecordId = HcRecordKeys.weight(dateKey),
                clientRecordVersion = now
            )
        }
    }

    /**
     * Delete a weigh-in and the Health Connect copy Avex wrote for it (M-02). The local delete
     * comes first and never waits on the mirror; the mirror delete is fail-soft and a no-op for a
     * day that was never mirrored (backdated, or logged before the write-back opt-in).
     */
    suspend fun delete(id: Long) {
        val entry = dao.byId(id)
        dao.delete(id)
        if (entry != null) health.deleteWeights(listOf(HcRecordKeys.weight(entry.dateKey)))
    }

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
        // shouldImport compares timestamps only, so a scale reading later the SAME day passes the
        // guard and collides on date_key. Under INSERT OR REPLACE that deleted the typed weigh-in
        // outright, taking its note with it — the one thing Health Connect cannot supply.
        val existing = dao.byDateKey(dateKey)
        dao.upsert(
            BodyweightEntry(
                id = existing?.id ?: 0,
                dateKey = dateKey,
                weightLb = hc.weightLb,
                recordedAt = hc.timeMs,
                note = existing?.note
            )
        )
        return hc.weightLb
    }

    /**
     * Bulk backfill of the Health Connect weight history into the local log (GYMAP-63, on first
     * connect). Reduces HC's readings to one-per-day and inserts ONLY days we don't already have
     * (see [BodyweightSync.historyToImport]) so a typed weigh-in is never overwritten and re-running
     * is idempotent.
     *
     * How much history this reaches is set by the grant, not the range asked for: without
     * [HealthConnectManager.canReadHistory] Health Connect returns only the 30 days before Avex's
     * first grant (H-05). The caller checks that grant and latches "entire history imported" only
     * when it was live; otherwise it records a partial window and keeps the retry offered.
     *
     * Returns the number of new days imported on a SUCCESSFUL read (0 when HC had nothing new), or
     * **null** when the read couldn't happen (unavailable / not granted / a transient provider error).
     * The caller latches either flag only on a non-null result, so a momentary failure right after
     * the grant never permanently skips the backfill.
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
