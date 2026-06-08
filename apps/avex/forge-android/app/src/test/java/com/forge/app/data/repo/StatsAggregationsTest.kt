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
        assertTrue("expected the id-10 set (100), got ${prs[0].weightText}", prs[0].weightText.startsWith("100"))
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

    // ── #37: deload heuristic — newer 3 sessions < 80% of older 3 ──
    @Test
    fun deloadInsightFiresWhenVolumeDropsOverRecentSessions() {
        val now = 1_000_000_000L
        val day = 24L * 3600 * 1000
        val sets = mutableListOf<SetWithExerciseAndSession>()
        // older 3 sessions: 100x10 = 1000 vol each
        for (i in 0 until 3) sets += set(100.0, 10, now - (6 - i) * day)
        // newer 3 sessions: 40x10 = 400 vol each → > 20% drop
        for (i in 0 until 3) sets += set(40.0, 10, now - (3 - i) * day)
        val insights = buildInsights(sets, emptyList(), emptyList(), now)
        assertTrue(insights.any { it.title == "Consider a deload" })
    }

    @Test
    fun deloadInsightAbsentWhenVolumeSteady() {
        val now = 1_000_000_000L
        val day = 24L * 3600 * 1000
        val sets = (0 until 6).map { set(100.0, 10, now - (6 - it) * day) }
        val insights = buildInsights(sets, emptyList(), emptyList(), now)
        assertFalse(insights.any { it.title == "Consider a deload" })
    }
}
