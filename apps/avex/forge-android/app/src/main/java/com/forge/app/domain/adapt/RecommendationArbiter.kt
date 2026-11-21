package com.forge.app.domain.adapt

/**
 * Final pure pass over all advisors' output: resolves conflicts, dedupes, and ranks —
 * the engine never hands the UI two recommendations that argue with each other.
 *
 * Precedence rules:
 *  1. Muted ids (dismissed advice, fed from `advice_event` in Phase 4) are dropped first.
 *  2. An active [Recommendation.DeloadSuggestion] suppresses every load *increase*
 *     ([Recommendation.WeightChange] with positive delta, micro-loads included) — pushing
 *     weight up contradicts "you need a recovery week". Decreases (resets, back-offs)
 *     survive, as do rep-range shifts and swaps (program-structure advice, not load).
 *     Suppressed increases are dropped, not converted to "hold" chips — the deload
 *     suggestion itself is the explanation, and a hold chip is noise.
 *  3. Per exercise, one structural change at most: a RepRangeShift and a VariationSwap
 *     for the same exercise collapse to the higher confidence (tie → the rep shift, the
 *     less disruptive intervention).
 *
 * Ranking (deterministic, stable): system priority (deload → readiness → progression →
 * rest → ordering → insight), then confidence desc, then id asc.
 */
object RecommendationArbiter {

    private val SYSTEM_PRIORITY = listOf(
        AdviceSystem.DELOAD,
        AdviceSystem.READINESS,
        AdviceSystem.PROGRESSION,
        AdviceSystem.REST,
        AdviceSystem.ORDERING,
        AdviceSystem.INSIGHT
    )

    fun arbitrate(
        recommendations: List<Recommendation>,
        mutedIds: Set<String> = emptySet()
    ): List<Recommendation> {
        var recs = recommendations.filter { it.id !in mutedIds }

        val deloadActive = recs.any { it is Recommendation.DeloadSuggestion }
        if (deloadActive) {
            recs = recs.filterNot { it is Recommendation.WeightChange && it.deltaLb > 0 }
        }

        // Collapse competing structural suggestions per exercise.
        val structural = recs.filter { it is Recommendation.RepRangeShift || it is Recommendation.VariationSwap }
            .groupBy { exerciseIdOf(it) }
        val losers = structural.values
            .filter { it.size > 1 }
            .flatMap { group ->
                val winner = group.maxWith(
                    compareBy<Recommendation> { it.confidence.ordinal }
                        .thenBy { it is Recommendation.RepRangeShift }
                )
                group - winner
            }
            .toSet()
        recs = recs.filterNot { it in losers }

        return recs.sortedWith(
            compareBy<Recommendation> { SYSTEM_PRIORITY.indexOf(it.system) }
                .thenByDescending { it.confidence.ordinal }
                .thenBy { it.id }
        )
    }

    private fun exerciseIdOf(rec: Recommendation): String = when (rec) {
        is Recommendation.WeightChange -> rec.exerciseId
        is Recommendation.RepRangeShift -> rec.exerciseId
        is Recommendation.VariationSwap -> rec.exerciseId
        else -> rec.id
    }
}
