package com.forge.app.domain.engine

import kotlin.math.roundToInt

/**
 * The personal heart-rate zone model (Engine E-A).
 *
 * Zones are anchored to THIS athlete: their max heart rate and their resting heart rate, using
 * heart-rate reserve (Karvonen) rather than a percentage of max, because reserve accounts for the
 * fact that a fit person's zone 2 sits at a different absolute pulse than an unfit one's.
 *
 * **The age problem is real.** The app has never asked for an age, so this cannot silently assume
 * one. With neither an age nor a max-HR override, [maxHr] is null and every zone claim disappears
 * — surfaces show effort words instead of bpm bands. That is the sensor ladder's rung one, and it
 * is a complete experience, not a degraded one.
 */
data class ConditioningProfile(
    /** The user's explicit max-HR override, when they know it. Wins over any estimate. */
    val maxHrOverride: Int? = null,
    /** Age in years, for the Tanaka estimate. Null when the user hasn't said. */
    val ageYears: Int? = null,
    /** Resting HR from Health Connect, for heart-rate reserve. Null falls back to percent-of-max. */
    val restingHr: Int? = null
) {

    /** Max heart rate, or null when there is no honest way to know it. */
    val maxHr: Int?
        get() = maxHrOverride ?: ageYears?.let { tanaka(it) }

    /** True when this profile can make zone claims at all. */
    val hasZones: Boolean get() = maxHr != null

    /** The zone a heart rate falls in, or null when zones are unknowable. */
    fun zoneFor(bpm: Int): Int? {
        val max = maxHr ?: return null
        val rest = restingHr ?: 0
        val reserve = (max - rest).takeIf { it > 0 } ?: return null
        val fraction = (bpm - rest).toDouble() / reserve
        return when {
            fraction < Z1_TOP -> 1
            fraction < Z2_TOP -> 2
            fraction < Z3_TOP -> 3
            fraction < Z4_TOP -> 4
            else -> 5
        }
    }

    /** The bpm band for a zone, or null when zones are unknowable. */
    fun bandFor(zone: Int): IntRange? {
        val max = maxHr ?: return null
        val rest = restingHr ?: 0
        val reserve = (max - rest).takeIf { it > 0 } ?: return null
        fun bpm(fraction: Double) = (rest + reserve * fraction).roundToInt()
        return when (zone) {
            1 -> bpm(0.0)..bpm(Z1_TOP)
            2 -> bpm(Z1_TOP)..bpm(Z2_TOP)
            3 -> bpm(Z2_TOP)..bpm(Z3_TOP)
            4 -> bpm(Z3_TOP)..bpm(Z4_TOP)
            5 -> bpm(Z4_TOP)..max
            else -> null
        }
    }

    /**
     * Refine the max upward from an observed session peak — but only from a SUSTAINED reading.
     * A single wrist-HR spike is an artifact, and letting one shift every zone would quietly
     * recalibrate the athlete's whole conditioning model off a bad second of data.
     *
     * @param sustainedPeakBpm the highest bpm held for at least [SUSTAIN_SECONDS].
     */
    fun refinedWith(sustainedPeakBpm: Int, heldSeconds: Int): ConditioningProfile {
        if (heldSeconds < SUSTAIN_SECONDS) return this
        if (sustainedPeakBpm !in PLAUSIBLE_HR) return this
        val current = maxHr ?: return copy(maxHrOverride = sustainedPeakBpm)
        return if (sustainedPeakBpm > current) copy(maxHrOverride = sustainedPeakBpm) else this
    }

    companion object {
        /** Zone tops as fractions of heart-rate reserve. */
        const val Z1_TOP = 0.60
        const val Z2_TOP = 0.70
        const val Z3_TOP = 0.80
        const val Z4_TOP = 0.90

        /** A max-HR refinement needs this many seconds held, not one spike. */
        const val SUSTAIN_SECONDS = 30

        /** Readings outside this are sensor artifacts, not heart rates. */
        val PLAUSIBLE_HR = 25..240

        /**
         * Tanaka: 208 − 0.7 × age. Chosen over the old 220 − age because it fits the data better
         * across ages, particularly for older athletes.
         */
        fun tanaka(ageYears: Int): Int = (208 - 0.7 * ageYears).roundToInt()
    }
}

/**
 * How an athlete's own effort words map onto zones when no heart rate exists (rung one of the
 * sensor ladder). Deliberately coarse: this is a proxy, and pretending otherwise would be the
 * silent-wrong failure the Engine plan forbids.
 */
object EffortZones {
    /** "easy" ≈ zone 2, "moderate" ≈ zone 3, "hard" ≈ zone 4. */
    fun zoneForEffort(effortCode: String?): Int? = when (effortCode) {
        "easy" -> 2
        "moderate" -> 3
        "hard" -> 4
        else -> null
    }

    /** The talk test in one line, the thing that makes zone 2 checkable without a strap. */
    fun talkTestFor(zone: Int): String = when (zone) {
        1 -> "You could hold a conversation without noticing you were exercising."
        2 -> "You can speak in full sentences, but you would rather not sing."
        3 -> "Sentences are getting short."
        4 -> "A few words at a time."
        else -> "Talking is not happening."
    }
}
