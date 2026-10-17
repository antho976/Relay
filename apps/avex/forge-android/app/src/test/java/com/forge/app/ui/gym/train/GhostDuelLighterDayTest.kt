package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.gym.train.state.ExerciseUiState
import org.junit.Assert.assertEquals
import org.junit.Test

/** A lift the coach asked to go lighter on today stays out of the "vs last session" duel. */
class GhostDuelLighterDayTest {

    private val base = com.forge.app.program.Program.days.first().exercises.first()

    private fun set(lb: Double, reps: Int) = LoggedSet(
        loggedExerciseId = 1L, setIndex = 0, weightText = lb.toString(), weightLb = lb, reps = reps, completedAt = 0L
    )

    private fun ex(id: String, deltaLb: Double?, today: LoggedSet, last: LoggedSet) = ExerciseUiState(
        plan = base.copy(id = id, sets = 1),
        loggedSets = listOf(today),
        priorSets = listOf(last),
        suggestedDeltaLb = deltaLb
    )

    @Test
    fun `a lighter lift leaves the duel and a held one stays in it`() {
        val state = DayUiState(dayPlan = com.forge.app.program.Program.days.first(), displayName = "Upper A", exercises = listOf(
            // Asked for 0.7 lb less and did it: not a loss to last session.
            ex("pulldown", deltaLb = -0.7, today = set(32.5, 10), last = set(33.2, 10)),
            // Asked to hold, and beat it by a rep.
            ex("press", deltaLb = 0.0, today = set(28.6, 9), last = set(28.6, 8)),
        ))
        assertEquals(1, state.ghostComparable)
        assertEquals(1, state.ghostBeats)
    }

    @Test
    fun `rounding noise under half a pound still counts as chasing`() {
        val state = DayUiState(dayPlan = com.forge.app.program.Program.days.first(), displayName = "Upper A", exercises = listOf(
            ex("press", deltaLb = -0.3, today = set(28.6, 8), last = set(28.6, 8)),
        ))
        assertEquals(1, state.ghostComparable)
    }
}
