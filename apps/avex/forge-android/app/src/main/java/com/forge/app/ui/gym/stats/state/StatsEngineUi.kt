package com.forge.app.ui.gym.stats.state

import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.RatioCounts
import com.forge.app.domain.adapt.Recommendation

/**
 * UI projections of the adaptation engine's Stats read (AdaptationRepository.engineStatsRead).
 * The mapping functions are pure so the banding/wording rules are unit-testable without
 * Compose or the repository.
 */

/** The fatigue pulse band shown on Snapshot. Bands mirror DeloadAdvisor's score thresholds. */
enum class PulseBand(val label: String) {
    FRESH("Fresh"),
    BUILDING("Building"),
    DELOAD_SOON("Deload soon")
}

/** Snapshot's readiness pulse: band + the engine's verbatim drivers as the "why" line. */
data class ReadinessPulse(
    val band: PulseBand,
    val score: Int,
    val drivers: List<String>
)

/** One plateaued lift + the ladder's short prescription, rendered as a chip on Strength. */
data class PlateauFlagUi(
    val exerciseId: String,
    val exerciseName: String,
    /** Small-caps chip text, e.g. "SHIFT TO 8-10 REPS". */
    val advice: String,
    /** The engine's full reason — the italic line under the chip. */
    val detail: String
)

/** Always-on structural balance bar (push/pull, quad/ham) for the Volume tab. */
data class BalanceRatioUi(
    val title: String,
    val labelA: String,
    val labelB: String,
    val setsA: Int,
    val setsB: Int,
    val healthyLow: Double,
    val healthyHigh: Double
) {
    val total: Int get() = setsA + setsB
    val ratio: Double? get() = if (setsA > 0 && setsB > 0) setsA.toDouble() / setsB else null
    val balanced: Boolean? get() = ratio?.let { it in healthyLow..healthyHigh }
}

/**
 * Score → band. The BUILDING window is [threshold-2, threshold) — the same sub-threshold
 * range InsightEngine's "recovery signals building" insight uses, so the pulse and the
 * insight can never disagree.
 */
fun buildReadinessPulse(f: DeloadAdvisor.FatigueAssessment, threshold: Int): ReadinessPulse {
    val band = when {
        f.score >= threshold -> PulseBand.DELOAD_SOON
        f.score >= threshold - 2 -> PulseBand.BUILDING
        else -> PulseBand.FRESH
    }
    return ReadinessPulse(band = band, score = f.score, drivers = f.drivers)
}

/** Ladder recommendation → chip wording. Non-plateau recommendation kinds map to null. */
fun plateauFlagOf(rec: Recommendation): PlateauFlagUi? = when (rec) {
    is Recommendation.RepRangeShift ->
        PlateauFlagUi(rec.exerciseId, rec.exerciseName, "SHIFT TO ${rec.toReps} REPS", rec.reason)
    is Recommendation.VariationSwap ->
        PlateauFlagUi(rec.exerciseId, rec.exerciseName, "TRY A VARIATION", rec.reason)
    is Recommendation.WeightChange -> PlateauFlagUi(
        rec.exerciseId, rec.exerciseName,
        if (rec.deltaLb < 0) "RESET & REBUILD" else "PUSH A SMALL INCREASE",
        rec.reason
    )
    else -> null
}

fun balanceRatioUi(rc: RatioCounts): BalanceRatioUi = BalanceRatioUi(
    title = when (rc.key) {
        "pushpull" -> "Push / Pull"
        "quadham" -> "Quad / Ham"
        else -> "${rc.labelA} / ${rc.labelB}"
    },
    labelA = rc.labelA,
    labelB = rc.labelB,
    setsA = rc.setsA,
    setsB = rc.setsB,
    healthyLow = rc.healthyLow,
    healthyHigh = rc.healthyHigh
)
