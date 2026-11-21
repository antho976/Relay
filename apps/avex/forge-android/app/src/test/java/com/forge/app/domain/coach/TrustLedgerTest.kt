package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Hardening decisions 2 + 3 — per-type earned auto-apply, demoted on any bad outcome. */
class TrustLedgerTest {

    private var nextId = 1L

    private fun decision(
        type: String,
        status: String,
        outcome: String = "pending"
    ) = CoachDecision(
        id = nextId++, weekId = "2026-W${nextId}", type = type, targetKey = "ua1",
        targetName = "Lift", summary = "s", reason = "r", status = status,
        dayKey = "upper-a", payload = null, appliedAt = nextId * 1000,
        outcome = outcome, undoData = "∅"
    )

    private fun trustFor(type: String, decisions: List<CoachDecision>): TypeTrust =
        TrustLedger.assess(decisions).first { it.type == type }

    @Test
    fun coldStart_nothingEarned() {
        val trust = TrustLedger.assess(emptyList())
        assertTrue(trust.all { !it.earned && it.streak == 0 })
        assertTrue(TrustLedger.earnedTypes(emptyList()).isEmpty())
    }

    @Test
    fun conservativeType_earnsAtThree() {
        val d = (1..3).map { decision("rep_shift", "applied", outcome = "ok") }
        val t = trustFor("rep_shift", d)
        assertEquals(3, t.streak)
        assertTrue(t.earned)
    }

    @Test
    fun aggressiveType_needsFour() {
        val three = (1..3).map { decision("swap", "applied", outcome = "ok") }
        assertFalse(trustFor("swap", three).earned)
        val four = three + decision("swap", "applied")
        assertTrue(trustFor("swap", four).earned)
    }

    @Test
    fun skipBreaksTheStreak() {
        val d = listOf(
            decision("rep_shift", "applied", "ok"),
            decision("rep_shift", "applied", "ok"),
            decision("rep_shift", "skipped"),
            decision("rep_shift", "applied", "ok")
        )
        // Newest-first walk: 1 applied, then the skip breaks it.
        assertEquals(1, trustFor("rep_shift", d).streak)
    }

    @Test
    fun failedOutcomeDemotes_evenAfterEarning() {
        val d = (1..4).map { decision("swap", "applied", "ok") } +
            decision("swap", "applied", "failed")
        val t = trustFor("swap", d)
        assertEquals(0, t.streak)
        assertFalse(t.earned)
    }

    @Test
    fun userRevertDemotes() {
        val d = (1..3).map { decision("volume_down", "applied", "ok") } +
            decision("volume_down", "reverted", "failed")
        assertFalse(trustFor("volume_down", d).earned)
    }

    @Test
    fun trustRebuildsAfterADemotion() {
        val d = decision("rep_shift", "applied", "failed").let { failure ->
            listOf(failure) + (1..3).map { decision("rep_shift", "applied", "ok") }
        }
        assertTrue(trustFor("rep_shift", d).earned)
    }

    @Test
    fun deloadAndRevertNeverEarn() {
        val d = (1..10).flatMap {
            listOf(decision("deload", "applied", "ok"), decision("revert", "applied", "ok"))
        }
        assertTrue(TrustLedger.earnedTypes(d).isEmpty())
    }

    @Test
    fun undecidedProposalsDoNotCount() {
        val d = (1..3).map { decision("rep_shift", "applied", "ok") } +
            decision("rep_shift", "proposed")
        // The open proposal neither extends nor breaks the streak.
        assertEquals(3, trustFor("rep_shift", d).streak)
    }

    @Test
    fun foldedPending_doesNotCount_untilTheWatcherValidatesIt() {
        // A regenerate folded these before their outcome window closed (status=folded, outcome=pending).
        // Folding ends the watcher's jurisdiction, so an unjudged fold is "applied but never validated"
        // and must NOT advance trust — otherwise a refresh/auto-rotation could promote autopilot on
        // changes nothing ever judged (auto-coach seam audit 2026-06-15, finding P1).
        val d = (1..3).map { decision("rep_shift", "folded", outcome = "pending") }
        val t = trustFor("rep_shift", d)
        assertEquals(0, t.streak)
        assertFalse(t.earned)
        assertTrue(TrustLedger.earnedTypes(d).isEmpty())
    }

    @Test
    fun foldedOk_countsAsAValidatedAcceptance() {
        // Once the watcher rules an in-window folded change "ok", it has earned its credit and counts.
        val d = (1..3).map { decision("rep_shift", "folded", outcome = "ok") }
        val t = trustFor("rep_shift", d)
        assertEquals(3, t.streak)
        assertTrue(t.earned)
    }

    @Test
    fun foldedFailed_breaksTheStreak() {
        // A folded change the watcher later judged failed demotes the type, just like an applied failure.
        val d = (1..3).map { decision("rep_shift", "applied", "ok") } +
            decision("rep_shift", "folded", outcome = "failed")
        assertEquals(0, trustFor("rep_shift", d).streak)
    }

    @Test
    fun unperformedChanges_areInvisibleToTrust() {
        // A swap or rep shift the coach could not actually perform (its slot left the program, or it
        // already sat at the proposed value) is retired as skipped + NOT FOLLOWED (audit M-08). It
        // must neither break a streak like a real skip nor extend one like an accepted change.
        val d = listOf(
            decision("rep_shift", "applied", "ok"),
            decision("rep_shift", "skipped", CoachDecision.OUTCOME_NOT_FOLLOWED),
            decision("rep_shift", "applied", "ok")
        )
        assertEquals(2, trustFor("rep_shift", d).streak)
    }

    @Test
    fun aRunOfAppliedButNeverTrainedChanges_earnsNothing() {
        // Four swaps applied but never performed close as NOT FOLLOWED — the watcher no longer scores
        // an empty window as ok — so an aggressive type cannot reach autopilot on them.
        val d = (1..4).map { decision("swap", "applied", CoachDecision.OUTCOME_NOT_FOLLOWED) }
        assertEquals(0, trustFor("swap", d).streak)
        assertTrue(TrustLedger.earnedTypes(d).isEmpty())
    }

    @Test
    fun foldedOkAndAppliedAcceptancesCompose() {
        val d = (1..2).map { decision("swap", "folded", "ok") } +
            (1..2).map { decision("swap", "applied", "ok") }
        // 2 validated folds + 2 applied-ok = a streak of 4 → an aggressive type earns.
        assertEquals(4, trustFor("swap", d).streak)
        assertTrue(trustFor("swap", d).earned)
    }
}
