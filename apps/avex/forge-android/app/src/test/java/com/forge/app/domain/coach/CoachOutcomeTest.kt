package com.forge.app.domain.coach

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CoachOutcomeTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 100 * day

    @Test
    fun appliedPending_countsDownTheWatcherWindow() {
        // Applied 4 days ago → 14 − 4 = ~10 days left in the watcher window.
        val l = CoachOutcome.label("applied", "pending", now - 4 * day, now)!!
        assertTrue(l.contains("still watching"))
        assertTrue(l.contains("10"))
    }

    @Test
    fun appliedVerdicts_readPlainEnglish() {
        assertEquals("worked", CoachOutcome.label("applied", "ok", now - 20 * day, now))
        assertEquals("didn't stick", CoachOutcome.label("applied", "failed", now - 20 * day, now))
    }

    @Test
    fun windowClosedButStillPending_dropsTheCountdown() {
        assertEquals("still watching", CoachOutcome.label("applied", "pending", now - 20 * day, now))
    }

    @Test
    fun skippedAndReverted_arePlainEnglish() {
        assertEquals("you skipped this", CoachOutcome.label("skipped", "pending", null, now))
        assertEquals("you undid this", CoachOutcome.label("reverted", "failed", null, now))
    }

    @Test
    fun openProposal_addsNothing() {
        assertNull(CoachOutcome.label("proposed", "pending", null, now))
    }

    @Test
    fun shadow_readsAsObservedWhilePaused() {
        // Recorded while the coach was switched off — watched, never acted on.
        assertEquals("observed while paused", CoachOutcome.label("shadow", "pending", null, now))
    }
}
