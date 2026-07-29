package com.forge.app.domain.coach

import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** E's mid-session re-plans: the three things that go wrong, each answered in one tap. */
class SessionAdaptorTest {

    private fun slot(
        id: String,
        muscle: MuscleGroup,
        sets: Int = 3,
        swaps: List<String> = listOf("alt-$id", "alt2-$id")
    ) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = sets, repsText = "8-10", swapCandidateIds = swaps
    )

    private val session = listOf(
        slot("bench", MuscleGroup.CHEST, sets = 4),
        slot("row", MuscleGroup.BACK, sets = 4),
        slot("curl", MuscleGroup.BICEPS, sets = 3),
        slot("raise", MuscleGroup.SHOULDERS, sets = 3),
        slot("calf", MuscleGroup.CALVES, sets = 3)
    )

    // ── "I have N minutes" ─────────────────────────────────────────────────────

    @Test
    fun plentyOfTimeKeepsEverything() {
        val t = SessionAdaptor.triage(session, minutesAvailable = 120)
        assertEquals(session.size, t.keep.size)
        assertTrue(t.drop.isEmpty())
        assertTrue(t.reason.contains("fits"))
    }

    @Test
    fun halfTheTimeKeepsTheCompounds() {
        val t = SessionAdaptor.triage(session, minutesAvailable = 30)
        assertTrue(t.keep.isNotEmpty())
        assertTrue("compounds survive", t.keep.any { it.exerciseId == "bench" || it.exerciseId == "row" })
        assertTrue("isolation goes first", t.drop.any { it.exerciseId == "calf" })
    }

    @Test
    fun goalMusclesSurviveAheadOfEverything() {
        val t = SessionAdaptor.triage(
            session, minutesAvailable = 20, goalMuscles = setOf(MuscleGroup.BICEPS)
        )
        assertTrue("the goal muscle's work is protected", t.keep.any { it.muscle == MuscleGroup.BICEPS })
    }

    @Test
    fun aSessionIsNeverTriagedToNothing() {
        val t = SessionAdaptor.triage(session, minutesAvailable = 1)
        assertTrue(t.keep.size >= SessionAdaptor.MIN_EXERCISES)
    }

    @Test
    fun whatSurvivesKeepsTheOriginalOrder() {
        val t = SessionAdaptor.triage(session, minutesAvailable = 30)
        val originalOrder = session.filter { it in t.keep }.map { it.exerciseId }
        assertEquals(originalOrder, t.keep.map { it.exerciseId })
    }

    @Test
    fun anEmptySessionIsHandled() {
        val t = SessionAdaptor.triage(emptyList(), minutesAvailable = 45)
        assertTrue(t.keep.isEmpty())
    }

    // ── "The rack is taken" ────────────────────────────────────────────────────

    @Test
    fun swapsComeFromTheSlotsOwnPool() {
        val out = SessionAdaptor.swapCandidates(session.first(), LifeEvents.State.NONE)
        assertEquals(listOf("alt-bench", "alt2-bench"), out)
    }

    @Test
    fun anInjuredAlternativeIsNeverOffered() {
        val life = LifeEvents.State.NONE.copy(restrictedExerciseIds = setOf("alt-bench"))
        val out = SessionAdaptor.swapCandidates(session.first(), life)
        assertEquals(listOf("alt2-bench"), out)
    }

    // ── "Something hurts" ──────────────────────────────────────────────────────

    @Test
    fun soreMuscleWorkComesOut_andTheRestStands() {
        val t = SessionAdaptor.soreReroute(session, MuscleGroup.CHEST, LifeEvents.State.NONE)
        assertTrue(t.drop.all { it.muscle == MuscleGroup.CHEST })
        assertTrue(t.keep.none { it.muscle == MuscleGroup.CHEST })
        assertEquals(session.size - 1, t.keep.size)
    }

    @Test
    fun anInjuryIsWordedDifferentlyFromSoreness() {
        val life = LifeEvents.State.NONE.copy(restrictedMuscles = setOf(MuscleGroup.CHEST))
        val t = SessionAdaptor.soreReroute(session, MuscleGroup.CHEST, life)
        assertTrue(t.reason.contains("injured"))
    }

    @Test
    fun aMuscleNotInTodaysSessionChangesNothing() {
        val t = SessionAdaptor.soreReroute(session, MuscleGroup.QUADS, LifeEvents.State.NONE)
        assertEquals(session.size, t.keep.size)
        assertTrue(t.drop.isEmpty())
    }

    // ── The logged-sets rule ───────────────────────────────────────────────────

    @Test
    fun aSwapIsOnlyCleanBeforeSetsExist() {
        assertTrue(SessionAdaptor.canSwapCleanly(0))
        assertFalse("re-keying logged sets would mis-attribute them", SessionAdaptor.canSwapCleanly(1))
    }

    @Test
    fun deterministic() {
        assertEquals(
            SessionAdaptor.triage(session, 30),
            SessionAdaptor.triage(session, 30)
        )
    }
}
