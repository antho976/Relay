package com.forge.app.domain.health

/**
 * Pure decision rule for the Health Connect → Avex lean-mass import (W6), the sibling of
 * [BodyFatSync]. Import-only (the watch's BIA sensor is the sole author), so the single rule is
 * "only something newer, and physically plausible".
 */
object LeanMassSync {

    /**
     * Should a Health Connect lean-mass reading be imported into the local log? Only when there is
     * one AND it's strictly newer than the newest local entry — re-importing is idempotent — and
     * the value is physically plausible (a corrupt 0 or a tonne-scale row never lands).
     */
    fun shouldImport(hcTimeMs: Long?, hcWeightLb: Double?, localLatestMs: Long?): Boolean {
        if (hcTimeMs == null || hcWeightLb == null) return false
        if (hcWeightLb <= 0.0 || hcWeightLb > 700.0) return false
        return localLatestMs == null || hcTimeMs > localLatestMs
    }
}
