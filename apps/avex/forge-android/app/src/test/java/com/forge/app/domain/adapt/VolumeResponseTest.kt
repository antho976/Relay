package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The shared volume-response model (H-06). Strength change must be read WITHIN a lift and only
 * then aggregated by muscle-week; the exercise mix of a week is never a strength signal.
 * Fixtures: UTC, one bout per lift per ISO week, weeks 7 days apart so every bout lands in its
 * own Monday bucket.
 */
class VolumeResponseTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 200 * day

    private fun sets(weight: Double, n: Int) = List(n) { i ->
        LoggedSet(
            loggedExerciseId = 1, setIndex = i, weightText = "$weight",
            weightLb = weight, reps = 10, completedAt = 0
        )
    }

    /** One bout on ISO week [week] (0-based) — [weight] lb for [setCount] sets of 10. */
    private fun bout(week: Int, weight: Double, setCount: Int) = ExerciseBout(
        sessionStartedAt = (30 + week * 7) * day, effort = null, hitFullTarget = true,
        skipped = false, swappedName = null, sets = sets(weight, setCount)
    )

    private fun slot(id: String, muscle: MuscleGroup = MuscleGroup.CHEST) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(history: Map<String, List<ExerciseBout>>) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("upper-a", "Upper A", listOf(slot("bench"), slot("fly")))),
        sessions = emptyList(), exerciseHistory = history, prefs = PrefsSnap()
    )

    private fun analyse(history: Map<String, List<ExerciseBout>>) =
        VolumeResponse.analyse(snapshot(history), minWeeks = 8, minPerTier = 3)[MuscleGroup.CHEST]

    // ── The audit scenario: exercise mix is not physiology ─────────────────────

    @Test
    fun mixedLiftWeeks_withNeitherLiftMoving_readAsZeroResponse() {
        // High-volume weeks: flat 300 lb bench (4 sets) + flat 50 lb fly (3 sets) = 7 sets.
        // Low-volume weeks: the same flat fly alone = 3 sets. Alternating, eight weeks.
        val bench = (0 until 8 step 2).map { bout(it, 300.0, 4) }
        val fly = (0 until 8).map { bout(it, 50.0, 3) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        // The bench is only ever in one of any two consecutive weeks, so it is never compared; the
        // fly is compared with itself and hasn't moved. Both tiers read exactly zero.
        assertEquals(0.0, r!!.highAvgPct, 1e-9)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
        assertEquals(0.0, r.gapPct, 1e-9)
        assertEquals(4, r.highWeeks)
        assertEquals(3, r.lowWeeks)
        assertEquals(5, r.splitSets)
    }

    @Test
    fun reversedLiftMix_withNeitherLiftMoving_readsAsZeroResponse() {
        // Now the heavy lift sits in the LOW-volume weeks: 8 sets of fly alone vs 2 bench + 1 fly.
        // The old max-e1RM read swung the other way here and raised the cap; this reads zero too.
        val fly = (0 until 8).map { bout(it, 50.0, if (it % 2 == 0) 8 else 1) }
        val bench = (1 until 8 step 2).map { bout(it, 300.0, 2) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        assertEquals(0.0, r!!.gapPct, 1e-9)
    }

    @Test
    fun liftsThatNeverShareAWeek_areNeverCompared() {
        // Bench on even weeks, fly on odd weeks, forever. No lift is ever read in two consecutive
        // weeks, so there is no comparable change at all — and no response, however many weeks.
        val bench = (0 until 12 step 2).map { bout(it, 300.0, 4) }
        val fly = (1 until 12 step 2).map { bout(it, 50.0, 4) }
        assertNull(analyse(mapOf("bench" to bench, "fly" to fly)))
    }

    // ── A genuine within-lift change is still read ─────────────────────────────

    @Test
    fun withinLiftGain_afterHighVolumeWeeks_isAPositiveResponse() {
        // One lift, 8 sets on even weeks and 4 on odd; each high week is followed by ~+9%, each
        // low week by a flat week.
        val plan = listOf(8 to 50.0, 4 to 55.0, 8 to 55.0, 4 to 60.0, 8 to 60.0, 4 to 65.0, 8 to 65.0, 4 to 70.0, 8 to 70.0)
        val bench = plan.mapIndexed { i, (sets, w) -> bout(i, w, sets) }
        val r = analyse(mapOf("bench" to bench))
        assertNotNull(r)
        assertTrue(r!!.highAvgPct > 7.0)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
        assertTrue(r.gapPct > 7.0)
    }

    @Test
    fun withinLiftGain_afterLowVolumeWeeks_isANegativeResponse() {
        val plan = listOf(4 to 50.0, 8 to 55.0, 4 to 55.0, 8 to 60.0, 4 to 60.0, 8 to 65.0, 4 to 65.0, 8 to 70.0, 4 to 70.0)
        val bench = plan.mapIndexed { i, (sets, w) -> bout(i, w, sets) }
        val r = analyse(mapOf("bench" to bench))
        assertNotNull(r)
        assertTrue(r!!.gapPct < -7.0)
    }

    @Test
    fun changeIsNormalisedPerLift_soAHeavyLiftDoesNotOutweighALightOne() {
        // Both lifts present every week. Bench flat at 300; fly +10% after every high week. The
        // response is the mean of per-lift PERCENT changes, so the fly's move counts at its own scale
        // (10%) averaged with the bench's zero — not drowned by 300 lb of raw e1RM.
        val bench = (0 until 9).map { bout(it, 300.0, if (it % 2 == 0) 5 else 2) }
        val flyWeights = listOf(50.0, 55.0, 55.0, 60.5, 60.5, 66.55, 66.55, 73.205, 73.205)
        val fly = flyWeights.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 3 else 1) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        assertEquals(5.0, r!!.highAvgPct, 1e-6)   // mean of (0%, +10%)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
    }

    // ── Gates ──────────────────────────────────────────────────────────────────

    @Test
    fun belowTheWeekGate_saysNothing() {
        val bench = (0 until 7).map { bout(it, 50.0 + it * 5, if (it % 2 == 0) 8 else 4) }
        assertNull(analyse(mapOf("bench" to bench)))
    }

    @Test
    fun aLiftOutsideTheProgram_isIgnored() {
        val bench = (0 until 9).map { bout(it, 50.0 + it * 5, if (it % 2 == 0) 8 else 4) }
        assertTrue(VolumeResponse.analyse(snapshot(mapOf("not-in-program" to bench)), 8, 3).isEmpty())
    }
}
