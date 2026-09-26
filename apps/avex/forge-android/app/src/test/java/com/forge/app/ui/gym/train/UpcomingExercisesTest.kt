package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.ui.gym.train.state.ExerciseUiState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Where MOVE TO NEXT goes, and when the card offers FINISH WORKOUT instead (audit 2026-09-26,
 * still open from gym-ui.md G1): the advance looked only at LATER, non-skipped exercises and ignored
 * completion, so jumping to the last slot and logging it finished the workout with earlier
 * exercises untouched, and MOVE TO NEXT could land on one already done.
 */
class UpcomingExercisesTest {

    private val base = com.forge.app.program.Program.days.first().exercises.first()

    private fun set(i: Int) = LoggedSet(
        loggedExerciseId = 1L, setIndex = i, weightText = "100", weightLb = 100.0, reps = 5, completedAt = 0L
    )

    /** A 2-set slot with [logged] sets done. */
    private fun ex(id: String, logged: Int = 0, skipped: Boolean = false) = ExerciseUiState(
        plan = base.copy(id = id, sets = 2),
        loggedSets = (0 until logged).map(::set),
        skipped = skipped
    )

    /** The advance target the card uses: null means FINISH WORKOUT. */
    private fun next(exercises: List<ExerciseUiState>, currentId: String) =
        upcomingExercises(exercises, currentId).firstOrNull()?.second?.plan?.id

    @Test
    fun `the last slot wraps to earlier incomplete exercises instead of finishing`() {
        val day = listOf(ex("a"), ex("b", logged = 2), ex("c"), ex("d", logged = 2))
        assertEquals("a", next(day, "d"))
        assertEquals(listOf("a", "c"), upcomingExercises(day, "d").map { it.second.plan.id })
    }

    @Test
    fun `later exercises come before earlier ones`() {
        val day = listOf(ex("a"), ex("b"), ex("c"), ex("d"))
        assertEquals(listOf("c", "d", "a"), upcomingExercises(day, "b").map { it.second.plan.id })
        assertEquals(listOf(2, 3, 0), upcomingExercises(day, "b").map { it.first })
    }

    @Test
    fun `finishes only once everything else is complete`() {
        val day = listOf(ex("a", logged = 2), ex("b", skipped = true), ex("c", logged = 2))
        assertNull(next(day, "c"))
        // The current exercise itself is never its own next, finished or not.
        assertNull(next(listOf(ex("a")), "a"))
    }

    @Test
    fun `complete and skipped slots are passed over`() {
        val day = listOf(ex("a"), ex("b", logged = 2), ex("c", skipped = true), ex("d"))
        assertEquals("d", next(day, "a"))
        assertEquals(listOf("d"), upcomingExercises(day, "a").map { it.second.plan.id })
    }
}
