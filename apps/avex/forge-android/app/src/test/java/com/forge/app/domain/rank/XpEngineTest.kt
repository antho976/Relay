package com.forge.app.domain.rank

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class XpEngineTest {

    @Test fun `cold start is zero with no sources`() {
        val b = XpEngine.compute(XpSnapshot(0, 0, 0, 0.0, 0, 0))
        assertEquals(0L, b.total)
        assertTrue(b.sources.isEmpty())
    }

    @Test fun `each source contributes its weight`() {
        val b = XpEngine.compute(
            XpSnapshot(
                finishedSessions = 2,   // 200
                totalSets = 10,         // 40
                totalPrs = 1,           // 50
                totalVolumeLb = 4_000.0,// 20
                activeWeeks = 1,        // 40
                trophyPoints = 25       // 25
            )
        )
        assertEquals(200L + 40 + 50 + 20 + 40 + 25, b.total)
    }

    @Test fun `breakdown drops empty sources but keeps populated ones`() {
        val b = XpEngine.compute(XpSnapshot(1, 0, 0, 0.0, 0, 0))
        assertEquals(1, b.sources.size)
        assertEquals("Workouts", b.sources.first().label)
        assertEquals(XpEngine.WORKOUT_XP, b.total)
    }

    @Test fun `breakdown total equals sum of source xp`() {
        val b = XpEngine.compute(XpSnapshot(86, 1900, 23, 412_000.0, 20, 300))
        assertEquals(b.total, b.sources.sumOf { it.xp })
    }

    @Test fun `established lifter lands in a healthy mid-ladder rank`() {
        // ~85 sessions / ~400k lb / ~20 active weeks → expect Pulsar (not capped, not low).
        val b = XpEngine.compute(XpSnapshot(86, 1900, 23, 412_000.0, 20, 300))
        val rank = RankLadder.rankFor(b.total)
        assertEquals(RankTier.PULSAR, rank.tier)
    }

    @Test fun `deterministic`() {
        val s = XpSnapshot(5, 50, 3, 9_000.0, 3, 60)
        assertEquals(XpEngine.compute(s), XpEngine.compute(s))
    }
}
