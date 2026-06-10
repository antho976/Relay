package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.Program
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SnapshotAssemblerTest {

    private fun session(id: Long, startedAt: Long) =
        Session(id = id, dayKey = "upper-a", startedAt = startedAt, finishedAt = startedAt + 1)

    private fun le(id: Long, sessionId: Long, exerciseId: String = "ua1") =
        LoggedExercise(id = id, sessionId = sessionId, exerciseId = exerciseId, orderIndex = 0,
            difficulty = EffortRating.JUST_RIGHT)

    private fun set(leId: Long, idx: Int, weight: Double = 45.0) =
        LoggedSet(id = leId * 10 + idx, loggedExerciseId = leId, setIndex = idx,
            weightText = "$weight", weightLb = weight, reps = 10, completedAt = 0L)

    private fun assemble(
        sessions: List<Session>,
        les: List<LoggedExercise>,
        sets: List<LoggedSet>
    ) = SnapshotAssembler.assemble(
        nowMs = 1_000L,
        program = Program.seedDays,
        swapCandidateIds = { plan -> listOf("swap-for-${plan.id}") },
        sessions = sessions,
        loggedExercises = les,
        loggedSets = sets,
        prefs = PrefsSnap()
    )

    @Test
    fun boutsAreGroupedPerExerciseAndOrderedBySessionDate() {
        // Sessions deliberately passed newest-first; bouts must come out oldest-first.
        val snap = assemble(
            sessions = listOf(session(2, startedAt = 200), session(1, startedAt = 100)),
            les = listOf(le(20, sessionId = 2), le(10, sessionId = 1)),
            sets = listOf(set(20, 0), set(10, 1), set(10, 0))
        )
        val bouts = snap.exerciseHistory.getValue("ua1")
        assertEquals(2, bouts.size)
        assertEquals(100L, bouts[0].sessionStartedAt)
        assertEquals(200L, bouts[1].sessionStartedAt)
        // Sets within a bout are in performed (setIndex) order.
        assertEquals(listOf(0, 1), bouts[0].sets.map { it.setIndex })
        assertEquals(EffortRating.JUST_RIGHT, bouts[0].effort)
        // Sessions list itself is re-sorted oldest-first.
        assertEquals(listOf(1L, 2L), snap.sessions.map { it.id })
    }

    @Test
    fun rowsFromSessionsOutsideTheListAreDropped() {
        // An exercise whose parent session isn't in the (finished, tracked) list — e.g. an
        // unfinished or untracked session — must not produce a bout.
        val snap = assemble(
            sessions = listOf(session(1, startedAt = 100)),
            les = listOf(le(10, sessionId = 1), le(99, sessionId = 99)),
            sets = listOf(set(10, 0), set(99, 0))
        )
        assertEquals(1, snap.exerciseHistory.getValue("ua1").size)
    }

    @Test
    fun programSlotsCarryTheirSwapCandidates() {
        val snap = assemble(emptyList(), emptyList(), emptyList())
        val firstSlot = snap.program.first().slots.first()
        assertEquals(listOf("swap-for-${firstSlot.exerciseId}"), firstSlot.swapCandidateIds)
        assertTrue(snap.exerciseHistory.isEmpty())
    }
}
