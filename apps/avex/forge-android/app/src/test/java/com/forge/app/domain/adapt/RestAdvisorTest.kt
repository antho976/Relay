package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.RestEvent
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.Difficulty
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseTag
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * System 2 of the adaptation engine. "10-12"/"12-15" rep ranges keep SessionEstimate's
 * heavy-reps bonus out of the way, so the canonical bases here are 180s (compound) and
 * 90s (isolation).
 */
class RestAdvisorTest {

    private val compoundPlan = ExercisePlan(
        "ua1", "DB Bench Press", 3, "10-12", ExerciseUnit.DUMBBELL,
        MuscleGroup.CHEST, Difficulty.BEGINNER, "", tags = listOf(ExerciseTag.COMPOUND)
    )
    private val isolationPlan = ExercisePlan(
        "ua4", "DB Lateral Raise", 3, "12-15", ExerciseUnit.DUMBBELL,
        MuscleGroup.SHOULDERS, Difficulty.BEGINNER, "", tags = listOf(ExerciseTag.ISOLATION)
    )
    private val planById = mapOf(compoundPlan.id to compoundPlan, isolationPlan.id to isolationPlan)

    private fun event(exerciseId: String, realized: Int) = RestEvent(
        sessionId = 1, exerciseId = exerciseId, setIndex = 0,
        plannedSeconds = 180, realizedSeconds = realized,
        endedBy = "next_set", secondsAdded = 0, loggedAt = 0
    )

    private fun tuningFor(events: List<RestEvent>): RestTuning =
        RestAdvisor.tuning(RestAdvisor.samples(events) { planById[it] })

    // ── Precedence + canonical behavior ───────────────────────────────────────

    @Test
    fun manualOverrideAlwaysWins() {
        val tuning = tuningFor(List(20) { event("ua1", 60) }) // strong shorten signal
        val p = RestAdvisor.restSeconds(compoundPlan, EffortRating.BRUTAL, overrideSeconds = 75, tuning = tuning)
        assertEquals(75, p.seconds)
        assertTrue(p.reason.contains("custom"))
    }

    @Test
    fun coldStart_noSamples_usesCanonicalBase() {
        val p = RestAdvisor.restSeconds(compoundPlan, lastEffort = null, overrideSeconds = null)
        assertEquals(180, p.seconds)
        assertTrue(p.reason.contains("compound base 3:00"))
    }

    @Test
    fun isolationBaseIsShorter() {
        val p = RestAdvisor.restSeconds(isolationPlan, null, null)
        assertEquals(90, p.seconds)
        assertTrue(p.reason.contains("isolation"))
    }

    @Test
    fun brutalSetAddsTheBonus_withReason() {
        val p = RestAdvisor.restSeconds(compoundPlan, EffortRating.BRUTAL, null)
        assertEquals(210, p.seconds)
        assertTrue(p.reason.contains("+30s after a brutal set"))
    }

    // ── Confidence gate ───────────────────────────────────────────────────────

    @Test
    fun tooFewSamples_staysUntuned() {
        // 7 samples (one below the gate) of strong shortening must change nothing.
        val tuning = tuningFor(List(7) { event("ua1", 90) })
        assertEquals(180, RestAdvisor.restSeconds(compoundPlan, null, null, tuning).seconds)
    }

    @Test
    fun enoughSamples_tunesTowardTheUsersPace() {
        // 8 samples at 135s vs a 180s base → factor 0.75 → 135s, with the % in the reason.
        val tuning = tuningFor(List(8) { event("ua1", 135) })
        val p = RestAdvisor.restSeconds(compoundPlan, null, null, tuning)
        assertEquals(135, p.seconds)
        assertTrue(p.reason.contains("−25%"))
        assertTrue(p.reason.contains("usual rest"))
    }

    @Test
    fun adjustmentIsClampedToTheMaxBand() {
        // Realized 45s vs 180s base = ratio 0.25 — clamps to 0.60 → 108 → grid 105.
        val tuning = tuningFor(List(12) { event("ua1", 45) })
        assertEquals(0.60, tuning.factors.getValue(MovementRole.COMPOUND), 0.0001)
        assertEquals(105, RestAdvisor.restSeconds(compoundPlan, null, null, tuning).seconds)
    }

    @Test
    fun tunedDurationSnapsToTheGrid() {
        // Factor 0.9 → 162s, which isn't a timer-friendly value → snaps to 165.
        val tuning = tuningFor(List(8) { event("ua1", 162) })
        assertEquals(165, RestAdvisor.restSeconds(compoundPlan, null, null, tuning).seconds)
    }

    // ── Sample qualification ──────────────────────────────────────────────────

    @Test
    fun insaneRealizedRestsAreExcluded() {
        // 5s mistaps and 700s walked-away intervals never count.
        val events = List(8) { event("ua1", 5) } + List(8) { event("ua1", 700) }
        assertTrue(RestAdvisor.samples(events) { planById[it] }.isEmpty())
    }

    @Test
    fun eventsForUnresolvableExercisesAreExcluded() {
        val events = List(8) { event("gone-from-program", 120) }
        assertTrue(RestAdvisor.samples(events) { planById[it] }.isEmpty())
    }

    @Test
    fun roleBucketsAreIndependent() {
        // Heavy shortening on compounds must not touch the isolation prescription.
        val tuning = tuningFor(List(20) { event("ua1", 110) })
        assertEquals(90, RestAdvisor.restSeconds(isolationPlan, null, null, tuning).seconds)
    }

    @Test
    fun medianResistsOutliers() {
        // Eight samples at ratio 0.75 and one at 2× — the median holds at 0.75.
        val events = List(8) { event("ua1", 135) } + event("ua1", 360)
        assertEquals(0.75, tuningFor(events).factors.getValue(MovementRole.COMPOUND), 0.0001)
    }

    // ── Determinism ───────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSamePrescription() {
        val tuning = tuningFor(List(8) { event("ua1", 135) })
        assertEquals(
            RestAdvisor.restSeconds(compoundPlan, EffortRating.BRUTAL, null, tuning),
            RestAdvisor.restSeconds(compoundPlan, EffortRating.BRUTAL, null, tuning)
        )
    }
}
