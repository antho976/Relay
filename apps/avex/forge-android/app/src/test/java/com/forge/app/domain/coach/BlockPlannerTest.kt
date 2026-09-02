package com.forge.app.domain.coach

import com.forge.app.data.db.entities.TrainingBlock
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Phase C's state machine: scheduled, idempotent, and still interruptible by real fatigue. */
class BlockPlannerTest {

    private val now = 1_000_000L

    private fun start(weeks: Int = BlockPlanner.DEFAULT_WEEKS) =
        BlockPlanner.start(nowMs = now, weekId = "2026-W01", plannedWeeks = weeks)

    /** Walk the block forward [n] weeks, one ISO week at a time. */
    private fun walk(block: TrainingBlock, n: Int, fatigue: Int = 0): TrainingBlock {
        var b = block
        for (i in 2..(n + 1)) {
            b = BlockPlanner.advance(b, "2026-W%02d".format(i), now + i * 1000L, fatigue)
        }
        return b
    }

    // ── Starting ───────────────────────────────────────────────────────────────

    @Test
    fun aNewBlockStartsAccumulatingAtWeekOne() {
        val b = start()
        assertEquals(BlockPhase.ACCUMULATE.code, b.phase)
        assertEquals(1, b.weekIndex)
        assertTrue(b.isActive)
    }

    @Test
    fun anAbsurdlyShortBlockIsClampedToSomethingExpressible() {
        assertEquals(BlockPlanner.MIN_WEEKS, start(weeks = 1).plannedWeeks)
    }

    // ── Advancing ──────────────────────────────────────────────────────────────

    @Test
    fun advancingTwiceInOneWeekMovesNothing() {
        val b = start()
        val once = BlockPlanner.advance(b, "2026-W02", now)
        val twice = BlockPlanner.advance(once, "2026-W02", now)
        assertEquals(once, twice)
    }

    @Test
    fun aFiveWeekBlockRunsBuildIntensifyPeakDeload() {
        var b = start(weeks = 5)
        assertEquals(BlockPhase.ACCUMULATE.code, b.phase)
        b = BlockPlanner.advance(b, "2026-W02", now)
        assertEquals(BlockPhase.ACCUMULATE.code, b.phase) // week 2
        b = BlockPlanner.advance(b, "2026-W03", now)
        assertEquals(BlockPhase.INTENSIFY.code, b.phase)  // week 3
        b = BlockPlanner.advance(b, "2026-W04", now)
        assertEquals(BlockPhase.PEAK.code, b.phase)       // week 4
        b = BlockPlanner.advance(b, "2026-W05", now)
        assertEquals(BlockPhase.DELOAD.code, b.phase)     // week 5
    }

    @Test
    fun theBlockEndsAfterItsDeloadWeek() {
        val deloaded = walk(start(weeks = 5), 4)
        assertEquals(BlockPhase.DELOAD.code, deloaded.phase)
        val ended = BlockPlanner.advance(deloaded, "2026-W07", now)
        assertFalse(ended.isActive)
        assertNotNull(ended.endedAt)
    }

    @Test
    fun weeksAwayAreCaughtUp_notCollapsedIntoOne() {
        // Three unopened weeks used to advance the block by one, so "Deload in N weeks" was wrong by
        // however long the user was away.
        val b = BlockPlanner.advance(start(weeks = 6), "2026-W04", now)
        assertEquals(4, b.weekIndex)
    }

    @Test
    fun anEarlierWeekNeverRewindsTheBlock() {
        val b = BlockPlanner.advance(start(weeks = 5), "2026-W03", now)
        assertEquals(b, BlockPlanner.advance(b, "2026-W02", now))
    }

    // ── Fatigue still speaks ───────────────────────────────────────────────────

    @Test
    fun realFatiguePullsTheDeloadForward() {
        // Week 3 of 6 with a fatigue score over the threshold: the schedule bends to the body.
        val b = walk(start(weeks = 6), 1)
        val early = BlockPlanner.advance(b, "2026-W04", now, fatigueScore = 6)
        assertEquals(BlockPhase.DELOAD.code, early.phase)
    }

    @Test
    fun fatigueCannotDeloadTheFirstWeeks() {
        // A block that deloads in week two isn't a block; the tripwire waits for real history.
        val early = BlockPlanner.advance(start(weeks = 6), "2026-W02", now, fatigueScore = 9)
        assertEquals(BlockPhase.ACCUMULATE.code, early.phase)
    }

    @Test
    fun quietFatigueLetsTheScheduleRun() {
        val b = walk(start(weeks = 6), 2, fatigue = 1)
        assertEquals(BlockPhase.ACCUMULATE.code, b.phase)
    }

    // ── Readouts ───────────────────────────────────────────────────────────────

    @Test
    fun weeksToDeloadCountsDown_andBottomsOutAtZero() {
        assertEquals(4, BlockPlanner.weeksToDeload(start(weeks = 5)))
        assertEquals(0, BlockPlanner.weeksToDeload(walk(start(weeks = 5), 4)))
    }

    @Test
    fun everyPhaseDescribesItselfInPlainWords() {
        var b = start(weeks = 5)
        val seen = mutableSetOf<String>()
        repeat(5) {
            seen += BlockPlanner.describe(b)
            b = BlockPlanner.advance(b, "2026-W%02d".format(it + 2), now)
        }
        // Five distinct lines from five weeks: the phase changes AND the deload countdown moves,
        // so no two weeks read identically.
        assertEquals("every week reads differently", 5, seen.size)
        assertTrue(seen.all { it.isNotBlank() && !it.contains("null") })
        assertTrue("the deload week says so", seen.any { it.contains("Deload week") })
        assertTrue("the countdown is stated", seen.any { it.contains("Deload in") })
    }

    @Test
    fun peakIsTheTestWeek() {
        val peak = walk(start(weeks = 5), 3)
        assertEquals(BlockPhase.PEAK.code, peak.phase)
        assertTrue(BlockPlanner.isTestWeek(peak))
        assertFalse(BlockPlanner.isTestWeek(start()))
    }

    @Test
    fun phasesCarryTheirOwnAmbition() {
        assertTrue(BlockPhase.DELOAD.progressionScale < 1.0)
        assertTrue(BlockPhase.PEAK.progressionScale > 1.0)
        assertTrue(BlockPhase.ACCUMULATE.volumeDelta > 0)
        assertTrue(BlockPhase.DELOAD.volumeDelta < 0)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        val b = start()
        assertEquals(
            BlockPlanner.advance(b, "2026-W02", now),
            BlockPlanner.advance(b, "2026-W02", now)
        )
    }

    // ── Entering the deload week is the moment the deload is served ───────────

    @Test
    fun steppingIntoTheDeloadWeekEntersDeload() {
        val peak = walk(start(weeks = 5), 3)
        assertEquals(BlockPhase.PEAK.code, peak.phase)
        val deload = BlockPlanner.advance(peak, "2026-W05", now)
        assertEquals(BlockPhase.DELOAD.code, deload.phase)
        assertTrue(BlockPlanner.entersDeload(peak, deload))
    }

    @Test
    fun aBuildingWeekOrAWeekAlreadyDeloadingEntersNothing() {
        val b = start(weeks = 5)
        assertFalse(BlockPlanner.entersDeload(b, BlockPlanner.advance(b, "2026-W02", now)))
        val deloading = walk(start(weeks = 5), 4)
        assertFalse(BlockPlanner.entersDeload(deloading, deloading))
    }

    @Test
    fun catchingUpPastTheDeloadWeekMissesIt() {
        // A long absence steps through the deload week and ends the block in one advance: the week
        // is gone, so generating a deload for it now would be a deload nobody scheduled.
        val peak = walk(start(weeks = 5), 3)
        val ended = BlockPlanner.advance(peak, "2026-W09", now)
        assertFalse(ended.isActive)
        assertFalse(BlockPlanner.entersDeload(peak, ended))
    }
}
