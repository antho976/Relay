package com.forge.app.domain.coach

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.Confidence
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.shared.weight.ProtocolWeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** B2's per-exercise targets: loadable numbers, honest silences, and a seed for new movements. */
class PreSessionBriefTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 100 * day

    private fun set(weight: Double, reps: Int = 8, setType: String? = null) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = reps, completedAt = 0, setType = setType
    )

    private fun bout(daysAgo: Int, sets: List<LoggedSet>) = ExerciseBout(
        sessionStartedAt = now - daysAgo * day, effort = null, hitFullTarget = true,
        skipped = false, swappedName = null, sets = sets
    )

    private fun slot(
        id: String,
        muscle: MuscleGroup = MuscleGroup.CHEST,
        unit: ExerciseUnit = ExerciseUnit.DUMBBELL
    ) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = unit,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(
        slots: List<ProgramSlotSnap> = listOf(slot("bench")),
        history: Map<String, List<ExerciseBout>> = mapOf("bench" to listOf(bout(3, listOf(set(50.0)))))
    ) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("push", "Push day", slots)),
        sessions = emptyList(),
        exerciseHistory = history,
        prefs = PrefsSnap()
    )

    private fun build(
        s: AdaptationSnapshot = snapshot(),
        readiness: Recommendation.ReadinessScale? = null,
        life: LifeEvents.State = LifeEvents.State.NONE
    ) = PreSessionBrief.build(s, "push", readiness, life, ProtocolWeightUnit.LB)

    // ── The basics ─────────────────────────────────────────────────────────────

    @Test
    fun unknownDayHasNoBrief() {
        assertNull(PreSessionBrief.build(snapshot(), "legs", null, LifeEvents.State.NONE))
    }

    @Test
    fun lastWorkingWeightCarriesForward() {
        val t = build()!!.targets.single()
        assertEquals(50.0, t.targetWeightLb!!, 0.001)
        assertTrue(!t.coldStart)
    }

    @Test
    fun warmupSetsNeverSetTheTarget() {
        val s = snapshot(history = mapOf("bench" to listOf(bout(3, listOf(set(200.0, setType = "warmup"), set(50.0))))))
        assertEquals(50.0, build(s)!!.targets.single().targetWeightLb!!, 0.001)
    }

    @Test
    fun bodyweightMovesGetNoWeight() {
        val s = snapshot(slots = listOf(slot("pullup", unit = ExerciseUnit.BODYWEIGHT)), history = emptyMap())
        val t = build(s)!!.targets.single()
        assertNull(t.targetWeightLb)
        assertTrue(t.intent.contains("Reps"))
    }

    // ── Readiness and soreness shape the whole session ─────────────────────────

    @Test
    fun lowReadinessHoldsTheWholeSessionUnder() {
        val r = Recommendation.ReadinessScale(-5, "slept badly", Confidence.MEDIUM)
        val t = build(readiness = r)!!.targets.single()
        assertTrue("must round down to a loadable weight", t.targetWeightLb!! < 50.0)
        assertEquals(0.0, t.targetWeightLb!! % 5.0, 0.001)
    }

    @Test
    fun aSoreMuscleComesInLighter_andSaysSo() {
        val life = LifeEvents.State.NONE.copy(soreMuscles = setOf(MuscleGroup.CHEST))
        val t = build(life = life)!!.targets.single()
        assertTrue(t.easedForSoreness)
        assertTrue(t.targetWeightLb!! < 50.0)
        assertTrue(t.intent.contains("sore"))
    }

    @Test
    fun anInjuredMovementIsRemovedEntirely() {
        val life = LifeEvents.State.NONE.copy(restrictedMuscles = setOf(MuscleGroup.CHEST))
        assertTrue(build(life = life)!!.targets.isEmpty())
    }

    @Test
    fun theReturnRampScalesTheSession() {
        val life = LifeEvents.State.NONE.copy(
            layoff = LifeEvents.Layoff(days = 20, away = false, returning = true, returnedAtMs = now - day, gapStartMs = now - 21 * day)
        )
        val brief = build(life = life)!!
        assertTrue(brief.targets.single().targetWeightLb!! < 50.0)
        assertTrue(brief.intent.contains("Easing back"))
    }

    // ── Cold start: "what weight do I start with?" ─────────────────────────────

    @Test
    fun aNewMovementIsSeededFromSimilarLifts() {
        val s = snapshot(
            slots = listOf(slot("bench"), slot("incline-db")),
            history = mapOf("bench" to listOf(bout(3, listOf(set(100.0)))))
        )
        val fresh = build(s)!!.targets.first { it.exerciseId == "incline-db" }
        assertTrue(fresh.coldStart)
        assertNotNull(fresh.targetWeightLb)
        assertTrue("a seed is conservative", fresh.targetWeightLb!! < 100.0)
        assertTrue(fresh.intent.contains("seeded"))
    }

    @Test
    fun withNothingComparable_theCoachStaysSilentRatherThanGuess() {
        val s = snapshot(slots = listOf(slot("bench")), history = emptyMap())
        val t = build(s)!!.targets.single()
        assertTrue(t.coldStart)
        assertNull(t.targetWeightLb)
        assertTrue(t.intent.contains("control"))
    }

    // ── Loadability ────────────────────────────────────────────────────────────

    @Test
    fun everyTargetLandsOnALoadableStep() {
        val r = Recommendation.ReadinessScale(-3, "low", Confidence.MEDIUM)
        val s = snapshot(history = mapOf("bench" to listOf(bout(3, listOf(set(47.5))))))
        val weight = build(s, readiness = r)!!.targets.single().targetWeightLb!!
        assertEquals("lb steps are 5s", 0.0, weight % 5.0, 0.001)
    }

    @Test
    fun kgUsersGetKgSteps() {
        val brief = PreSessionBrief.build(
            snapshot(), "push", Recommendation.ReadinessScale(-4, "low", Confidence.MEDIUM),
            LifeEvents.State.NONE, ProtocolWeightUnit.KG
        )!!
        assertEquals(0.0, brief.targets.single().targetWeightLb!! % 2.5, 0.001)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        assertEquals(build(), build())
    }
}
