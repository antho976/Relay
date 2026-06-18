package com.forge.app.domain.rank

/**
 * The cosmic/stellar rank ladder: six major tiers (Ember → … → Supernova), each split into
 * five sub-ranks (I–V), for 30 lifetime ranks. Pure + deterministic — mirrors the
 * [com.forge.app.domain.trophy.TrophyEvaluator] / [com.forge.app.domain.adapt] style so it's
 * unit-testable in isolation. Lifetime XP (see [XpEngine]) is the only input; every threshold
 * lives in [RankLadder.TIER_FLOOR] so the whole economy is tuned in one place.
 */
enum class RankTier(val display: String, val colorArgb: Long) {
    EMBER("Ember", 0xFFB55A3C),         // dim ember orange — the first spark
    FLARE("Flare", 0xFFE8743B),         // brighter solar-flare orange
    NOVA("Nova", 0xFFF2B43C),           // gold-white nova burst
    PULSAR("Pulsar", 0xFF6FB7E0),       // blue pulsar beam
    QUASAR("Quasar", 0xFF9C7BE8),       // violet quasar core
    SUPERNOVA("Supernova", 0xFFEAF2FF)  // brilliant white-blue detonation (apex)
}

const val SUB_RANKS_PER_TIER = 5

/** Resolved rank for a given lifetime XP total. */
data class RankInfo(
    val tier: RankTier,
    val subRank: Int,          // 1..5
    val globalIndex: Int,      // 0..29 (Ember I = 0 … Supernova V = 29)
    val xpTotal: Long,
    val xpIntoRank: Long,      // XP earned past the current rank's floor
    val xpForRank: Long,       // span of the current rank (0 once maxed)
    val xpToNextTier: Long,    // XP remaining to reach the next tier (0 in Supernova)
    val nextTierName: String?, // null at Supernova
    /** The immediate next sub-rank's label (e.g. "Flare V" or "Nova I"); null once maxed. */
    val nextRankName: String?,
    /** XP remaining to reach the next sub-rank (0 once maxed) — closer milestone than the next tier. */
    val xpToNextRank: Long,
    val isMax: Boolean
) {
    val roman: String get() = romanFor(subRank)

    /** "Flare IV" — the user-facing rank label. */
    val displayName: String get() = "${tier.display} $roman"

    /** 0f..1f progress through the current sub-rank (1f when maxed). */
    val progressInRank: Float
        get() = if (isMax || xpForRank <= 0) 1f else (xpIntoRank.toFloat() / xpForRank).coerceIn(0f, 1f)

    companion object {
        private val ROMAN = listOf("I", "II", "III", "IV", "V")
        /** Roman numeral for a 1-based sub-rank (1..5) — the single source for rank labels. */
        fun romanFor(subRank: Int): String = ROMAN[subRank - 1]
    }
}

object RankLadder {

    /**
     * Cumulative XP required to ENTER sub-rank I of each tier. Calibrated so an established
     * lifter (~85 sessions, ~400k lb lifetime) lands in Pulsar, leaving Quasar + Supernova as
     * long-horizon goals. Tune the whole ladder here.
     */
    private val TIER_FLOOR = longArrayOf(
        0L,        // Ember I
        1_500L,    // Flare I
        6_000L,    // Nova I
        16_000L,   // Pulsar I
        40_000L,   // Quasar I
        90_000L    // Supernova I
    )

    /** Fixed XP step between Supernova sub-ranks — the top tier is open-ended. */
    private const val SUPERNOVA_SUBSTEP = 14_000L

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
            // Supernova: linear sub-steps above its floor.
            TIER_FLOOR.last() + SUPERNOVA_SUBSTEP * sub
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

        // The immediate next sub-rank — a closer, more motivating milestone than the next tier.
        // Single source of the ladder structure so the UI never re-derives it.
        val nextRankName = if (isMax) null else {
            val ng = g + 1
            "${RankTier.entries[ng / SUB_RANKS_PER_TIER].display} ${RankInfo.romanFor(ng % SUB_RANKS_PER_TIER + 1)}"
        }
        val xpToNextRank = if (isMax) 0L else (floorFor(g + 1) - xp).coerceAtLeast(0)

        return RankInfo(
            tier = tier,
            subRank = g % SUB_RANKS_PER_TIER + 1,
            globalIndex = g,
            xpTotal = xp,
            xpIntoRank = xp - floor,
            xpForRank = xpForRank,
            xpToNextTier = xpToNextTier,
            nextTierName = nextTierName,
            nextRankName = nextRankName,
            xpToNextRank = xpToNextRank,
            isMax = isMax
        )
    }
}
