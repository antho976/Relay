package com.forge.app.domain.coach

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The Week Brief's "last week" assembler. Week start pinned to day 56; last week = days 49–55. */
class WeeklyReviewTest {

    private val day = 24L * 60 * 60 * 1000
    private val weekStart = 56 * day
    private val now = 56 * day + 3_600_000

    private fun session(id: Long, startDay: Int, volume: Double) = Session(
        id = id, dayKey = "upper-a", startedAt = startDay * day,
        finishedAt = startDay * day + 3_600_000, totalVolumeLb = volume
    )

    private fun set(weight: Double) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = 8, completedAt = 0
    )

    private fun bout(startDay: Int, weight: Double) = ExerciseBout(
        sessionStartedAt = startDay * day, effort = EffortRating.JUST_RIGHT, hitFullTarget = false,
        skipped = false, swappedName = null, sets = listOf(set(weight))
    )

    private fun snapshot(
        sessions: List<Session>,
        history: Map<String, List<ExerciseBout>> = emptyMap()
    ) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = sessions,
        exerciseHistory = history, prefs = PrefsSnap()
    )

    @Test
    fun countsSessionsAndVolumePerWeekWindow() {
        val s = snapshot(
            sessions = listOf(
                session(1, 43, 800.0),  // prior week
                session(2, 45, 800.0),  // prior week
                session(3, 50, 1000.0), // last week
                session(4, 52, 1000.0), // last week
                session(5, 54, 1000.0)  // last week
            )
        )
        val r = WeeklyReview.assemble(s, weekStart, sessionsTarget = 4, hasDeloadShadow = false)
        assertEquals(3, r.sessionsLastWeek)
        assertEquals(4, r.sessionsTarget)
        assertEquals(3000.0, r.volumeLastWeekLb, 0.001)
        assertEquals(1600.0, r.volumePriorWeekLb, 0.001)
        assertEquals(87, r.volumeDeltaPct)
    }

    @Test
    fun countsPrsAsNewTopWeightOverAllPriorHistory() {
        // 40 → 45 (before last week) → 50 in last week (PR) → 50 again (not a PR).
        val history = mapOf(
            "ua1" to listOf(bout(40, 40.0), bout(44, 45.0), bout(50, 50.0), bout(53, 50.0))
        )
        val r = WeeklyReview.assemble(snapshot(emptyList(), history), weekStart, 4, hasDeloadShadow = false)
        assertEquals(1, r.prsLastWeek)
    }

    @Test
    fun firstEverBoutIsNotAPr() {
        val history = mapOf("ua1" to listOf(bout(51, 45.0)))
        val r = WeeklyReview.assemble(snapshot(emptyList(), history), weekStart, 4, hasDeloadShadow = false)
        assertEquals(0, r.prsLastWeek)
    }

    @Test
    fun fatigueBandFallsBackWhenGatesUnmet() {
        val r = WeeklyReview.assemble(snapshot(emptyList()), weekStart, 4, hasDeloadShadow = false)
        assertEquals("No read yet", r.fatigueBand)
        assertEquals(null, r.fatigueScore)
    }

    @Test
    fun focusLineFollowsTheWeekShape() {
        val calm = WeeklyReview.assemble(snapshot(emptyList()), weekStart, 4, hasDeloadShadow = false)
        assertTrue(calm.focusLine.contains("Keep doing"))
        val deload = WeeklyReview.assemble(snapshot(emptyList()), weekStart, 4, hasDeloadShadow = true)
        assertTrue(deload.focusLine.contains("Recovery"))
    }

    @Test
    fun assembleIsDeterministic() {
        val s = snapshot(listOf(session(1, 50, 1000.0)))
        assertEquals(
            WeeklyReview.assemble(s, weekStart, 4, hasDeloadShadow = false),
            WeeklyReview.assemble(s, weekStart, 4, hasDeloadShadow = false)
        )
    }
}
