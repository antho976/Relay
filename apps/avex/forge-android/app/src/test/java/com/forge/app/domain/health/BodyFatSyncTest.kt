package com.forge.app.domain.health

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BodyFatSyncTest {

    @Test
    fun importsWhenNoLocalEntryExists() {
        assertTrue(BodyFatSync.shouldImport(hcTimeMs = 1_000, hcPercent = 18.0, localLatestMs = null))
    }

    @Test
    fun importsWhenHealthConnectReadingIsNewer() {
        assertTrue(BodyFatSync.shouldImport(hcTimeMs = 2_000, hcPercent = 18.0, localLatestMs = 1_000))
    }

    @Test
    fun skipsWhenLocalEntryIsNewerOrSameAge() {
        // A figure typed in Avex just now must not be clobbered by an older scale reading.
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 1_000, hcPercent = 18.0, localLatestMs = 2_000))
        // Equal timestamps → idempotent no-op (re-running import doesn't duplicate).
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 1_000, hcPercent = 18.0, localLatestMs = 1_000))
    }

    @Test
    fun skipsWhenHealthConnectHasNothing() {
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = null, hcPercent = null, localLatestMs = null))
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 1_000, hcPercent = null, localLatestMs = null))
    }

    @Test
    fun skipsCorruptOutOfRangeReading() {
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 9_000, hcPercent = 0.0, localLatestMs = null))
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 9_000, hcPercent = -5.0, localLatestMs = null))
        assertFalse(BodyFatSync.shouldImport(hcTimeMs = 9_000, hcPercent = 150.0, localLatestMs = null))
    }
}
