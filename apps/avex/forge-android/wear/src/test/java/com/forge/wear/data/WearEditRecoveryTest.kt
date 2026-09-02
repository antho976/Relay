package com.forge.wear.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * M-10: an edit the watch could not deliver must come back, not disappear.
 *
 * The reproduction is an ordinary one — walk out of Bluetooth range and tap Save RPE. The rating
 * hid its own row before the transport was asked, `sendWithRetry` gave up, and nothing put the row
 * back: the rating was never queued and there was nowhere left to make it from.
 */
class WearEditRecoveryTest {

    private val logged = WristLastLog(setId = 42L, atLocalMs = 1_000L, rpeSent = true)

    @Test
    fun aFailedRatingOffersTheSameSetAgain() {
        val restored = WearEditRecovery.afterFailedRating(logged, ratedSetId = 42L)

        assertEquals(42L, restored?.setId)
        assertFalse("the rate row must come back", restored?.rpeSent ?: true)
        assertEquals("and it is the same window, not a fresh one", 1_000L, restored?.atLocalMs)
    }

    @Test
    fun aFailedRatingDoesNotReopenARowThatHasMovedOn() {
        // A later set was logged while the rating was in flight. Reopening the row here would
        // invite a rating that lands on the wrong set — the ten-minute RPE window is wide enough
        // for exactly that.
        val newerSet = WristLastLog(setId = 43L, atLocalMs = 9_000L, rpeSent = false)

        assertEquals(newerSet, WearEditRecovery.afterFailedRating(newerSet, ratedSetId = 42L))
        assertNull(WearEditRecovery.afterFailedRating(null, ratedSetId = 42L))
    }

    @Test
    fun aFailedUndoPutsItsRowBack() {
        val removed = WristLastLog(setId = 42L, atLocalMs = 1_000L)

        assertEquals(removed, WearEditRecovery.afterFailedUndo(current = null, removed = removed))
    }

    @Test
    fun aFailedUndoDoesNotResurrectARowUnderALaterSet() {
        val newerSet = WristLastLog(setId = 43L, atLocalMs = 9_000L)
        val removed = WristLastLog(setId = 42L, atLocalMs = 1_000L)

        assertEquals(newerSet, WearEditRecovery.afterFailedUndo(current = newerSet, removed = removed))
    }

    @Test
    fun aFailedUndoWithNothingToRestoreStaysEmpty() {
        assertNull(WearEditRecovery.afterFailedUndo(current = null, removed = null))
    }
}
