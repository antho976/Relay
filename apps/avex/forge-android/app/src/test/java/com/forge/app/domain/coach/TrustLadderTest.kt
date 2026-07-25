package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * E's initiative ladder. The two properties that matter: it can't oscillate, and it can't take
 * autonomy without being given it.
 */
class TrustLadderTest {

    private fun decision(outcome: String, status: String = "applied", id: Long = 0) = CoachDecision(
        id = id, weekId = "2026-W01", type = "weight_nudge", targetKey = "ua1", targetName = "Lift",
        summary = "s", reason = "r", status = status, outcome = outcome
    )

    private fun ledger(wins: Int, failures: Int, reverts: Int = 0): List<CoachDecision> =
        (1..wins).map { decision(CoachDecision.OUTCOME_OK, id = it.toLong()) } +
            (1..failures).map { decision(CoachDecision.OUTCOME_FAILED, id = 100L + it) } +
            (1..reverts).map { decision(CoachDecision.OUTCOME_OK, status = "reverted", id = 200L + it) }

    // ── Earning ────────────────────────────────────────────────────────────────

    @Test
    fun aNewCoachObserves() {
        val a = TrustLadder.assess(emptyList(), weeksCoached = 0)
        assertEquals(TrustLadder.Tier.OBSERVE, a.tier)
        assertFalse(a.mayInitiate)
    }

    @Test
    fun aFewCallsInItOnlyProposes() {
        val a = TrustLadder.assess(ledger(wins = 3, failures = 0), weeksCoached = 1)
        assertEquals(TrustLadder.Tier.PROPOSE, a.tier)
    }

    @Test
    fun aSolidRecordEarnsAutoApply() {
        val a = TrustLadder.assess(ledger(wins = 8, failures = 3), weeksCoached = 4)
        assertEquals(TrustLadder.Tier.AUTO_APPLY, a.tier)
        assertFalse("initiative is a later tier", a.mayInitiate)
    }

    @Test
    fun aLongGoodRecordEarnsInitiative() {
        val a = TrustLadder.assess(ledger(wins = 16, failures = 4), weeksCoached = 10)
        assertEquals(TrustLadder.Tier.PROACTIVE, a.tier)
        assertTrue(a.mayInitiate)
        assertFalse("acting first is T4", a.mayActFirst)
        assertEquals(4, a.changesPerWeek)
        assertEquals(2, a.volumeStep)
    }

    // ── Autonomy is given, never taken ─────────────────────────────────────────

    @Test
    fun autonomyIsEarnedButNotAssumed() {
        val a = TrustLadder.assess(ledger(wins = 19, failures = 2), weeksCoached = 20)
        assertEquals(TrustLadder.Tier.AUTONOMOUS, a.tier)
        assertEquals("without consent it acts as proactive", TrustLadder.Tier.PROACTIVE, a.effective)
        assertTrue(a.awaitingConsent)
        assertFalse(a.mayActFirst)
    }

    @Test
    fun consentUnlocksIt() {
        val a = TrustLadder.assess(ledger(wins = 19, failures = 2), weeksCoached = 20, autonomyConsented = true)
        assertEquals(TrustLadder.Tier.AUTONOMOUS, a.effective)
        assertTrue(a.mayActFirst)
        assertFalse(a.awaitingConsent)
    }

    @Test
    fun theUsersCapAlwaysWins() {
        val a = TrustLadder.assess(
            ledger(wins = 19, failures = 2), weeksCoached = 20,
            userCap = TrustLadder.Tier.PROPOSE, autonomyConsented = true
        )
        assertEquals(TrustLadder.Tier.AUTONOMOUS, a.tier)
        assertEquals(TrustLadder.Tier.PROPOSE, a.effective)
        assertFalse(a.mayInitiate)
    }

    // ── Demotion is rate-based, with hysteresis ────────────────────────────────

    @Test
    fun oneFailureNeverDemotes() {
        // The exact failure mode the plan called out: a T3 coach making many calls at a real-world
        // win rate must not fall a tier the moment one call misses.
        val a = TrustLadder.assess(ledger(wins = 15, failures = 5), weeksCoached = 10)
        assertFalse(TrustLadder.shouldDemote(TrustLadder.Tier.PROACTIVE, a))
    }

    @Test
    fun aSustainedBadRunDoesDemote() {
        val a = TrustLadder.assess(ledger(wins = 8, failures = 12), weeksCoached = 10)
        assertTrue(TrustLadder.shouldDemote(TrustLadder.Tier.PROACTIVE, a))
    }

    @Test
    fun thereIsABandBetweenEarningAndLosing() {
        // A rate that would no longer EARN T3 still HOLDS it — that gap is the hysteresis.
        val a = TrustLadder.assess(ledger(wins = 14, failures = 6), weeksCoached = 10)
        assertTrue("0.70 would not earn T3", a.winRate < TrustLadder.T3_EARN_RATE)
        assertFalse("but it holds it", TrustLadder.shouldDemote(TrustLadder.Tier.PROACTIVE, a))
    }

    @Test
    fun repeatedUserRevertsCapTheCoachRegardlessOfRate() {
        val a = TrustLadder.assess(ledger(wins = 20, failures = 1, reverts = 3), weeksCoached = 20)
        assertEquals(TrustLadder.Tier.PROPOSE, a.tier)
        assertTrue(TrustLadder.shouldDemote(TrustLadder.Tier.PROACTIVE, a))
    }

    @Test
    fun sparseHistoryNeverDemotes() {
        val a = TrustLadder.assess(ledger(wins = 1, failures = 2), weeksCoached = 6)
        assertFalse(TrustLadder.shouldDemote(TrustLadder.Tier.AUTO_APPLY, a))
    }

    @Test
    fun notFollowedIsInvisibleToTrust() {
        val withAbsence = ledger(wins = 8, failures = 3) +
            (1..5).map { decision(CoachDecision.OUTCOME_NOT_FOLLOWED, id = 300L + it) }
        val without = TrustLadder.assess(ledger(wins = 8, failures = 3), weeksCoached = 4)
        val with = TrustLadder.assess(withAbsence, weeksCoached = 4)
        assertEquals(without.winRate, with.winRate, 0.0001)
        assertEquals(without.tier, with.tier)
    }

    // ── Readout ────────────────────────────────────────────────────────────────

    @Test
    fun theReadoutStatesWhatItsBasedOn() {
        val a = TrustLadder.assess(ledger(wins = 16, failures = 4), weeksCoached = 10)
        val line = TrustLadder.describe(a)
        assertTrue(line.contains("80%"))
        assertTrue(line.contains("20 judged calls"))
    }

    @Test
    fun deterministic() {
        val l = ledger(wins = 9, failures = 3)
        assertEquals(TrustLadder.assess(l, 5), TrustLadder.assess(l, 5))
    }
}
