package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BodyweightSyncTest {

    private fun w(dateKey: String, weightLb: Double, recordedAtMs: Long) =
        BodyweightSync.DatedWeight(dateKey, weightLb, recordedAtMs)

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

    // ── History backfill (GYMAP-63) ────────────────────────────────────────────

    @Test
    fun historyImportsEveryNewDaySortedChronologically() {
        val readings = listOf(
            w("2026-01-03", 181.0, 3),
            w("2026-01-01", 180.0, 1),
            w("2026-01-02", 180.5, 2),
        )
        val result = BodyweightSync.historyToImport(readings, existingDateKeys = emptySet())
        assertEquals(listOf("2026-01-01", "2026-01-02", "2026-01-03"), result.map { it.dateKey })
    }

    @Test
    fun historyNeverClobbersADayThatAlreadyExists() {
        val readings = listOf(w("2026-01-01", 180.0, 1), w("2026-01-02", 181.0, 2))
        // The user already has a typed weigh-in on the 1st — it must be preserved, not overwritten.
        val result = BodyweightSync.historyToImport(readings, existingDateKeys = setOf("2026-01-01"))
        assertEquals(listOf("2026-01-02"), result.map { it.dateKey })
    }

    @Test
    fun historyKeepsTheLatestReadingOfEachDay() {
        val readings = listOf(
            w("2026-01-01", 180.0, 100),
            w("2026-01-01", 179.0, 300), // later in the day → this is the one kept
            w("2026-01-01", 181.0, 200),
        )
        val result = BodyweightSync.historyToImport(readings, existingDateKeys = emptySet())
        assertEquals(1, result.size)
        assertEquals(179.0, result.single().weightLb, 0.0001)
        assertEquals(300L, result.single().recordedAtMs)
    }

    @Test
    fun historyDropsCorruptRowsAndIsIdempotent() {
        val readings = listOf(
            w("2026-01-01", 0.0, 1),   // non-positive → dropped
            w("2026-01-01", -3.0, 2),  // negative → dropped
            w("", 180.0, 3),           // blank date key → dropped
            w("2026-01-02", 180.0, 4),
        )
        val first = BodyweightSync.historyToImport(readings, existingDateKeys = emptySet())
        assertEquals(listOf("2026-01-02"), first.map { it.dateKey })
        // Re-running with those days now present imports nothing (idempotent backfill).
        val second = BodyweightSync.historyToImport(readings, existingDateKeys = setOf("2026-01-02"))
        assertTrue(second.isEmpty())
    }
}
