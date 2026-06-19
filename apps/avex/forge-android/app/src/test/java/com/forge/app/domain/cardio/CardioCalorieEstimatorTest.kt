package com.forge.app.domain.cardio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CardioCalorieEstimatorTest {

    @Test fun `null for a rest day`() {
        assertNull(CardioCalorieEstimator.estimate(CardioType.REST, 30, null, 180.0))
    }

    @Test fun `null without bodyweight`() {
        assertNull(CardioCalorieEstimator.estimate(CardioType.RUN, 30, CardioEffort.MODERATE, null))
    }

    @Test fun `null for zero duration`() {
        assertNull(CardioCalorieEstimator.estimate(CardioType.RUN, 0, CardioEffort.MODERATE, 180.0))
    }

    @Test fun `a moderate run hour matches the MET formula`() {
        // 9.0 MET × (180 lb × 0.45359237) kg × 1 h = 734.8 → 735
        val kcal = CardioCalorieEstimator.estimate(CardioType.RUN, 60, CardioEffort.MODERATE, 180.0)!!
        assertEquals(735, kcal)
    }

    @Test fun `effort scales the burn`() {
        val easy = CardioCalorieEstimator.estimate(CardioType.CYCLE, 45, CardioEffort.EASY, 170.0)!!
        val moderate = CardioCalorieEstimator.estimate(CardioType.CYCLE, 45, CardioEffort.MODERATE, 170.0)!!
        val hard = CardioCalorieEstimator.estimate(CardioType.CYCLE, 45, CardioEffort.HARD, 170.0)!!
        assertTrue(easy < moderate)
        assertTrue(moderate < hard)
    }

    @Test fun `unrated effort counts as moderate`() {
        val unrated = CardioCalorieEstimator.estimate(CardioType.WALK, 40, null, 165.0)
        val moderate = CardioCalorieEstimator.estimate(CardioType.WALK, 40, CardioEffort.MODERATE, 165.0)
        assertEquals(moderate, unrated)
    }

    @Test fun `a higher-MET activity burns more than a lower one`() {
        val run = CardioCalorieEstimator.estimate(CardioType.RUN, 30, CardioEffort.MODERATE, 180.0)!!
        val yoga = CardioCalorieEstimator.estimate(CardioType.YOGA, 30, CardioEffort.MODERATE, 180.0)!!
        assertTrue(run > yoga)
    }
}
