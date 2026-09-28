package com.forge.app.data.repo

import com.forge.app.data.db.projections.RecentPrRow
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import com.forge.app.program.Program
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

    // ── Unmatched imported lifts (ext-*) are listed, named from the stored row ──
    private fun extSet(weightLb: Double, reps: Int, session: Long, name: String?, id: String = "ext-zercher-squat") =
        SetWithExerciseAndSession(
            weightLb = weightLb, reps = reps, exerciseId = id,
            sessionStartedAt = session, loggedExerciseId = 1L, swappedName = name
        )

    @Test
    fun unmatchedImportedLiftIsKeptWithItsStoredName() {
        val sets = listOf(extSet(100.0, 5, 1L, "Zercher Squat"), extSet(110.0, 5, 2L, "Zercher Squat"))
        val lifts = buildE1rmLifts(sets)
        assertEquals(1, lifts.size)
        assertEquals("Zercher Squat", lifts[0].exerciseName)

        val records = buildHallOfFame(sets)
        assertEquals(1, records.size)
        assertEquals("Zercher Squat", records[0].exerciseName)
        assertEquals(110.0, records[0].maxWeightLb, 0.001)
        assertEquals(null, records[0].muscle)

        val curves = buildStrengthCurves(sets + sets + sets)
        assertEquals(1, curves.size)
        assertEquals("Zercher Squat", curves[0].exerciseName)
    }

    @Test
    fun matchedLiftKeepsItsPlanNameOverStoredName() {
        val sets = listOf(set(100.0, 5, 1L).copy(swappedName = "Something else"))
        assertEquals(Program.exercise(ex)?.name, buildE1rmLifts(sets)[0].exerciseName)
    }

    @Test
    fun unresolvedNonImportedLiftStaysDroppedAndNeverTakesItsSwapName() {
        // A removed library movement / rotated-out id: its swapped_name is a mid-session substitute,
        // so it must not be listed under that name (nor as a duplicate of its successor).
        val id = "removed-movement-that-no-longer-exists"
        assertEquals(null, Program.exercise(id))
        val sets = listOf(
            extSet(100.0, 5, 1L, null, id = id),
            extSet(110.0, 5, 2L, "Dumbbell Press", id = id)
        )
        assertTrue(buildE1rmLifts(sets).isEmpty())
        assertTrue(buildHallOfFame(sets).isEmpty())
        assertTrue(buildStrengthCurves(sets + sets + sets).isEmpty())
        assertEquals(null, liftDisplayName(id, sets))
    }

    // The old volume-drop deload insight (#80) and its tests were retired with buildInsights:
    // the adaptation engine's DeloadAdvisor supersedes it (see DeloadAdvisorTest), and the
    // remaining insight rules moved to InsightEngine (see InsightEngineTest).
}
