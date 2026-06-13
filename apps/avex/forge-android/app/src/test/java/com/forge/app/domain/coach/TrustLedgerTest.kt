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
    fun foldedChangesStillCountAsAcceptances() {
        // A regenerate baked these accepted changes into the baseline (status=folded). They must
        // keep counting toward the streak, otherwise folding would silently reset earned trust
        // and autopilot could never be earned by changes a refresh absorbed (seam fix).
        val d = (1..3).map { decision("rep_shift", "folded", outcome = "pending") }
        val t = trustFor("rep_shift", d)
        assertEquals(3, t.streak)
        assertTrue(t.earned)
    }

    @Test
    fun foldedAndAppliedAcceptancesCompose() {
        val d = (1..2).map { decision("swap", "folded") } +
            (1..2).map { decision("swap", "applied", "ok") }
        // 2 folded + 2 applied = a streak of 4 → an aggressive type earns.
        assertEquals(4, trustFor("swap", d).streak)
        assertTrue(trustFor("swap", d).earned)
    }
}
