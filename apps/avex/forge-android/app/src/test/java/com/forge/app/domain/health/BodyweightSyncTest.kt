package com.forge.app.domain.health

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BodyweightSyncTest {

    @Test
    fun importsWhenNoLocalEntryExists() {
        assertTrue(BodyweightSync.shouldImport(hcTimeMs = 1_000, hcWeightLb = 180.0, localLatestMs = null))
    }

    @Test
    fun importsWhenHealthConnectReadingIsNewer() {
        assertTrue(BodyweightSync.shouldImport(hcTimeMs = 2_000, hcWeightLb = 180.0, localLatestMs = 1_000))
    }

    @Test
    fun skipsWhenLocalEntryIsNewerOrSameAge() {
        // A weigh-in typed in Forge just now must not be clobbered by an older scale reading.
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = 1_000, hcWeightLb = 180.0, localLatestMs = 2_000))
        // Equal timestamps → idempotent no-op (re-running import doesn't duplicate).
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = 1_000, hcWeightLb = 180.0, localLatestMs = 1_000))
    }

    @Test
    fun skipsWhenHealthConnectHasNothing() {
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = null, hcWeightLb = null, localLatestMs = null))
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = 1_000, hcWeightLb = null, localLatestMs = null))
    }

    @Test
    fun skipsCorruptNonPositiveReading() {
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = 9_000, hcWeightLb = 0.0, localLatestMs = null))
        assertFalse(BodyweightSync.shouldImport(hcTimeMs = 9_000, hcWeightLb = -5.0, localLatestMs = null))
    }
}
