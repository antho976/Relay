package com.forge.app.ui.gym.train

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.gym.train.state.ExerciseUiState
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What "unsaved work" means when backing out of a live session.
 *
 * It meant logged sets, and only logged sets. Everything else the session screen lets you write —
 * the journal, an exercise note, an effort rating, a deliberate skip — is stored against the session
 * and CASCADE-deleted with it, so leaving a workout you had annotated but not yet logged a set into
 * discarded all of it with no prompt. Writing a note about why the gym was too busy to train and
 * then leaving is exactly that shape, and the app's answer was to delete the note.
 */
class UnsavedWorkTest {

    /** A real plan from the catalogue — cheaper and truer than inventing one. */
    private val dayPlan = com.forge.app.program.Program.days.first()
    private fun plan() = dayPlan.exercises.first()

    private fun exercise(
        note: String? = null,
        difficulty: EffortRating? = null,
        skipped: Boolean = false,
        sets: List<LoggedSet> = emptyList()
    ) = ExerciseUiState(plan = plan(), loggedSets = sets, note = note, difficulty = difficulty, skipped = skipped)

    private fun state(
        journal: String = "",
        exercises: List<ExerciseUiState> = listOf(exercise())
    ) = DayUiState(
        dayPlan = dayPlan,
        displayName = dayPlan.defaultName,
        sessionJournal = journal,
        exercises = exercises
    )

    private fun set() = LoggedSet(
        loggedExerciseId = 1L, setIndex = 0, weightText = "100", weightLb = 100.0, reps = 5, completedAt = 0L
    )

    @Test
    fun `an untouched session has nothing to lose`() {
        assertFalse(state().hasUnsavedWork)
        assertFalse(state().hasAuthoredMetadata)
    }

    @Test
    fun `a logged set is still unsaved work`() {
        assertTrue(state(exercises = listOf(exercise(sets = listOf(set())))).hasUnsavedWork)
    }

    @Test
    fun `a session journal counts`() {
        assertTrue(state(journal = "Gym was packed, did what I could").hasUnsavedWork)
    }

    @Test
    fun `an exercise note counts`() {
        assertTrue(state(exercises = listOf(exercise(note = "left shoulder twinge"))).hasUnsavedWork)
    }

    @Test
    fun `an effort rating counts`() {
        assertTrue(state(exercises = listOf(exercise(difficulty = EffortRating.BRUTAL))).hasUnsavedWork)
    }

    @Test
    fun `a skip counts — it is a statement, not an absence`() {
        // The user said "not today" about that exercise, and the session's honesty percentage is
        // computed from it.
        assertTrue(state(exercises = listOf(exercise(skipped = true))).hasUnsavedWork)
    }

    @Test
    fun `whitespace is not a journal`() {
        assertFalse(state(journal = "   ").hasUnsavedWork)
        assertFalse(state(exercises = listOf(exercise(note = "  "))).hasUnsavedWork)
    }
}
