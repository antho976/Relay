package com.forge.app.domain.adapt

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RecommendationArbiterTest {

    private fun weight(exerciseId: String, deltaLb: Double, confidence: Confidence = Confidence.MEDIUM) =
        Recommendation.WeightChange(
            exerciseId = exerciseId, exerciseName = exerciseId, previousMaxLb = 45.0,
            targetWeightLb = 45.0 + deltaLb, deltaLb = deltaLb, inputText = "${45.0 + deltaLb}",
            reason = "test", confidence = confidence
        )

    private fun repShift(exerciseId: String, confidence: Confidence = Confidence.MEDIUM) =
        Recommendation.RepRangeShift(exerciseId, exerciseId, "8-10", "12-15", "test", confidence)

    private fun swap(exerciseId: String, confidence: Confidence = Confidence.MEDIUM) =
        Recommendation.VariationSwap(exerciseId, exerciseId, listOf("alt"), "test", confidence)

    private fun deload(confidence: Confidence = Confidence.HIGH) =
        Recommendation.DeloadSuggestion(7, listOf("driver"), "fatigue", confidence)

    private fun readiness() = Recommendation.ReadinessScale(-3, "rough day", Confidence.MEDIUM)

    @Test
    fun mutedIdsAreDroppedFirst() {
        val out = RecommendationArbiter.arbitrate(
            listOf(swap("ua1"), repShift("ua2")),
            mutedIds = setOf("progression.swap.ua1")
        )
        assertEquals(listOf("progression.reprange.ua2"), out.map { it.id })
    }

    @Test
    fun deloadSuppressesIncreasesButKeepsBackoffs() {
        val out = RecommendationArbiter.arbitrate(
            listOf(weight("ua1", +2.5), weight("ua2", -5.0), deload())
        )
        assertTrue(out.any { it is Recommendation.DeloadSuggestion })
        assertTrue("the +2.5 increase contradicts a deload call", out.none { it is Recommendation.WeightChange && it.deltaLb > 0 })
        assertTrue("the back-off agrees with a deload call", out.any { it is Recommendation.WeightChange && it.deltaLb < 0 })
    }

    @Test
    fun withoutADeload_increasesSurvive() {
        val out = RecommendationArbiter.arbitrate(listOf(weight("ua1", +2.5)))
        assertEquals(1, out.size)
    }

    @Test
    fun competingStructuralAdvice_higherConfidenceWins() {
        val out = RecommendationArbiter.arbitrate(listOf(repShift("ua1", Confidence.MEDIUM), swap("ua1", Confidence.HIGH)))
        assertEquals(listOf("progression.swap.ua1"), out.map { it.id })
    }

    @Test
    fun structuralTie_prefersTheLessDisruptiveRepShift() {
        val out = RecommendationArbiter.arbitrate(listOf(swap("ua1"), repShift("ua1")))
        assertEquals(listOf("progression.reprange.ua1"), out.map { it.id })
    }

    @Test
    fun structuralAdviceOnDifferentExercises_bothSurvive() {
        val out = RecommendationArbiter.arbitrate(listOf(repShift("ua1"), swap("ua2")))
        assertEquals(2, out.size)
    }

    @Test
    fun rankingIsSystemPriorityThenConfidenceThenId() {
        val out = RecommendationArbiter.arbitrate(
            listOf(
                weight("ua2", -2.5, Confidence.MEDIUM),
                repShift("ua1", Confidence.HIGH),
                readiness(),
                deload()
            )
        )
        assertEquals(
            listOf("deload.suggest", "readiness.scale", "progression.reprange.ua1", "progression.weight.ua2"),
            out.map { it.id }
        )
    }

    @Test
    fun arbitrationIsDeterministic() {
        val input = listOf(weight("ua1", +2.5), repShift("ua2"), readiness(), deload())
        assertEquals(RecommendationArbiter.arbitrate(input), RecommendationArbiter.arbitrate(input))
    }
}
