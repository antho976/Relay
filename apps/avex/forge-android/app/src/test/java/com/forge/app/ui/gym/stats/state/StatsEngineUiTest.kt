package com.forge.app.ui.gym.stats.state

import com.forge.app.domain.adapt.Confidence
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.Recommendation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Pure UI mappings of the engine read: pulse banding + plateau chip wording. */
class StatsEngineUiTest {

    private fun fatigue(score: Int) =
        DeloadAdvisor.FatigueAssessment(score = score, drivers = listOf("driver"))

    // Default deloadScoreThreshold is 5: FRESH < 3, BUILDING 3–4, DELOAD_SOON ≥ 5.
    @Test
    fun pulseBands_matchTheRecoveryInsightWindow() {
        assertEquals(PulseBand.FRESH, buildReadinessPulse(fatigue(0), 5).band)
        assertEquals(PulseBand.FRESH, buildReadinessPulse(fatigue(2), 5).band)
        assertEquals(PulseBand.BUILDING, buildReadinessPulse(fatigue(3), 5).band)
        assertEquals(PulseBand.BUILDING, buildReadinessPulse(fatigue(4), 5).band)
        assertEquals(PulseBand.DELOAD_SOON, buildReadinessPulse(fatigue(5), 5).band)
        assertEquals(PulseBand.DELOAD_SOON, buildReadinessPulse(fatigue(9), 5).band)
    }

    @Test
    fun pulseCarriesDriversVerbatim() {
        val pulse = buildReadinessPulse(DeloadAdvisor.FatigueAssessment(4, listOf("a", "b")), 5)
        assertEquals(listOf("a", "b"), pulse.drivers)
        assertEquals(4, pulse.score)
    }

    @Test
    fun plateauFlags_mapLadderRungsToChipWording() {
        val shift = Recommendation.RepRangeShift("x", "Bench", "8-12", "6-10", "why", Confidence.HIGH)
        assertEquals("SHIFT TO 6-10 REPS", plateauFlagOf(shift)!!.advice)

        val swap = Recommendation.VariationSwap("x", "Bench", listOf("alt"), "why", Confidence.HIGH)
        assertEquals("TRY A VARIATION", plateauFlagOf(swap)!!.advice)

        val reset = Recommendation.WeightChange("x", "Bench", 100.0, 90.0, -10.0, "90", "why", Confidence.MEDIUM)
        assertEquals("RESET & REBUILD", plateauFlagOf(reset)!!.advice)

        val nudge = Recommendation.WeightChange("x", "Bench", 100.0, 102.5, 2.5, "102.5", "why", Confidence.MEDIUM)
        assertEquals("PUSH A SMALL INCREASE", plateauFlagOf(nudge)!!.advice)
    }

    @Test
    fun nonPlateauRecommendations_mapToNull() {
        val deload = Recommendation.DeloadSuggestion(6, listOf("d"), "why", Confidence.HIGH)
        assertNull(plateauFlagOf(deload))
    }
}
