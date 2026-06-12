package com.forge.app.domain.rank

/**
 * The forge-journey rank ladder: six major tiers (Ember → … → Damascus), each split into
 * five sub-ranks (I–V), for 30 lifetime ranks. Pure + deterministic — mirrors the
 * [com.forge.app.domain.trophy.TrophyEvaluator] / [com.forge.app.domain.adapt] style so it's
 * unit-testable in isolation. Lifetime XP (see [XpEngine]) is the only input; every threshold
 * lives in [RankLadder.TIER_FLOOR] so the whole economy is tuned in one place.
 */
enum class RankTier(val display: String, val colorArgb: Long) {
    EMBER("Ember", 0xFFB55A3C),       // dim ember orange — the spark
    IRON("Iron", 0xFF9CA3AF),         // raw grey iron
    STEEL("Steel", 0xFF7C97B5),       // refined steel blue-grey
    TEMPERED("Tempered", 0xFFC8893B), // heat-treated bronze
    FORGED("Forged", 0xFFE3B23C),     // finished gold
    DAMASCUS("Damascus", 0xFFD8DEE9)  // legendary folded-steel sheen (apex)
}

const val SUB_RANKS_PER_TIER = 5

/** Resolved rank for a given lifetime XP total. */
data class RankInfo(
    val tier: RankTier,
    val subRank: Int,          // 1..5
    val globalIndex: Int,      // 0..29 (Ember I = 0 … Damascus V = 29)
    val xpTotal: Long,
    val xpIntoRank: Long,      // XP earned past the current rank's floor
    val xpForRank: Long,       // span of the current rank (0 once maxed)
    val xpToNextTier: Long,    // XP remaining to reach the next tier (0 in Damascus)
    val nextTierName: String?, // null at Damascus
    val isMax: Boolean
) {
    val roman: String get() = ROMAN[subRank - 1]

    /** "Iron IV" — the user-facing rank label. */
    val displayName: String get() = "${tier.display} $roman"

    /** 0f..1f progress through the current sub-rank (1f when maxed). */
    val progressInRank: Float
        get() = if (isMax || xpForRank <= 0) 1f else (xpIntoRank.toFloat() / xpForRank).coerceIn(0f, 1f)

    companion object {
        private val ROMAN = listOf("I", "II", "III", "IV", "V")
    }
}

object RankLadder {

    /**
     * Cumulative XP required to ENTER sub-rank I of each tier. Calibrated so an established
     * lifter (~85 sessions, ~400k lb lifetime) lands in Tempered, leaving Forged + Damascus as
     * long-horizon goals. Tune the whole ladder here.
     */
    private val TIER_FLOOR = longArrayOf(
        0L,        // Ember I
        1_500L,    // Iron I
        6_000L,    // Steel I
        16_000L,   // Tempered I
        40_000L,   // Forged I
        90_000L    // Damascus I
    )

    /** Fixed XP step between Damascus sub-ranks — the top tier is open-ended. */
    private const val DAMASCUS_SUBSTEP = 14_000L

    val maxGlobalIndex = RankTier.entries.size * SUB_RANKS_PER_TIER - 1 // 29

    /** Cumulative XP floor for global rank [g] (0..29). */
    private fun floorFor(g: Int): Long {
        val tier = g / SUB_RANKS_PER_TIER
        val sub = g % SUB_RANKS_PER_TIER
        return if (tier < TIER_FLOOR.size - 1) {
            val base = TIER_FLOOR[tier]
            val span = TIER_FLOOR[tier + 1] - base
            base + span * sub / SUB_RANKS_PER_TIER
        } else {
            // Damascus: linear sub-steps above its floor.
            TIER_FLOOR.last() + DAMASCUS_SUBSTEP * sub
        }
    }

    /** XP needed to reach sub-rank I of tier [t] — used by the rank track UI. */
    fun tierFloor(t: RankTier): Long = TIER_FLOOR[t.ordinal]

    fun rankFor(xpTotal: Long): RankInfo {
        val xp = xpTotal.coerceAtLeast(0)
        // Highest global index whose floor ≤ xp.
        var g = 0
        for (i in 0..maxGlobalIndex) if (xp >= floorFor(i)) g = i else break

        val tier = RankTier.entries[g / SUB_RANKS_PER_TIER]
        val floor = floorFor(g)
        val isMax = g == maxGlobalIndex
        val xpForRank = if (isMax) 0L else floorFor(g + 1) - floor

        val nextTierIndex = g / SUB_RANKS_PER_TIER + 1
        val nextTierName = RankTier.entries.getOrNull(nextTierIndex)?.display
        val xpToNextTier = if (nextTierName == null) 0L else (TIER_FLOOR[nextTierIndex] - xp).coerceAtLeast(0)

        return RankInfo(
            tier = tier,
            subRank = g % SUB_RANKS_PER_TIER + 1,
            globalIndex = g,
            xpTotal = xp,
            xpIntoRank = xp - floor,
            xpForRank = xpForRank,
            xpToNextTier = xpToNextTier,
            nextTierName = nextTierName,
            isMax = isMax
        )
    }
}
