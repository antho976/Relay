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

    // ── Window vs history (H-05) ───────────────────────────────────────────────

    @Test
    fun historyLatchesCompleteOnlyWhenHistoryAccessWasLive() {
        assertEquals(
            BodyweightSync.HistoryOutcome.COMPLETE,
            BodyweightSync.historyOutcome(readSucceeded = true, historyGranted = true)
        )
        // A successful read WITHOUT history access reached only the 30-day window: partial, never
        // complete. This is the latch that used to swallow months of a smart scale's history.
        assertEquals(
            BodyweightSync.HistoryOutcome.PARTIAL,
            BodyweightSync.historyOutcome(readSucceeded = true, historyGranted = false)
        )
    }

    @Test
    fun historyFailedReadLatchesNothingWhateverTheGrant() {
        assertEquals(
            BodyweightSync.HistoryOutcome.RETRY,
            BodyweightSync.historyOutcome(readSucceeded = false, historyGranted = true)
        )
        assertEquals(
            BodyweightSync.HistoryOutcome.RETRY,
            BodyweightSync.historyOutcome(readSucceeded = false, historyGranted = false)
        )
    }

    @Test
    fun backfillRunsOnFirstGrantWithOrWithoutHistory() {
        assertTrue(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = true, complete = false, partial = false))
        // History declined: the ordinary 30-day window still imports.
        assertTrue(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = false, complete = false, partial = false))
    }

    @Test
    fun backfillNeverRunsWithoutWeightReadOrOnceComplete() {
        assertFalse(BodyweightSync.shouldBackfillHistory(weightReadGranted = false, historyGranted = true, complete = false, partial = false))
        assertFalse(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = true, complete = true, partial = false))
        assertFalse(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = false, complete = true, partial = false))
    }

    @Test
    fun partialWindowRunsOnceUntilHistoryAccessAppears() {
        // Declined history + window already imported: don't re-read the provider every refresh.
        assertFalse(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = false, complete = false, partial = true))
        // History granted later (in Health Connect's own settings): fetch the rest.
        assertTrue(BodyweightSync.shouldBackfillHistory(weightReadGranted = true, historyGranted = true, complete = false, partial = true))
    }

    @Test
    fun pageSaysPartialOnlyAfterAWindowPassWithoutHistory() {
        assertTrue(BodyweightSync.historyWindowIsPartial(weightReadGranted = true, historyGranted = false, complete = false, partial = true))
        // Nothing has run yet (or the first read failed): nothing to disclose, the retry is the refresh.
        assertFalse(BodyweightSync.historyWindowIsPartial(weightReadGranted = true, historyGranted = false, complete = false, partial = false))
        // Complete with history live: the whole history is in.
        assertFalse(BodyweightSync.historyWindowIsPartial(weightReadGranted = true, historyGranted = true, complete = true, partial = false))
        // Weight disconnected: the row has nothing to say about history.
        assertFalse(BodyweightSync.historyWindowIsPartial(weightReadGranted = false, historyGranted = false, complete = true, partial = false))
    }

    @Test
    fun pageDoesNotTrustAPreHistoryLatchOverTheLiveGrant() {
        // An install that latched "complete" before history access existed could only have imported
        // the window: offer the older import rather than believe the latch.
        assertTrue(BodyweightSync.historyWindowIsPartial(weightReadGranted = true, historyGranted = false, complete = true, partial = false))
    }
}
