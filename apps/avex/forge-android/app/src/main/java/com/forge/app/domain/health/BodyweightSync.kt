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
}
