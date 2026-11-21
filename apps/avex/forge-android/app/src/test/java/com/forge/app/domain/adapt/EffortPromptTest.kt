package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EffortPromptTest {

    private fun set(
        index: Int,
        rpe: Double? = null,
        toFailure: Boolean = false,
        tag: String? = null,
        setType: String? = null
    ) = LoggedSet(
        id = index.toLong(), loggedExerciseId = 1, setIndex = index, weightText = "100", weightLb = 100.0,
        reps = 8, completedAt = index * 1000L, difficultyTag = tag, toFailure = toFailure, setType = setType, rpe = rpe
    )

    @Test
    fun `reps left is stored as the RPE it equals`() {
        assertEquals(10.0, EffortPrompt.rpeFor(0), 0.0)
        assertEquals(8.0, EffortPrompt.rpeFor(2), 0.0)
        assertEquals(6.0, EffortPrompt.rpeFor(4), 0.0)
        // The open end is one answer however far past four it is asked for.
        assertEquals(6.0, EffortPrompt.rpeFor(9), 0.0)
    }

    @Test
    fun `an RPE reads back as its reps left, half steps included`() {
        assertEquals(0, EffortPrompt.repsLeftFor(10.0))
        assertEquals(2, EffortPrompt.repsLeftFor(8.0))
        assertEquals(4, EffortPrompt.repsLeftFor(6.0))
        assertEquals(4, EffortPrompt.repsLeftFor(3.0))
        assertEquals(2, EffortPrompt.repsLeftFor(8.5))
        assertNull(EffortPrompt.repsLeftFor(null))
    }

    @Test
    fun `labels name the open end`() {
        assertEquals(listOf("0", "1", "2", "3", "4+"), EffortPrompt.CHOICES.map { EffortPrompt.label(it) })
    }

    @Test
    fun `last-set mode waits for the planned sets`() {
        val two = listOf(set(0), set(1))
        assertNull(EffortPrompt.setToAsk(two, targetSets = 3, mode = EffortPromptMode.LAST_SET))
        val three = two + set(2)
        assertEquals(2L, EffortPrompt.setToAsk(three, targetSets = 3, mode = EffortPromptMode.LAST_SET)?.id)
        // A bonus set past the plan is the new last set, and gets asked about in its turn.
        val four = three + set(3)
        assertEquals(3L, EffortPrompt.setToAsk(four, targetSets = 3, mode = EffortPromptMode.LAST_SET)?.id)
    }

    @Test
    fun `warm-ups do not count toward the planned sets and are never asked about`() {
        val sets = listOf(set(0, setType = "warmup"), set(1), set(2))
        assertNull(EffortPrompt.setToAsk(sets, targetSets = 3, mode = EffortPromptMode.LAST_SET))
        assertNull(EffortPrompt.setToAsk(listOf(set(0, setType = "warmup")), 1, EffortPromptMode.EVERY_SET))
    }

    @Test
    fun `every-set mode asks after each working set`() {
        assertEquals(0L, EffortPrompt.setToAsk(listOf(set(0)), targetSets = 3, mode = EffortPromptMode.EVERY_SET)?.id)
    }

    @Test
    fun `a set that already answered is not asked again`() {
        assertNull(EffortPrompt.setToAsk(listOf(set(0, rpe = 8.0)), 1, EffortPromptMode.EVERY_SET))
        assertNull(EffortPrompt.setToAsk(listOf(set(0, toFailure = true)), 1, EffortPromptMode.EVERY_SET))
        assertNull(EffortPrompt.setToAsk(listOf(set(0, tag = "hard")), 1, EffortPromptMode.EVERY_SET))
        assertNull(EffortPrompt.setToAsk(listOf(set(0, setType = "drop")), 1, EffortPromptMode.EVERY_SET))
    }

    @Test
    fun `off never asks, and nothing logged is nothing to ask`() {
        assertNull(EffortPrompt.setToAsk(listOf(set(0)), 1, EffortPromptMode.OFF))
        assertNull(EffortPrompt.setToAsk(emptyList(), 1, EffortPromptMode.EVERY_SET))
    }

    @Test
    fun `an exercise with no effort signal offers its last working set`() {
        val sets = listOf(set(0, setType = "warmup"), set(1), set(2))
        assertEquals(2L, EffortPrompt.unratedSet(sets, exerciseRating = null)?.id)
    }

    @Test
    fun `an exercise that already says how hard it was is left alone`() {
        assertNull(EffortPrompt.unratedSet(listOf(set(0), set(1, rpe = 9.0)), null))
        assertNull(EffortPrompt.unratedSet(listOf(set(0), set(1)), EffortRating.HARD))
        assertNull(EffortPrompt.unratedSet(listOf(set(0, toFailure = true)), null))
        // Warm-ups alone are not work to rate.
        assertNull(EffortPrompt.unratedSet(listOf(set(0, setType = "warmup")), null))
    }

    @Test
    fun `an unknown stored mode falls back to the default`() {
        assertEquals(EffortPromptMode.LAST_SET, EffortPromptMode.fromCode(null))
        assertEquals(EffortPromptMode.LAST_SET, EffortPromptMode.fromCode("nonsense"))
        assertEquals(EffortPromptMode.EVERY_SET, EffortPromptMode.fromCode("every"))
    }
}
