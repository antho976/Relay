package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A1's proximity-to-failure model. The acceptance triad: it must read the NEW fields
 * (toFailure / setType / difficultyTag) when they're logged, must behave EXACTLY like v2's
 * inlined RPE rules when they aren't, and must resolve conflicts by a stated authority order.
 */
class EffortModelTest {

    private fun set(
        reps: Int = 8,
        rpe: Double? = null,
        toFailure: Boolean = false,
        setType: String? = null,
        difficultyTag: String? = null
    ) = LoggedSet(
        loggedExerciseId = 1,
        setIndex = 0,
        weightText = "100",
        weightLb = 100.0,
        reps = reps,
        completedAt = 0,
        rpe = rpe,
        toFailure = toFailure,
        setType = setType,
        difficultyTag = difficultyTag
    )

    // ── Backwards compatibility: nothing new logged ────────────────────────────

    @Test
    fun noSignals_allowsProgress_matchingV2() {
        val r = EffortModel.read(listOf(set()), prevEffort = null)
        assertTrue(r.roomToProgress)
        assertFalse(r.backOff)
        assertFalse(r.highEffort)
    }

    @Test
    fun rpeOnly_behavesLikeV2() {
        assertTrue(EffortModel.read(listOf(set(rpe = 7.0)), null).roomToProgress)
        assertFalse(EffortModel.read(listOf(set(rpe = 8.5)), null).roomToProgress)
        assertTrue(EffortModel.read(listOf(set(rpe = 9.5)), null).backOff)
        assertTrue(EffortModel.read(listOf(set(rpe = 9.0)), null).highEffort)
    }

    @Test
    fun exerciseRatingOnly_behavesLikeV2() {
        assertTrue(EffortModel.read(listOf(set()), EffortRating.JUST_RIGHT).roomToProgress)
        assertFalse(EffortModel.read(listOf(set()), EffortRating.HARD).roomToProgress)
        assertTrue(EffortModel.read(listOf(set()), EffortRating.BRUTAL).backOff)
        assertTrue(EffortModel.read(listOf(set()), EffortRating.HARD).highEffort)
    }

    // ── The new fields ─────────────────────────────────────────────────────────

    @Test
    fun setTakenToFailure_blocksProgressAndBacksOff_withoutAnyRpe() {
        val r = EffortModel.read(listOf(set(), set(toFailure = true)), prevEffort = null)
        assertFalse(r.roomToProgress)
        assertTrue(r.backOff)
        assertTrue(r.highEffort)
        assertEquals("last set taken to failure", r.backOffReason)
    }

    @Test
    fun pastFailureTechniques_readAsFailure() {
        for (type in listOf("drop", "myo", "rest_pause", "negative")) {
            val r = EffortModel.read(listOf(set(setType = type)), prevEffort = null)
            assertFalse("$type should block progress", r.roomToProgress)
            assertTrue("$type should back off", r.backOff)
        }
    }

    @Test
    fun ordinarySetTypes_areNotFailureSignals() {
        for (type in listOf("paused", "partial", "isometric", "cluster", "emom")) {
            val r = EffortModel.read(listOf(set(setType = type)), prevEffort = null)
            assertTrue("$type should stay neutral", r.roomToProgress)
            assertFalse("$type should not back off", r.backOff)
        }
    }

    @Test
    fun difficultyTagSpeaks_onlyWhenNoRpe() {
        assertFalse(EffortModel.read(listOf(set(difficultyTag = "hard")), null).roomToProgress)
        assertTrue(EffortModel.read(listOf(set(difficultyTag = "hard")), null).highEffort)
        assertTrue(EffortModel.read(listOf(set(difficultyTag = "easy")), EffortRating.HARD).roomToProgress)
        // RPE outranks the tag.
        assertTrue(EffortModel.read(listOf(set(rpe = 7.0, difficultyTag = "hard")), null).roomToProgress)
    }

    @Test
    fun mixedTags_doNotReadAsEasyThroughout() {
        val sets = listOf(set(difficultyTag = "easy"), set(difficultyTag = null))
        // Not every set was tagged easy, so the coarse rating decides — HARD blocks.
        assertFalse(EffortModel.read(sets, EffortRating.HARD).roomToProgress)
    }

    @Test
    fun warmupSets_neverSpeakForEffort() {
        val sets = listOf(set(rpe = 5.0, setType = EffortModel.SET_TYPE_WARMUP), set(rpe = 9.5))
        val r = EffortModel.read(sets, prevEffort = null)
        assertTrue("the working set's 9.5 must win", r.backOff)
        assertEquals(1, EffortModel.workingSets(sets).size)
    }

    @Test
    fun warmupTaggedToFailure_isIgnored() {
        val sets = listOf(set(toFailure = true, setType = EffortModel.SET_TYPE_WARMUP), set(rpe = 7.0))
        assertTrue(EffortModel.read(sets, prevEffort = null).roomToProgress)
    }

    // ── Conflicting signals ────────────────────────────────────────────────────

    @Test
    fun rpeOutranksFailureFlag_forTheBackOffSentence() {
        // Both logged: RPE is the finer instrument, so it owns the reason.
        val r = EffortModel.read(listOf(set(rpe = 9.5, toFailure = true)), prevEffort = null)
        assertTrue(r.backOff)
        assertEquals("last RPE hit 9.5", r.backOffReason)
    }

    @Test
    fun failureAtLowRpe_stillBlocksProgress() {
        // A user who logs "to failure" at RPE 8 is contradicting themselves; failure is the
        // safer read, so no progression is offered.
        val r = EffortModel.read(listOf(set(rpe = 8.0, toFailure = true)), prevEffort = null)
        assertFalse(r.roomToProgress)
        assertFalse("RPE 8 is below the brutal line, so no back-off", r.backOff)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        val sets = listOf(set(rpe = 9.0), set(toFailure = true, difficultyTag = "hard"))
        assertEquals(EffortModel.read(sets, EffortRating.HARD), EffortModel.read(sets, EffortRating.HARD))
    }
}
