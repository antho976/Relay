package com.forge.app.data.repo

import com.forge.app.data.db.projections.RecentPrRow
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-function coverage for the stats aggregation helpers (#37) and the loggedExerciseId
 * PR matching introduced for #69. No DAO/DI — these operate on plain projection lists.
 */
class StatsAggregationsTest {

    // A real library id so Program.exercise(id) resolves (buildE1rmLifts needs the name).
    private val ex = "db-bench-press"

    private fun set(weightLb: Double?, reps: Int, session: Long, loggedExerciseId: Long = 1L) =
        SetWithExerciseAndSession(
            weightLb = weightLb,
            reps = reps,
            exerciseId = ex,
            sessionStartedAt = session,
            loggedExerciseId = loggedExerciseId
        )

    // ── #69: buildPrEntries matches the exact logged exercise, not session+exercise ──
    @Test
    fun prEntryMatchesByLoggedExerciseIdNotSessionDate() {
        // Two logged instances of the SAME exercise in ONE session (identical started_at).
        val session = 1_000L
        val allSets = listOf(
            set(100.0, 5, session, loggedExerciseId = 10L),
            set(140.0, 3, session, loggedExerciseId = 20L)
        )
        // The PR row points at the lighter instance (id 10) — it must resolve to that set's 100,
        // not the heavier 140 from the other instance in the same session.
        val rows = listOf(
            RecentPrRow(exerciseId = ex, swappedName = null, sessionStartedAt = session, loggedExerciseId = 10L)
        )
        val prs = buildPrEntries(rows, allSets)
        assertEquals(1, prs.size)
        assertEquals(5, prs[0].reps)
        assertEquals("expected the id-10 set (100)", 100.0, prs[0].weightLb, 0.001)
    }

    // ── #37: Epley e1RM, taking the best set within each session ──
    @Test
    fun e1rmUsesEpleyAndPicksSessionMax() {
        // 110x3 → 110*(1+3/30)=121 ; 100x5 → 100*(1+5/30)=116.67 ; session best = 121
        val sets = listOf(set(100.0, 5, 1L), set(110.0, 3, 1L))
        val lifts = buildE1rmLifts(sets)
        assertEquals(1, lifts.size)
        assertEquals(121.0, lifts[0].currentE1rm, 0.01)
    }

    // The old volume-drop deload insight (#80) and its tests were retired with buildInsights:
    // the adaptation engine's DeloadAdvisor supersedes it (see DeloadAdvisorTest), and the
    // remaining insight rules moved to InsightEngine (see InsightEngineTest).
}
