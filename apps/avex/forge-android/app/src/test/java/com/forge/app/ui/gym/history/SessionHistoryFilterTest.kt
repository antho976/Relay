package com.forge.app.ui.gym.history

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure coverage for the History search / tag / duration filtering ([buildFilteredHistory]). */
class SessionHistoryFilterTest {

    /** The gym-session ids surviving the filter, in order. */
    private fun List<HistoryItem>.workoutIds(): List<Long> =
        mapNotNull { (it as? HistoryItem.Workout)?.session?.id }

    private fun session(
        id: Long,
        dayKey: String = "push-a",
        tags: String = "",
        journal: String = "",
        volumeLb: Double? = 1000.0,
        activeSeconds: Int = 30 * 60
    ) = Session(
        id = id, dayKey = dayKey, startedAt = 0L, finishedAt = 1L,
        totalVolumeLb = volumeLb, tags = tags, journal = journal, activeSeconds = activeSeconds
    )

    private fun cardio(id: Long, date: Long, type: String = "run", dur: Int = 20) =
        CardioEntry(id = id, date = date, type = type, durationMin = dur)

    @Test
    fun availableTagsAreDistinctTrimmedAndSorted() {
        val tags = availableTagsOf(
            listOf(
                session(1, tags = "heavy, focus"),
                session(2, tags = "focus,deload"),
                session(3, tags = "")
            )
        )
        assertEquals(listOf("deload", "focus", "heavy"), tags)
    }

    @Test
    fun tagFilterMatchesAnyOfTheSessionsTags() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1, tags = "heavy,focus"), session(2, tags = "deload")),
            cardio = emptyList(),
            exerciseNamesBySession = emptyMap(),
            filters = HistoryFilters(tag = "focus")
        )
        assertEquals(listOf(1L), out.workoutIds())
    }

    @Test
    fun searchMatchesJournalCaseInsensitively() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1, journal = "Felt strong on bench"), session(2, journal = "easy day")),
            cardio = emptyList(),
            exerciseNamesBySession = emptyMap(),
            filters = HistoryFilters(query = "STRONG")
        )
        assertEquals(listOf(1L), out.workoutIds())
    }

    @Test
    fun searchMatchesLoggedExerciseName() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1), session(2)),
            cardio = emptyList(),
            exerciseNamesBySession = mapOf(
                1L to listOf("DB Bench Press"),
                2L to listOf("DB Row (1-arm)")
            ),
            filters = HistoryFilters(query = "bench")
        )
        assertEquals(listOf(1L), out.workoutIds())
    }

    @Test
    fun durationShortFilterUsesActiveMinutes() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1, activeSeconds = 20 * 60), session(2, activeSeconds = 70 * 60)),
            cardio = emptyList(),
            exerciseNamesBySession = emptyMap(),
            filters = HistoryFilters(duration = SessionHistoryFilter.SHORT)
        )
        assertEquals(listOf(1L), out.workoutIds())
    }

    @Test
    fun anyFilterActiveReflectsInputs() {
        assertFalse(SessionHistoryUiState().anyFilterActive)
        assertTrue(SessionHistoryUiState(query = "x").anyFilterActive)
        assertTrue(SessionHistoryUiState(tagFilter = "focus").anyFilterActive)
    }

    @Test
    fun cardioAndWorkoutsMergeNewestFirst() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1)),               // startedAt = 0
            cardio = listOf(cardio(5, date = 100)),       // later than the workout
            exerciseNamesBySession = emptyMap(),
            filters = HistoryFilters()
        )
        // Both appear; the later cardio session sorts ahead of the day-zero workout.
        assertEquals(listOf("c5", "w1"), out.map { it.key })
    }

    @Test
    fun tagFilterHidesCardio() {
        val out = buildFilteredHistory(
            workouts = listOf(session(1, tags = "focus")),
            cardio = listOf(cardio(5, date = 100)),
            exerciseNamesBySession = emptyMap(),
            filters = HistoryFilters(tag = "focus")
        )
        // Cardio carries no tags, so a tag filter leaves only the matching workout.
        assertEquals(listOf("w1"), out.map { it.key })
    }
}
