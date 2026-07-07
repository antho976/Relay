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

    /** [count] sessions one day apart starting at [firstDay] — first session anchors the block. */
    private fun blockSessions(firstDay: Int, count: Int = 8): List<Session> =
        (0 until count).map { session(it + 1L, startDay = firstDay + it, volume = 1000.0) }

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
        assertTrue(calm.focusLine, calm.focusLine.contains("quiet week"))
        val deload = WeeklyReview.assemble(snapshot(emptyList()), weekStart, 4, hasDeloadShadow = true)
        assertTrue(deload.focusLine.contains("Recovery"))
    }

    @Test
    fun focusLine_tracksTheMesocyclePhase() {
        // weekStart = day 56. With no deload, the block is anchored on the first session, so the
        // week-in-block (and thus the phase cue) is set by how far back that first session sits.
        val accumulation = WeeklyReview.assemble(snapshot(blockSessions(firstDay = 49)), weekStart, 4, hasDeloadShadow = false)
        assertTrue("week 1 → accumulation: ${accumulation.focusLine}", accumulation.focusLine.contains("accumulation"))

        val mid = WeeklyReview.assemble(snapshot(blockSessions(firstDay = 42)), weekStart, 4, hasDeloadShadow = false)
        assertTrue("week 2 → mid-block: ${mid.focusLine}", mid.focusLine.contains("Mid-block"))

        val peak = WeeklyReview.assemble(snapshot(blockSessions(firstDay = 28)), weekStart, 4, hasDeloadShadow = false)
        assertTrue("week 4 → peak: ${peak.focusLine}", peak.focusLine.contains("Peak week"))

        val overdue = WeeklyReview.assemble(snapshot(blockSessions(firstDay = 0)), weekStart, 4, hasDeloadShadow = false)
        assertTrue("week 8 → ease off soon: ${overdue.focusLine}", overdue.focusLine.contains("lighter week"))
    }

    @Test
    fun focusLine_belowTheDataGateFallsToADataGroundedQuietLine() {
        // Only 4 sessions — not enough block to name a phase, so a line grounded in the week's own
        // numbers holds (here: 4 of 4 sessions in → the "every session" cue), never the old generic.
        val r = WeeklyReview.assemble(snapshot(blockSessions(firstDay = 49, count = 4)), weekStart, 4, hasDeloadShadow = false)
        assertTrue(r.focusLine, r.focusLine.contains("Every session"))
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
