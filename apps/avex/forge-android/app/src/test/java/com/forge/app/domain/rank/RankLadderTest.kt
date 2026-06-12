package com.forge.app.domain.rank

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RankLadderTest {

    @Test fun `zero xp is Ember I`() {
        val r = RankLadder.rankFor(0)
        assertEquals(RankTier.EMBER, r.tier)
        assertEquals(1, r.subRank)
        assertEquals("Ember I", r.displayName)
        assertEquals(0, r.globalIndex)
        assertFalse(r.isMax)
    }

    @Test fun `negative xp clamps to Ember I`() {
        assertEquals(0, RankLadder.rankFor(-5000).globalIndex)
    }

    @Test fun `tier floor lands on sub-rank I`() {
        // Entering each tier's floor XP = that tier, sub-rank I.
        RankTier.entries.forEach { t ->
            val r = RankLadder.rankFor(RankLadder.tierFloor(t))
            assertEquals(t, r.tier)
            assertEquals(1, r.subRank)
        }
    }

    @Test fun `roman numerals advance within a tier`() {
        // Iron spans 1500..6000 over 5 sub-ranks (900 each): 1500=I, 2400=II, 3300=III, 4200=IV, 5100=V.
        assertEquals("Iron I", RankLadder.rankFor(1_500).displayName)
        assertEquals("Iron II", RankLadder.rankFor(2_400).displayName)
        assertEquals("Iron IV", RankLadder.rankFor(4_200).displayName)
        assertEquals("Iron V", RankLadder.rankFor(5_100).displayName)
        assertEquals("Steel I", RankLadder.rankFor(6_000).displayName)
    }

    @Test fun `xpToNextTier counts down to the next tier floor`() {
        val r = RankLadder.rankFor(5_100) // Iron V, Steel at 6000
        assertEquals("Steel", r.nextTierName)
        assertEquals(900L, r.xpToNextTier)
    }

    @Test fun `progress within rank is bounded and monotonic`() {
        val a = RankLadder.rankFor(1_500) // Iron I floor
        val b = RankLadder.rankFor(1_950) // halfway to Iron II (1500..2400)
        assertEquals(0f, a.progressInRank, 0.001f)
        assertTrue(b.progressInRank in 0.4f..0.6f)
    }

    @Test fun `huge xp caps at Damascus V and reports max`() {
        val r = RankLadder.rankFor(10_000_000)
        assertEquals(RankTier.DAMASCUS, r.tier)
        assertEquals(5, r.subRank)
        assertEquals(RankLadder.maxGlobalIndex, r.globalIndex)
        assertTrue(r.isMax)
        assertEquals(1f, r.progressInRank, 0.001f)
        assertEquals(null, r.nextTierName)
        assertEquals(0L, r.xpToNextTier)
    }

    @Test fun `rank is monotonic in xp`() {
        var prev = -1
        for (xp in 0L..200_000L step 137L) {
            val g = RankLadder.rankFor(xp).globalIndex
            assertTrue("rank went backwards at $xp", g >= prev)
            prev = g
        }
    }

    @Test fun `30 ranks exist across 6 tiers`() {
        assertEquals(29, RankLadder.maxGlobalIndex)
        assertEquals(6, RankTier.entries.size)
    }

    @Test fun `deterministic`() {
        assertEquals(RankLadder.rankFor(20_500), RankLadder.rankFor(20_500))
    }
}
