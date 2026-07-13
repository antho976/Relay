package com.forge.app.domain.rank

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class StandingEngineTest {

    @Test fun `emits one metric per dimension`() {
        val m = StandingEngine.standings(StandingSnapshot(3.0, 8, 25_000.0), com.forge.app.domain.units.WeightUnit.LB)
        assertEquals(listOf("consistency", "streak", "volume"), m.map { it.key })
    }

    @Test fun `more is better - top percent decreases as the metric rises`() {
        val low = StandingEngine.standings(StandingSnapshot(1.0, 1, 5_000.0), com.forge.app.domain.units.WeightUnit.LB)
        val high = StandingEngine.standings(StandingSnapshot(6.0, 40, 110_000.0), com.forge.app.domain.units.WeightUnit.LB)
        low.zip(high).forEach { (l, h) ->
            assertTrue("${l.key} should rank better when higher", h.topPercent <= l.topPercent)
        }
    }

    @Test fun `percentile is interpolated between anchors`() {
        // Consistency 3.0/wk anchor = top 30; halfway to 4.0 (top 15) ≈ top 22.
        val m = StandingEngine.standings(StandingSnapshot(3.5, 0, 0.0), com.forge.app.domain.units.WeightUnit.LB).first { it.key == "consistency" }
        assertTrue(m.topPercent in 20..25)
    }

    @Test fun `percentile stays within the 2 to 99 band`() {
        StandingEngine.standings(StandingSnapshot(0.0, 0, 0.0), com.forge.app.domain.units.WeightUnit.LB).forEach {
            assertTrue(it.topPercent in 2..99)
        }
        StandingEngine.standings(StandingSnapshot(99.0, 999, 9_999_999.0), com.forge.app.domain.units.WeightUnit.LB).forEach {
            assertTrue(it.topPercent in 2..99)
        }
    }

    @Test fun `value text formats cleanly`() {
        val m = StandingEngine.standings(StandingSnapshot(3.0, 8, 25_000.0), com.forge.app.domain.units.WeightUnit.LB)
        assertEquals("3×/wk", m[0].valueText)   // whole number, no decimal
        assertEquals("8 wk", m[1].valueText)
        assertEquals("25k lb", m[2].valueText)
    }

    @Test fun `deterministic`() {
        val s = StandingSnapshot(3.2, 8, 25_000.0)
        assertEquals(StandingEngine.standings(s, com.forge.app.domain.units.WeightUnit.LB), StandingEngine.standings(s, com.forge.app.domain.units.WeightUnit.LB))
    }

    @Test fun `a NaN metric reads as the worst rank, never the best`() {
        // A corrupt session that persisted a NaN volume must not render a nonsensical "TOP 2%" — NaN
        // compares false against every anchor range, so without the guard it falls through to the best.
        val m = StandingEngine.standings(StandingSnapshot(Double.NaN, 0, Double.NaN), com.forge.app.domain.units.WeightUnit.LB)
        val consistency = m.first { it.key == "consistency" }
        val volume = m.first { it.key == "volume" }
        assertEquals("NaN consistency → bottom anchor (top 95%)", 95, consistency.topPercent)
        assertEquals("NaN volume → bottom anchor (top 95%)", 95, volume.topPercent)
    }
}
