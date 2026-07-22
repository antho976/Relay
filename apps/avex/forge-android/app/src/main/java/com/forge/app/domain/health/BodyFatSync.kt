package com.forge.app.domain.health

/**
 * Pure decision rules for the Health Connect ⇄ Avex body-fat bridge (GYMAP-62), the sibling of
 * [BodyweightSync]. The Android-bound read/write lives in
 * [com.forge.app.data.health.HealthConnectManager]; this is the testable core so the "only import
 * something newer" rule can't silently regress.
 */
object BodyFatSync {

    /**
     * Should a Health Connect body-fat reading be imported into the local log? Only when there's a
     * reading AND it's strictly newer than what we already have — so a stale scale value can never
     * clobber a figure the user just typed, and re-importing is idempotent (an equal timestamp is a
     * no-op). A non-physical percentage (≤0 or >100) is rejected as corrupt.
     *
     * @param hcTimeMs      when the HC reading was taken (epoch-ms), or null if HC has nothing.
     * @param hcPercent     the HC reading as a percentage 0–100, or null if HC has nothing.
     * @param localLatestMs when the newest local entry was recorded (epoch-ms), or null if none.
     */
    fun shouldImport(hcTimeMs: Long?, hcPercent: Double?, localLatestMs: Long?): Boolean {
        if (hcTimeMs == null || hcPercent == null) return false
        if (hcPercent <= 0.0 || hcPercent > 100.0) return false
        return localLatestMs == null || hcTimeMs > localLatestMs
    }
}
