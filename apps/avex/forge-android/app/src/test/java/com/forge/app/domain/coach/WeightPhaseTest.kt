package com.forge.app.domain.coach

import com.forge.app.data.db.entities.BodyweightEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A2's bodyweight trend read. Gates first: an unsupported claim is worse than no claim. */
class WeightPhaseTest {

    private val day = 24L * 60 * 60 * 1000

    /** [count] weigh-ins, one every [everyDays] days, drifting [perWeek] lb per week. */
    private fun series(
        count: Int,
        startLb: Double = 180.0,
        perWeek: Double = 0.0,
        everyDays: Int = 3,
        noiseLb: Double = 0.0
    ): List<BodyweightEntry> = (0 until count).map { i ->
        val days = i * everyDays
        val drift = perWeek * days / 7.0
        // Deterministic saw-tooth "noise" — real weigh-ins bounce; the trend must survive it.
        val noise = if (i % 2 == 0) noiseLb else -noiseLb
        BodyweightEntry(
            dateKey = "2026-01-%02d".format((i % 28) + 1),
            weightLb = startLb + drift + noise,
            recordedAt = days * day
        )
    }

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun noEntries_isUnknown() {
        assertEquals(WeightPhase.UNKNOWN, WeightPhase.of(emptyList()))
        assertTrue(!WeightPhase.UNKNOWN.isKnown)
    }

    @Test
    fun tooFewEntries_isUnknown() {
        assertEquals(WeightPhase.UNKNOWN, WeightPhase.of(series(count = 5, perWeek = -1.5)))
    }

    @Test
    fun enoughEntriesButTooShortASpan_isUnknown() {
        // Six weigh-ins crammed into a week say nothing about a trend.
        assertEquals(WeightPhase.UNKNOWN, WeightPhase.of(series(count = 6, perWeek = -2.0, everyDays = 1)))
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun steadyLoss_readsAsCut() {
        assertEquals(WeightPhase.CUT, WeightPhase.of(series(count = 8, perWeek = -1.0)))
    }

    @Test
    fun steadyGain_readsAsBulk() {
        assertEquals(WeightPhase.BULK, WeightPhase.of(series(count = 8, perWeek = 0.8)))
    }

    @Test
    fun flatWeight_readsAsMaintain() {
        assertEquals(WeightPhase.MAINTAIN, WeightPhase.of(series(count = 8, perWeek = 0.0)))
    }

    @Test
    fun driftBelowTheRate_readsAsMaintain() {
        // −0.2 lb/wk is water and dinner, not a deficit.
        assertEquals(WeightPhase.MAINTAIN, WeightPhase.of(series(count = 10, perWeek = -0.2)))
    }

    // ── Conflicting signals ────────────────────────────────────────────────────

    @Test
    fun dailyNoiseDoesNotFlipTheTrend() {
        // ±2 lb of day-to-day bounce on top of a real 1 lb/wk loss.
        assertEquals(WeightPhase.CUT, WeightPhase.of(series(count = 12, perWeek = -1.0, noiseLb = 2.0)))
    }

    @Test
    fun smoothedLatest_ignoresOneHeavyMorning() {
        val steady = series(count = 8, perWeek = 0.0)
        val spiked = steady + BodyweightEntry(dateKey = "2026-02-01", weightLb = 195.0, recordedAt = 40 * day)
        val smoothed = WeightPhase.smoothedLatest(spiked)!!
        assertTrue("a 15 lb spike must not move the reading far", smoothed < 183.0)
    }

    @Test
    fun ratePerWeek_isNullBelowThreePoints() {
        assertNull(WeightPhase.ratePerWeek(series(count = 2)))
    }

    @Test
    fun ratePerWeek_recoversTheSlope() {
        val rate = WeightPhase.ratePerWeek(series(count = 10, perWeek = -1.0))!!
        assertEquals(-1.0, rate, 0.05)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        val s = series(count = 9, perWeek = -1.0, noiseLb = 1.0)
        assertEquals(WeightPhase.of(s), WeightPhase.of(s))
        assertEquals(WeightPhase.of(s), WeightPhase.of(s.shuffled()))
    }
}
