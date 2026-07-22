package com.forge.app.domain.health

/**
 * Pure decision rules for the Health Connect ⇄ Avex bodyweight bridge (HC-2). The Android-bound
 * read/write lives in [com.forge.app.data.health.HealthConnectManager]; this is the testable core
 * so the "only import something newer" rule can't silently regress.
 */
object BodyweightSync {

    /**
     * Should a Health Connect reading be imported into the local log? Only when there's a reading
     * AND it's strictly newer than what we already have — so a stale scale value can never clobber
     * a weigh-in the user just typed, and re-importing is idempotent (an equal timestamp is a no-op).
     *
     * @param hcTimeMs     when the HC reading was taken (epoch-ms), or null if HC has nothing.
     * @param hcWeightLb   the HC reading in lb, or null if HC has nothing.
     * @param localLatestMs when the newest local entry was recorded (epoch-ms), or null if none.
     */
    fun shouldImport(hcTimeMs: Long?, hcWeightLb: Double?, localLatestMs: Long?): Boolean {
        if (hcTimeMs == null || hcWeightLb == null) return false
        if (hcWeightLb <= 0.0) return false
        return localLatestMs == null || hcTimeMs > localLatestMs
    }

    /** One Health Connect weigh-in reduced to the day it lands on (system zone) — the unit the bulk
     *  history backfill (GYMAP-63) works in, since the local log stores one row per calendar day. */
    data class DatedWeight(val dateKey: String, val weightLb: Double, val recordedAtMs: Long)

    /**
     * Which HC history [readings] to insert on the first-connect backfill (GYMAP-63):
     *  - drop non-positive weights and blank date keys (corrupt rows),
     *  - keep the LATEST reading of each day (HC can hold several per day; the log keeps one),
     *  - and NEVER a day that already has a local entry ([existingDateKeys]) — so a manually typed or
     *    previously synced weigh-in is preserved, never clobbered.
     * The result is sorted by day so inserts land chronologically. Pure + tested so the "don't clobber"
     * rule can't silently regress; the Android/zone date-keying happens in the repository.
     */
    fun historyToImport(readings: List<DatedWeight>, existingDateKeys: Set<String>): List<DatedWeight> =
        readings
            .filter { it.weightLb > 0.0 && it.dateKey.isNotBlank() && it.dateKey !in existingDateKeys }
            .groupBy { it.dateKey }
            .map { (_, sameDay) -> sameDay.maxByOrNull { it.recordedAtMs }!! }
            .sortedBy { it.dateKey }
}
