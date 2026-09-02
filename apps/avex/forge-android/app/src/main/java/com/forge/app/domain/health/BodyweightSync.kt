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

    // ── History backfill: window vs history (H-05) ─────────────────────────────
    //
    // Health Connect limits an ordinary read to the 30 days before the app's first grant; only the
    // separate READ_HEALTH_DATA_HISTORY permission reaches further back. The backfill used to latch
    // "entire history imported" after ANY successful read, so a scale's older months vanished from
    // the migration path for good. These rules keep the latch honest and the retry available.

    /** What one backfill pass did, from what the read returned and the grant it ran under. */
    enum class HistoryOutcome {
        /** The read could not happen (no provider / not granted / a transient error): touch nothing, retry on the next refresh. */
        RETRY,
        /** The read succeeded with history access live: the entire history is in, latch the one-time flag. */
        COMPLETE,
        /** The read succeeded without history access: only the ordinary 30-day window came over. */
        PARTIAL
    }

    fun historyOutcome(readSucceeded: Boolean, historyGranted: Boolean): HistoryOutcome = when {
        !readSucceeded -> HistoryOutcome.RETRY
        historyGranted -> HistoryOutcome.COMPLETE
        else -> HistoryOutcome.PARTIAL
    }

    /**
     * Should a refresh run the backfill? Only while weight READ is granted and the history isn't
     * latched complete. A window-only pass runs once ([partial] latches it) so a declined history
     * grant doesn't re-read the provider on every refresh; but if history access has appeared since
     * (granted in Health Connect's own settings), run again to fetch the rest.
     */
    fun shouldBackfillHistory(
        weightReadGranted: Boolean,
        historyGranted: Boolean,
        complete: Boolean,
        partial: Boolean
    ): Boolean = weightReadGranted && !complete && (historyGranted || !partial)

    /**
     * Should the page say only the recent window came over and offer the older-weight import?
     * True whenever weight is connected, history access is NOT live, and a pass has already run.
     * That covers the partial latch, and also a "complete" latch with no history grant: that can
     * only be an install that latched before history access existed, or one that revoked it since,
     * so the live grant outranks the latch. The retry is idempotent either way, so offering it costs
     * nothing but a tap.
     */
    fun historyWindowIsPartial(
        weightReadGranted: Boolean,
        historyGranted: Boolean,
        complete: Boolean,
        partial: Boolean
    ): Boolean = weightReadGranted && !historyGranted && (complete || partial)
}
