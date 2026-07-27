package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * System 1 of the adaptation engine. Covers the acceptance triad for every system —
 * cold start (sparse data → silence), happy path, conflicting signals — plus the
 * unit-awareness rules and determinism.
 */
class ProgressionAdvisorTest {

    private fun set(
        weight: Double?,
        reps: Int,
        rpe: Double? = null,
        assisted: Boolean = false,
        idx: Int = 0
    ) = LoggedSet(
        loggedExerciseId = 1L, setIndex = idx,
        weightText = weight?.toString() ?: "BW", weightLb = weight,
        reps = reps, completedAt = 0L, rpe = rpe, isAssisted = assisted
    )

    private fun suggest(
        sets: List<LoggedSet>,
        effort: EffortRating? = null,
        reps: String = "8-10",
        unit: ExerciseUnit = ExerciseUnit.DUMBBELL,
        intensity: IntensityIntent = IntensityIntent.NORMAL,
        readiness: Recommendation.ReadinessScale? = null,
        dbMaxLb: Double? = null,
        fastStep: Boolean = false,
        consolidate: Boolean = false
    ) = ProgressionAdvisor.suggestNextLoad(
        exerciseId = "ua1", exerciseName = "DB Bench Press",
        prevSets = sets, prevEffort = effort,
        repsText = reps, unit = unit, plateLb = 15.0,
        intensity = intensity, readiness = readiness, dbMaxLb = dbMaxLb,
        fastStep = fastStep, consolidate = consolidate
    )

    private fun lowReadiness(percent: Int = -4) =
        Recommendation.ReadinessScale(percent, "rough day", Confidence.MEDIUM)

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun coldStart_noPreviousSets_staysSilent() {
        assertNull(suggest(emptyList(), EffortRating.JUST_RIGHT))
    }

    @Test
    fun bodyweightOnlyHistory_staysSilent() {
        assertNull(suggest(listOf(set(null, 12)), EffortRating.JUST_RIGHT))
    }

    @Test
    fun bodyweightUnit_neverSuggests() {
        assertNull(suggest(listOf(set(45.0, 10)), EffortRating.EASY, unit = ExerciseUnit.BODYWEIGHT))
    }

    // ── Bodyweight rep progression (CO5) ─────────────────────────────────────────

    private fun suggestReps(
        sets: List<LoggedSet>,
        effort: EffortRating? = null,
        reps: String = "8-12"
    ) = ProgressionAdvisor.suggestNextReps(
        exerciseId = "pull1", exerciseName = "Pull-up",
        prevSets = sets, prevEffort = effort, repsText = reps
    )

    @Test
    fun bodyweightReps_allSetsAtRangeTop_okEffort_suggestsOneMore() {
        val s = suggestReps(listOf(set(null, 12), set(null, 12, idx = 1)), EffortRating.JUST_RIGHT)
        assertNotNull(s)
        assertEquals(13, s!!.targetReps)
        assertEquals("8-12", s.fromReps)
    }

    @Test
    fun bodyweightReps_midRange_staysSilent() {
        // 9 reps in an 8-12 range: still climbing within the range, no nudge yet.
        assertNull(suggestReps(listOf(set(null, 9)), EffortRating.JUST_RIGHT))
    }

    @Test
    fun bodyweightReps_brutalEffort_staysSilent() {
        assertNull(suggestReps(listOf(set(null, 12)), EffortRating.BRUTAL))
    }

    @Test
    fun bodyweightReps_noHistory_staysSilent() {
        assertNull(suggestReps(emptyList(), EffortRating.JUST_RIGHT))
    }

    @Test
    fun bodyweightReps_oneSetBelowTop_staysSilent() {
        // Double progression: EVERY working set must clear the top before adding a rep.
        assertNull(suggestReps(listOf(set(null, 12), set(null, 10, idx = 1)), EffortRating.JUST_RIGHT))
    }

    @Test
    fun bodyweightReps_allAssisted_staysSilent() {
        // Every set band-assisted = no unassisted target. No fallback to assisted sets: stay silent,
        // exactly as the weighted chip does (would otherwise nudge +1 off easier assisted reps).
        assertNull(
            suggestReps(
                listOf(set(null, 12, assisted = true), set(null, 12, assisted = true, idx = 1)),
                EffortRating.JUST_RIGHT
            )
        )
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun allTopSetsAtRangeTop_moderateEffort_suggestsOneStepUp() {
        val s = suggest(listOf(set(45.0, 10), set(45.0, 10, idx = 1)), EffortRating.JUST_RIGHT)
        assertNotNull(s)
        assertEquals(47.5, s!!.targetWeightLb, 0.0001)
        assertEquals(2.5, s.deltaLb, 0.0001)
        assertEquals("47.5", s.inputText)
        assertEquals("hit top of range", s.reason)
    }

    @Test
    fun unratedExercise_topOfRange_stillProgresses() {
        // No rating = no objection — matches the old chip's behavior.
        assertNotNull(suggest(listOf(set(45.0, 10)), effort = null))
    }

    @Test
    fun lighterBackoffSetsDoNotBlockProgression() {
        // Only the top-weight sets must reach the range top; a lighter drop set may sit below it.
        val s = suggest(listOf(set(45.0, 10), set(40.0, 6, idx = 1)), EffortRating.JUST_RIGHT)
        assertNotNull(s)
        assertEquals(47.5, s!!.targetWeightLb, 0.0001)
    }

    @Test
    fun oneTopSetShortOfRangeTop_showsKeepGoingCue_notSilence() {
        // Below the rep-range top at this weight: instead of vanishing (which reads as "chip broken"),
        // surface a same-weight "keep going / reach the top" cue that teaches double-progression.
        val s = suggest(listOf(set(45.0, 10), set(45.0, 8, idx = 1)), EffortRating.JUST_RIGHT)
        assertNotNull(s)
        assertEquals(45.0, s!!.targetWeightLb, 0.0001)
        assertEquals(0.0, s.deltaLb, 0.0001)
        assertTrue("reason should explain working up: ${s.reason}", "reach" in s.reason)
    }

    @Test
    fun belowRangeTop_singleSet_showsKeepGoingCue() {
        // First time at a new weight, mid-range reps: still surfaces the explicit "keep this weight" cue.
        val s = suggest(listOf(set(45.0, 8)), EffortRating.JUST_RIGHT)
        assertNotNull(s)
        assertEquals(45.0, s!!.targetWeightLb, 0.0001)
        assertEquals(0.0, s.deltaLb, 0.0001)
    }

    @Test
    fun belowRangeTop_amrapHasNoTop_staysSilent() {
        // AMRAP / timed targets have no parseable range top to "reach", so no working-up cue fires.
        assertNull(suggest(listOf(set(45.0, 8)), EffortRating.JUST_RIGHT, reps = "AMRAP"))
    }

    @Test
    fun brutalRating_backsOffOneStep() {
        val s = suggest(listOf(set(45.0, 8)), EffortRating.BRUTAL)
        assertNotNull(s)
        assertEquals(42.5, s!!.targetWeightLb, 0.0001)
        assertEquals("last rated brutal", s.reason)
    }

    @Test
    fun hardButNotBrutal_holds() {
        assertNull(suggest(listOf(set(45.0, 10)), EffortRating.HARD))
    }

    // ── Conflicting signals ────────────────────────────────────────────────────

    @Test
    fun conflict_topOfRangeButBrutal_backsOffInsteadOfProgressing() {
        val s = suggest(listOf(set(45.0, 10), set(45.0, 10, idx = 1)), EffortRating.BRUTAL)
        assertNotNull(s)
        assertTrue("brutal must win over top-of-range", s!!.deltaLb < 0)
    }

    @Test
    fun conflict_highRpeOutranksEasyRating() {
        val s = suggest(listOf(set(45.0, 10, rpe = 9.5)), EffortRating.EASY)
        assertNotNull(s)
        assertTrue("RPE 9.5 must force a back-off despite the EASY rating", s!!.deltaLb < 0)
        assertTrue(s.reason.contains("RPE"))
    }

    @Test
    fun conflict_lowRpeOutranksBrutalRating() {
        val s = suggest(listOf(set(45.0, 10, rpe = 7.0)), EffortRating.BRUTAL)
        assertNotNull(s)
        assertEquals(2.5, s!!.deltaLb, 0.0001)
    }

    @Test
    fun midRangeRpe_holds() {
        // 8.5 is neither "room to progress" (≤8) nor brutal (≥9.5) — silence, not a guess.
        assertNull(suggest(listOf(set(45.0, 10, rpe = 8.5)), EffortRating.JUST_RIGHT))
    }

    @Test
    fun assistedSetsAreExcludedFromTheBaseline() {
        val s = suggest(
            listOf(set(50.0, 10, assisted = true), set(45.0, 10, idx = 1)),
            EffortRating.JUST_RIGHT
        )
        assertNotNull(s)
        assertEquals("baseline must be the unassisted 45", 47.5, s!!.targetWeightLb, 0.0001)
    }

    // ── Unit awareness ─────────────────────────────────────────────────────────

    @Test
    fun plates_topOfRange_addsOneWholePlateWithPlateCountText() {
        val s = suggest(listOf(set(45.0, 12)), EffortRating.JUST_RIGHT, reps = "10-12", unit = ExerciseUnit.PLATES)
        assertNotNull(s)
        assertEquals(60.0, s!!.targetWeightLb, 0.0001)
        assertEquals(15.0, s.deltaLb, 0.0001)
        assertEquals("4 plates", s.inputText)
    }

    @Test
    fun plates_backoffToSinglePlate_saysOnePlate() {
        val s = suggest(listOf(set(30.0, 8)), EffortRating.BRUTAL, reps = "10-12", unit = ExerciseUnit.PLATES)
        assertNotNull(s)
        assertEquals("1 plate", s!!.inputText)
    }

    @Test
    fun plates_lightIntensity_suppressesTheIncrease() {
        assertNull(
            suggest(
                listOf(set(45.0, 12)), EffortRating.JUST_RIGHT,
                reps = "10-12", unit = ExerciseUnit.PLATES, intensity = IntensityIntent.LIGHT
            )
        )
    }

    @Test
    fun plates_brutalAtOnePlate_staysSilentRatherThanSuggestZero() {
        assertNull(suggest(listOf(set(15.0, 10)), EffortRating.BRUTAL, unit = ExerciseUnit.PLATES))
    }

    @Test
    fun plates_offGridWeight_fallsBackToExplicitLbText() {
        // 50 lb isn't a whole number of 15 lb plates; "65 lb" parses as pounds even on a
        // PLATES exercise (WeightParser's lb-suffix rule), so the apply path stays correct.
        val s = suggest(listOf(set(50.0, 12)), EffortRating.JUST_RIGHT, reps = "10-12", unit = ExerciseUnit.PLATES)
        assertNotNull(s)
        assertEquals(65.0, s!!.targetWeightLb, 0.0001)
        assertEquals("65 lb", s.inputText)
    }

    @Test
    fun dumbbell_lightIntensity_scalesDownAndSnapsToGrid() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, intensity = IntensityIntent.LIGHT)
        assertNotNull(s)
        assertEquals(42.5, s!!.targetWeightLb, 0.0001) // (45 + 2.5) × 0.9 = 42.75 → grid 42.5
        assertTrue(s.reason.contains("intensity adjusted"))
    }

    // ── Readiness autoregulation (System 6 feeding System 1) ───────────────────

    @Test
    fun lowReadiness_easesTheSuggestionWithAReason() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, readiness = lowReadiness(-4))
        assertNotNull(s)
        assertEquals(45.0, s!!.targetWeightLb, 0.0001) // (45 + 2.5) × 0.96 = 45.6 → grid 45
        assertTrue(s.reason.contains("eased 4% — readiness low"))
    }

    @Test
    fun explicitIntensityPickOutranksReadiness() {
        // User chose HARD; a low readiness scale must not override their intent.
        val s = suggest(
            listOf(set(45.0, 10)), EffortRating.JUST_RIGHT,
            intensity = IntensityIntent.HARD, readiness = lowReadiness(-5)
        )
        assertNotNull(s)
        assertEquals(47.5, s!!.targetWeightLb, 0.0001) // (45 + 2.5) × 1.05 = 49.875 → grid 47.5
        assertTrue(s.reason.contains("intensity adjusted"))
        assertTrue(!s.reason.contains("readiness"))
    }

    @Test
    fun lowReadiness_suppressesPlateIncreases() {
        // A whole plate is too big a step up on a low-readiness day.
        assertNull(
            suggest(
                listOf(set(45.0, 12)), EffortRating.JUST_RIGHT,
                reps = "10-12", unit = ExerciseUnit.PLATES, readiness = lowReadiness(-3)
            )
        )
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSameSuggestion() {
        val sets = listOf(set(45.0, 10), set(45.0, 10, idx = 1))
        assertEquals(
            suggest(sets, EffortRating.JUST_RIGHT),
            suggest(sets, EffortRating.JUST_RIGHT)
        )
    }

    // ── Plateau ladder (snapshot path) ─────────────────────────────────────────

    private fun slot(
        id: String = "ua1",
        unit: ExerciseUnit = ExerciseUnit.DUMBBELL,
        reps: String = "8-10",
        swaps: List<String> = listOf("alt-1", "alt-2", "alt-3", "alt-4")
    ) = ProgramSlotSnap(
        exerciseId = id, name = "DB Bench Press", muscle = MuscleGroup.CHEST,
        unit = unit, tags = emptyList(), targetSets = 3, repsText = reps,
        swapCandidateIds = swaps
    )

    private fun bout(weight: Double, reps: Int, effort: EffortRating? = EffortRating.JUST_RIGHT, at: Long = 0L) =
        ExerciseBout(
            sessionStartedAt = at, effort = effort, hitFullTarget = false,
            skipped = false, swappedName = null, sets = listOf(set(weight, reps))
        )

    private fun snapshot(bouts: List<ExerciseBout>, s: ProgramSlotSnap = slot()) = AdaptationSnapshot(
        nowMs = 0L,
        program = listOf(ProgramDaySnap("upper-a", "Upper A", listOf(s))),
        sessions = emptyList(),
        exerciseHistory = mapOf(s.exerciseId to bouts),
        prefs = PrefsSnap()
    )

    /** [n] flat bouts after an improving baseline → stall length of exactly [n]. */
    private fun stalledBouts(n: Int, effort: EffortRating = EffortRating.JUST_RIGHT): List<ExerciseBout> =
        listOf(bout(45.0, 8, at = 0)) + (1..n).map { i -> bout(45.0, 8, effort, at = i.toLong()) }

    @Test
    fun plateau_coldStart_fourBoutsIsNotEnough() {
        // 4 flat bouts = a stall of 3 — below the 4-session gate. Silence.
        assertTrue(ProgressionAdvisor.evaluate(snapshot(stalledBouts(3))).isEmpty())
    }

    @Test
    fun plateau_fourStalledAtLowEffort_suggestsMicroLoad() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(4)))
        assertEquals(1, recs.size)
        val r = recs.single() as Recommendation.WeightChange
        assertTrue("low-effort stall should nudge upward", r.deltaLb > 0)
        assertEquals(Confidence.MEDIUM, r.confidence)
        assertTrue(r.reason.contains("4 sessions"))
    }

    @Test
    fun plateau_fourStalledAtHighEffort_suggestsResetDown() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(4, EffortRating.BRUTAL)))
        val r = recs.single() as Recommendation.WeightChange
        assertTrue("high-effort stall should reset down", r.deltaLb < 0)
        assertEquals(40.0, r.targetWeightLb, 0.0001) // 45 × 0.9 = 40.5 → grid 40
        assertTrue(r.reason.contains("build back up"))
    }

    @Test
    fun plateau_conflictingSignal_recentImprovementResetsTheStall() {
        val bouts = stalledBouts(5).toMutableList()
        // An e1RM jump one bout from the end — the stall counter restarts there.
        bouts[bouts.size - 2] = bout(50.0, 8, at = 98L)
        assertTrue(ProgressionAdvisor.evaluate(snapshot(bouts)).isEmpty())
    }

    @Test
    fun plateau_sixStalled_escalatesToRepRangeShift_highConfidence() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(6)))
        val r = recs.single() as Recommendation.RepRangeShift
        assertEquals("8-10", r.fromReps)
        assertEquals("12-15", r.toReps) // low-rep range shifts up
        assertEquals(Confidence.HIGH, r.confidence)
    }

    @Test
    fun plateau_highRepRangeShiftsDownInstead() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(6), slot(reps = "12-15")))
        val r = recs.single() as Recommendation.RepRangeShift
        assertEquals("8-10", r.toReps)
    }

    @Test
    fun plateau_eightStalled_escalatesToVariationSwap_cappedCandidates() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(8)))
        val r = recs.single() as Recommendation.VariationSwap
        assertEquals(listOf("alt-1", "alt-2", "alt-3"), r.candidateIds)
        assertEquals(Confidence.HIGH, r.confidence)
    }

    @Test
    fun plateau_eightStalledWithNoSwapCandidates_fallsBackToRepShift() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(8), slot(swaps = emptyList())))
        assertTrue(recs.single() is Recommendation.RepRangeShift)
    }

    @Test
    fun plateau_skippedBoutsDoNotCount() {
        val bouts = stalledBouts(4).map { it.copy(skipped = true) }
        assertTrue(ProgressionAdvisor.evaluate(snapshot(bouts)).isEmpty())
    }

    @Test
    fun plateau_bodyweightSlotIsSkipped() {
        val recs = ProgressionAdvisor.evaluate(snapshot(stalledBouts(8), slot(unit = ExerciseUnit.BODYWEIGHT)))
        assertTrue(recs.isEmpty())
    }

    @Test
    fun plateau_evaluateIsDeterministic() {
        val snap = snapshot(stalledBouts(6))
        assertEquals(ProgressionAdvisor.evaluate(snap), ProgressionAdvisor.evaluate(snap))
    }

    // ── Dumbbell ceiling (auto-coach Phase 0) ──────────────────────────────────

    @Test
    fun dbCeiling_atHeaviestDumbbell_anchorsWeightAndSuggestsReps() {
        val s = suggest(listOf(set(25.0, 10)), EffortRating.JUST_RIGHT, dbMaxLb = 25.0)
        assertNotNull(s)
        assertEquals(25.0, s!!.targetWeightLb, 0.0001)
        assertEquals(0.0, s.deltaLb, 0.0001)
        assertTrue("reason should pivot to reps: ${s.reason}", "reps" in s.reason)
    }

    @Test
    fun dbCeiling_belowCeiling_progressesNormally() {
        val s = suggest(listOf(set(20.0, 10)), EffortRating.JUST_RIGHT, dbMaxLb = 25.0)
        assertEquals(22.5, s!!.targetWeightLb, 0.0001)
        assertEquals("hit top of range", s.reason)
    }

    @Test
    fun dbCeiling_staleSetting_historyHeavier_isNotCapped() {
        // A logged set above the ceiling means the setting is stale — trust the data.
        val s = suggest(listOf(set(30.0, 10)), EffortRating.JUST_RIGHT, dbMaxLb = 25.0)
        assertEquals(32.5, s!!.targetWeightLb, 0.0001)
    }

    @Test
    fun dbCeiling_backOffIsUnaffected() {
        val s = suggest(listOf(set(25.0, 10)), EffortRating.BRUTAL, dbMaxLb = 25.0)
        assertEquals(22.5, s!!.targetWeightLb, 0.0001)
    }

    @Test
    fun dbCeiling_platesAreUnaffected() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, unit = ExerciseUnit.PLATES, dbMaxLb = 25.0)
        assertEquals(60.0, s!!.targetWeightLb, 0.0001)
    }

    @Test
    fun barbellWeight_ignoresTheDumbbellCeiling() {
        // The heaviest-dumbbell ceiling is for dumbbells only — a WEIGHT (barbell/machine) lift
        // keeps progressing past it instead of anchoring.
        val s = suggest(listOf(set(25.0, 10)), EffortRating.JUST_RIGHT, unit = ExerciseUnit.WEIGHT, dbMaxLb = 25.0)
        assertNotNull(s)
        assertEquals(27.5, s!!.targetWeightLb, 0.0001)
        assertEquals("hit top of range", s.reason)
    }

    // ── Step calibration (auto-coach Phase 2) ──────────────────────────────────

    @Test
    fun fastStep_doublesTheDumbbellIncrement() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, fastStep = true)
        assertEquals(50.0, s!!.targetWeightLb, 0.0001)
        assertTrue("reason should mention calibration: ${s.reason}", "calibrated" in s.reason)
    }

    @Test
    fun consolidate_holdsTheWeightInsteadOfProgressing() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, consolidate = true)
        assertEquals(45.0, s!!.targetWeightLb, 0.0001)
        assertEquals(0.0, s.deltaLb, 0.0001)
        assertTrue("reason should explain the hold: ${s.reason}", "consolidate" in s.reason)
    }

    @Test
    fun consolidate_doesNotAffectBackOffs() {
        val s = suggest(listOf(set(45.0, 10)), EffortRating.BRUTAL, consolidate = true)
        assertEquals(42.5, s!!.targetWeightLb, 0.0001)
    }

    @Test
    fun fastStep_stillRespectsTheDbCeiling() {
        // 45 + 5 (fast step) = 50, but the heaviest DB is 47.5 → capped.
        val s = suggest(listOf(set(45.0, 10)), EffortRating.JUST_RIGHT, fastStep = true, dbMaxLb = 47.5)
        assertEquals(47.5, s!!.targetWeightLb, 0.0001)
    }

    // ── Session types that aren't ordinary training (A1) ───────────────────────

    @Test
    fun techniqueBouts_doNotCountTowardAStall() {
        // A stall of 4 would fire — but three of those bouts were deliberately-light technique
        // days, so only 2 training bouts remain and the whole slot drops below the gate.
        val bouts = stalledBouts(4).mapIndexed { i, b ->
            if (i in 2..4) b.copy(sessionType = "technique") else b
        }
        assertTrue(ProgressionAdvisor.evaluate(snapshot(bouts)).isEmpty())
    }

    @Test
    fun testDayTopSingle_doesNotBecomeTheAnchorWeight() {
        // Four flat 45 lb bouts (a real stall) plus a test-day single at 95. If the test day
        // counted, it would both break the stall and anchor the next prescription at 95.
        val bouts = stalledBouts(4) + bout(95.0, 1, EffortRating.BRUTAL, at = 99).copy(sessionType = "test")
        val recs = ProgressionAdvisor.evaluate(snapshot(bouts))
        assertEquals(1, recs.size)
        val r = recs.single() as Recommendation.WeightChange
        assertEquals("anchor must stay on the working weight", 45.0, r.previousMaxLb, 0.0001)
    }

    @Test
    fun firstBackRamp_isNotAStall() {
        val bouts = stalledBouts(4).map { it.copy(sessionType = "first_back") }
        assertTrue(ProgressionAdvisor.evaluate(snapshot(bouts)).isEmpty())
    }

    @Test
    fun deloadBouts_stillCount_becauseADeloadIsPlannedTraining() {
        val bouts = stalledBouts(4).map { it.copy(sessionType = "deload") }
        assertEquals(1, ProgressionAdvisor.evaluate(snapshot(bouts)).size)
    }

    // ── Phase-aware stall interpretation (A2) ──────────────────────────────────

    /** [count] weigh-ins over 5 weeks drifting [perWeek] lb/week — enough to pass WeightPhase's gates. */
    private fun weighIns(perWeek: Double, count: Int = 10): List<com.forge.app.data.db.entities.BodyweightEntry> {
        val day = 24L * 60 * 60 * 1000
        return (0 until count).map { i ->
            com.forge.app.data.db.entities.BodyweightEntry(
                dateKey = "d$i",
                weightLb = 200.0 + perWeek * (i * 4) / 7.0,
                recordedAt = i * 4L * day
            )
        }
    }

    private fun snapshotWithWeight(
        bouts: List<ExerciseBout>,
        weight: List<com.forge.app.data.db.entities.BodyweightEntry>
    ): AdaptationSnapshot {
        val s = slot()
        return AdaptationSnapshot(
            nowMs = 40L * 24 * 60 * 60 * 1000,
            program = listOf(ProgramDaySnap("upper-a", "Upper A", listOf(s))),
            sessions = emptyList(),
            exerciseHistory = mapOf(s.exerciseId to bouts),
            bodyweight = weight,
            prefs = PrefsSnap()
        )
    }

    @Test
    fun flatLiftWhileCutting_isNotEscalated() {
        // The exact fixture that fires a plateau in maintenance: held e1RM for 4 bouts.
        val bouts = stalledBouts(4)
        assertEquals("control: this is a plateau at stable weight", 1, ProgressionAdvisor.evaluate(snapshot(bouts)).size)
        val cutting = snapshotWithWeight(bouts, weighIns(perWeek = -1.0))
        assertTrue(
            "holding strength in a deficit is a win, not a plateau",
            ProgressionAdvisor.evaluate(cutting).isEmpty()
        )
    }

    @Test
    fun decliningLiftWhileCutting_stillSpeaks() {
        // Holding is fine; sliding is not — the suppression must not silence a real regression.
        val declining = listOf(bout(50.0, 8, at = 0)) + (1..4).map { i -> bout(42.5, 8, at = i.toLong()) }
        val cutting = snapshotWithWeight(declining, weighIns(perWeek = -1.0))
        assertEquals(1, ProgressionAdvisor.evaluate(cutting).size)
    }

    @Test
    fun flatLiftWhileBulking_isStillAPlateau() {
        val bulking = snapshotWithWeight(stalledBouts(4), weighIns(perWeek = 1.0))
        assertEquals(1, ProgressionAdvisor.evaluate(bulking).size)
    }

    @Test
    fun sparseWeighIns_changeNothing() {
        // Below WeightPhase's gates the phase is UNKNOWN, so behavior is exactly pre-A2.
        val unknown = snapshotWithWeight(stalledBouts(4), weighIns(perWeek = -1.0, count = 3))
        assertEquals(1, ProgressionAdvisor.evaluate(unknown).size)
    }

    // ── Effort model on the plateau ladder (A1) ────────────────────────────────

    @Test
    fun stallWithSetsLoggedToFailure_readsAsHighEffort_andResetsDown() {
        // No RPE and no BRUTAL rating anywhere — only the to-failure checkbox. Pre-A1 this read
        // as a low-effort stall and nudged the weight UP.
        val failureSets = listOf(
            LoggedSet(loggedExerciseId = 1, setIndex = 0, weightText = "45", weightLb = 45.0,
                reps = 8, completedAt = 0, toFailure = true)
        )
        val bouts = listOf(bout(45.0, 8, at = 0)) + (1..4).map { i ->
            ExerciseBout(sessionStartedAt = i.toLong(), effort = null, hitFullTarget = false,
                skipped = false, swappedName = null, sets = failureSets)
        }
        val r = ProgressionAdvisor.evaluate(snapshot(bouts)).single() as Recommendation.WeightChange
        assertTrue("grinding to failure at a stall should reset down", r.deltaLb < 0)
    }
}
