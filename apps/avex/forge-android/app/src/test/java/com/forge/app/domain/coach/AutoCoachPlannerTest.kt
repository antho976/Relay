package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.domain.adapt.SleepNight
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Weekly Coach Pass planner (auto-coach Phases 1 + 3). Fixtures pin "now" to day 60,
 * mirroring DeloadAdvisorTest's window layout.
 */
class AutoCoachPlannerTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 60 * day

    private fun session(id: Long, startDay: Int, volume: Double = 1000.0) = Session(
        id = id, dayKey = "upper-a", startedAt = startDay * day,
        finishedAt = startDay * day + 3_600_000, totalVolumeLb = volume
    )

    /** 8 sessions across days 34-57 — passes the planner's baseline + gap gates. */
    private fun baseSessions(): List<Session> =
        (0 until 8).map { session(it + 1L, startDay = 34 + it * 3) }

    private fun set(weight: Double, reps: Int = 8) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = reps, completedAt = 0
    )

    private fun bout(startDay: Int, weight: Double, effort: EffortRating? = EffortRating.JUST_RIGHT, skipped: Boolean = false) =
        ExerciseBout(
            sessionStartedAt = startDay * day, effort = effort, hitFullTarget = false,
            skipped = skipped, swappedName = null, sets = listOf(set(weight))
        )

    /** stall = n: a first bout then n more with zero e1RM improvement. */
    private fun stalledBouts(n: Int): List<ExerciseBout> =
        (0..n).map { i -> bout(startDay = 38 + i * 2, weight = 45.0) }

    /** Linearly progressing bouts — what a working beginner plan looks like. */
    private fun progressingBouts(n: Int): List<ExerciseBout> =
        (0..n).map { i -> bout(startDay = 38 + i * 2, weight = 45.0 + i * 5) }

    /** Progressing bouts at a chosen per-bout step — a small step is "slow but climbing", not stalled. */
    private fun progressing(stepWeight: Double, startWeight: Double = 45.0, n: Int = 8): List<ExerciseBout> =
        (0..n).map { i -> bout(startDay = 38 + i * 2, weight = startWeight + i * stepWeight) }

    private fun slot(
        id: String = "ua1",
        swaps: List<String> = listOf("alt-1", "alt-2"),
        sets: Int = 3,
        muscle: MuscleGroup = MuscleGroup.CHEST
    ) =
        ProgramSlotSnap(
            exerciseId = id, name = "Lift $id", muscle = muscle,
            unit = ExerciseUnit.DUMBBELL, tags = emptyList(), targetSets = sets,
            repsText = "8-10", swapCandidateIds = swaps
        )

    private fun snapshot(
        history: Map<String, List<ExerciseBout>>,
        sessions: List<Session> = baseSessions(),
        slots: List<ProgramSlotSnap> = history.keys.map { slot(it) },
        cardio: List<CardioEntry> = emptyList(),
        health: HealthSnap = HealthSnap(),
        prefs: PrefsSnap = PrefsSnap()
    ) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("upper-a", "Upper A", slots)),
        sessions = sessions,
        exerciseHistory = history,
        cardio = cardio,
        prefs = prefs,
        health = health
    )

    /** Six short nights (6h ≤ the 6.5h ceiling) inside the deload window → the sleep-debt driver (+2). */
    private fun shortNights() = (0 until 6).map { SleepNight(endedAtMs = now - (1 + it) * day, durationMin = 360) }

    private fun beginner(target: Int = 0) = CoachPassInputs("beginner", sessionsTarget = target)

    /**
     * [baseSessions] plus one session inside the CURRENT ISO week — what the volume_up adherence
     * gate actually counts.
     *
     * That gate reads "sessions since Monday 00:00", matching WeeklyReview and the Brief, rather
     * than a rolling 7 x 24 h window. Epoch day 60 — this class's "now" — is itself a MONDAY at
     * 00:00 UTC, so every session in [baseSessions] (days 34-55) belongs to an earlier week and
     * this week reads zero however well the athlete trained. Without a session on the right side
     * of that boundary the gate can never open, and a test asserting SHADOW is really asserting
     * "the week is empty".
     */
    private fun sessionsMeetingThisWeeksTarget(): List<Session> =
        baseSessions() + session(id = 99, startDay = 60)

    // ── Hold semantics ─────────────────────────────────────────────────────────

    @Test
    fun coldStart_fewSessions_holdsWithExplanation() {
        val r = AutoCoachPlanner.evaluate(
            snapshot(emptyMap(), sessions = baseSessions().take(3)), beginner()
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
        assertTrue("reason should mention the baseline: ${r.holdReason}", "baseline" in r.holdReason!!)
    }

    @Test
    fun longTrainingGap_holdsWithEaseBackReason() {
        // 8 sessions, but the last one was 20 days ago.
        val old = (0 until 8).map { session(it + 1L, startDay = 16 + it * 3) }
        val r = AutoCoachPlanner.evaluate(snapshot(emptyMap(), sessions = old), beginner())
        assertEquals(CoachPassStatus.HOLD, r.status)
        assertTrue("reason should mention the break: ${r.holdReason}", "break" in r.holdReason!!)
    }

    @Test
    fun everythingProgressing_holdsAndSaysThePlanIsWorking() {
        val r = AutoCoachPlanner.evaluate(snapshot(mapOf("ua1" to progressingBouts(8))), beginner())
        assertEquals(CoachPassStatus.HOLD, r.status)
        assertTrue("reason should explain the hold: ${r.holdReason}", "working" in r.holdReason!!)
    }

    // ── Proposed decisions ─────────────────────────────────────────────────────

    @Test
    fun deloadSupersedesEverything_singleDeloadDecision() {
        // DeloadAdvisor needs ≥5 points: effort inflation (+2), sleep debt (+2), sore cardio (+1).
        val brutal = (0..8).map { i ->
            bout(startDay = 48 + i, weight = 45.0, effort = EffortRating.BRUTAL)
        }
        val cardio = listOf(CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"))
        val r = AutoCoachPlanner.evaluate(
            snapshot(mapOf("ua1" to brutal), cardio = cardio, health = HealthSnap(sleepNights = shortNights())), beginner()
        )
        assertEquals(CoachPassStatus.SHADOW, r.status)
        assertEquals(listOf("deload"), r.decisions.map { it.type })
    }

    @Test
    fun stalledLift_surfacesASwapWithReplacementPayload() {
        val r = AutoCoachPlanner.evaluate(snapshot(mapOf("ua1" to stalledBouts(8))), beginner())
        assertEquals(CoachPassStatus.SHADOW, r.status)
        assertEquals(1, r.decisions.size)
        val d = r.decisions.first()
        assertEquals("swap", d.type)
        assertEquals("alt-1", d.payload)
        assertEquals("upper-a", d.dayKey)
        assertTrue(d.reason.isNotBlank())
    }

    @Test
    fun beginnerIsCappedAtOneDecision_intermediateGetsTwo() {
        val history = mapOf("ua1" to stalledBouts(8), "ua2" to stalledBouts(8))
        assertEquals(1, AutoCoachPlanner.evaluate(snapshot(history), beginner()).decisions.size)
        assertEquals(2, AutoCoachPlanner.evaluate(snapshot(history), CoachPassInputs("intermediate")).decisions.size)
    }

    // ── Phase 3: locks, volume, reverts ────────────────────────────────────────

    @Test
    fun coachLockedSlot_isNeverTargeted() {
        val r = AutoCoachPlanner.evaluate(
            snapshot(mapOf("ua1" to stalledBouts(8))),
            CoachPassInputs("beginner", lockedExerciseIds = setOf("ua1"))
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
    }

    @Test
    fun volumeUp_whenMuscleProgressingFreshAndAdherent() {
        val r = AutoCoachPlanner.evaluate(
            snapshot(
                mapOf("ua1" to progressingBouts(8)),
                sessions = sessionsMeetingThisWeeksTarget()
            ),
            beginner(target = 1)
        )
        assertEquals(CoachPassStatus.SHADOW, r.status)
        val d = r.decisions.single()
        assertEquals("volume_up", d.type)
        assertEquals("4", d.payload)
        assertEquals("ua1", d.targetKey)
    }

    @Test
    fun buildingFatigue_holdsWithConsolidationGuidance() {
        // Fatigue at 3/5 (sleep debt +2, sore cardio +1) — below the deload line but climbing.
        // Lifts are progressing and target is 0 (no volume_up), so the pass is otherwise quiet:
        // the smart call is a consolidation week, not a generic "plan is working" hold.
        val cardio = listOf(CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"))
        val r = AutoCoachPlanner.evaluate(
            snapshot(mapOf("ua1" to progressingBouts(8)), cardio = cardio, health = HealthSnap(sleepNights = shortNights())), beginner()
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
        assertTrue("reason should propose a consolidation week: ${r.holdReason}", "consolidation" in r.holdReason!!)
    }

    @Test
    fun volumeUp_prioritisesTheLaggingMuscle() {
        // Two qualifying muscles: CHEST climbing fast, BACK climbing slowly. The extra set goes to
        // the laggard (BACK), not the muscle already winning.
        val history = mapOf("ua1" to progressing(5.0), "ub1" to progressing(1.0))
        val slots = listOf(slot("ua1"), slot("ub1", muscle = MuscleGroup.BACK))
        val r = AutoCoachPlanner.evaluate(
            snapshot(history, sessions = sessionsMeetingThisWeeksTarget(), slots = slots),
            beginner(target = 1)
        )
        assertEquals(CoachPassStatus.SHADOW, r.status)
        val d = r.decisions.single { it.type == "volume_up" }
        assertEquals("ub1", d.targetKey)
    }

    @Test
    fun volumeUp_blockedByDriftCap() {
        // Adherence deliberately SATISFIED, so the only thing left to block the extra set is the
        // drift cap. Without the in-week session this asserted HOLD for the wrong reason — an empty
        // week holds on its own, and the cap could have been deleted with the test still green.
        val r = AutoCoachPlanner.evaluate(
            snapshot(
                mapOf("ua1" to progressingBouts(8)),
                sessions = sessionsMeetingThisWeeksTarget()
            ),
            CoachPassInputs("beginner", sessionsTarget = 1, volumeNetByMuscle = mapOf(MuscleGroup.CHEST to 2))
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
    }

    @Test
    fun volumeUp_blockedWhileMuscleInOutcomeWindow() {
        // Adherence satisfied here too — see [volumeUp_blockedByDriftCap] — so the muscle lock is
        // the only thing under test.
        val r = AutoCoachPlanner.evaluate(
            snapshot(
                mapOf("ua1" to progressingBouts(8)),
                sessions = sessionsMeetingThisWeeksTarget()
            ),
            CoachPassInputs("beginner", sessionsTarget = 1, volumeLockedMuscles = setOf(MuscleGroup.CHEST))
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
    }

    @Test
    fun volumeDown_forChronicallySkippedExercise() {
        // 2 real bouts then 4 recent ones, 3 of them skipped — the slot is oversized.
        val history = listOf(bout(38, 45.0), bout(40, 45.0)) +
            listOf(bout(48, 45.0, skipped = true), bout(50, 45.0, skipped = true),
                bout(52, 45.0), bout(54, 45.0, skipped = true))
        val r = AutoCoachPlanner.evaluate(snapshot(mapOf("ua1" to history)), beginner())
        assertEquals(CoachPassStatus.SHADOW, r.status)
        val d = r.decisions.single()
        assertEquals("volume_down", d.type)
        assertEquals("2", d.payload)
    }

    @Test
    fun revertProposals_takePriorityOverNewChanges() {
        val revert = ShadowDecision("revert", "ua2", "Lift ua2", "Revert: swap", "didn't land", "upper-a", "42")
        val r = AutoCoachPlanner.evaluate(
            snapshot(mapOf("ua1" to stalledBouts(8))),
            CoachPassInputs("beginner", revertProposals = listOf(revert))
        )
        assertEquals(CoachPassStatus.SHADOW, r.status)
        assertEquals(listOf("revert"), r.decisions.map { it.type })
    }

    @Test
    fun declinedSwap_isNotReProposed() {
        // The user skipped/reverted this exact rotation before — it must not reappear every week
        // (seam fix, finding 19). alt-1 is the only candidate, so declining it yields a hold.
        val r = AutoCoachPlanner.evaluate(
            snapshot(mapOf("ua1" to stalledBouts(8))),
            CoachPassInputs("beginner", declinedStructural = setOf("swap:ua1:alt-1"))
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
    }

    @Test
    fun evaluateIsDeterministic() {
        val s = snapshot(mapOf("ua1" to stalledBouts(8)))
        assertEquals(
            AutoCoachPlanner.evaluate(s, beginner()),
            AutoCoachPlanner.evaluate(s, beginner())
        )
    }

    // ── Block phases are policy, not copy ─────────────────────────────────────

    private fun inBlock(phase: BlockPhase, target: Int = 1) = beginner(target = target).copy(blockPhase = phase)

    /** Fresh, adherent, progressing: the exact state in which the reactive pass adds a set. */
    private fun earningMore() = snapshot(mapOf("ua1" to progressingBouts(8)), sessions = sessionsMeetingThisWeeksTarget())

    @Test
    fun accumulate_stillAddsASet() {
        val r = AutoCoachPlanner.evaluate(earningMore(), inBlock(BlockPhase.ACCUMULATE))
        assertEquals("volume_up", r.decisions.single().type)
    }

    @Test
    fun intensify_holdsVolume_howeverFreshTheAthleteReads() {
        val r = AutoCoachPlanner.evaluate(earningMore(), inBlock(BlockPhase.INTENSIFY))
        assertTrue("Intensify never adds a set: ${r.decisions}", r.decisions.none { it.type == "volume_up" })
    }

    @Test
    fun peak_trimsASetFromTheBiggestSlotAndAddsNone() {
        val r = AutoCoachPlanner.evaluate(earningMore(), inBlock(BlockPhase.PEAK))
        assertTrue(r.decisions.none { it.type == "volume_up" })
        val down = r.decisions.single { it.type == "volume_down" }
        assertEquals("ua1", down.targetKey)
        assertEquals("2", down.payload)
        assertTrue(down.reason, "Peak week" in down.reason)
    }

    @Test
    fun peak_keepsTheStructureStill_soTheTestWeekMeasuresWhatWasBuilt() {
        // The same stall that earns a swap in a reactive pass earns nothing structural in Peak:
        // changing the movement the week it is tested changes what the test measures.
        val stalled = snapshot(mapOf("ua1" to stalledBouts(8)))
        assertEquals("swap", AutoCoachPlanner.evaluate(stalled, beginner()).decisions.single().type)
        val peak = AutoCoachPlanner.evaluate(stalled, inBlock(BlockPhase.PEAK, target = 0))
        assertTrue(peak.decisions.toString(), peak.decisions.none { it.type == "swap" || it.type == "rep_shift" })
    }

    @Test
    fun deloadWeek_withNoDeloadRunning_proposesTheDeloadItself() {
        // The block advanced into its deload week but nothing regenerated the program: the pass
        // must serve the week rather than propose more sets during it (the audit's repro).
        val r = AutoCoachPlanner.evaluate(earningMore(), inBlock(BlockPhase.DELOAD))
        assertEquals(CoachPassStatus.SHADOW, r.status)
        assertEquals(listOf("deload"), r.decisions.map { it.type })
    }

    @Test
    fun deloadWeek_withTheDeloadAlreadyRunning_holdsWithNoChanges() {
        val r = AutoCoachPlanner.evaluate(
            earningMore().copy(prefs = PrefsSnap(lastDeloadAppliedMs = now)),
            inBlock(BlockPhase.DELOAD)
        )
        assertEquals(CoachPassStatus.HOLD, r.status)
        assertTrue(r.holdReason!!, "Deload week" in r.holdReason!!)
        assertTrue(r.decisions.isEmpty())
    }
}
