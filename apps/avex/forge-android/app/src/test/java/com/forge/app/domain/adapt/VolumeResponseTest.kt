package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.Program
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The shared volume-response model (H-06). Strength change must be read WITHIN a lift and only
 * then aggregated by muscle-week; the exercise mix of a week is never a strength signal.
 * Fixtures: UTC, one bout per lift per ISO week, weeks 7 days apart so every bout lands in its
 * own Monday bucket.
 */
class VolumeResponseTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 200 * day

    private fun sets(weight: Double, n: Int) = List(n) { i ->
        LoggedSet(
            loggedExerciseId = 1, setIndex = i, weightText = "$weight",
            weightLb = weight, reps = 10, completedAt = 0
        )
    }

    /** One bout on ISO week [week] (0-based) — [weight] lb for [setCount] sets of 10. */
    private fun bout(week: Int, weight: Double, setCount: Int) = ExerciseBout(
        sessionStartedAt = (30 + week * 7) * day, effort = null, hitFullTarget = true,
        skipped = false, swappedName = null, sets = sets(weight, setCount)
    )

    private fun slot(id: String, muscle: MuscleGroup = MuscleGroup.CHEST) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(history: Map<String, List<ExerciseBout>>) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("upper-a", "Upper A", listOf(slot("bench"), slot("fly")))),
        sessions = emptyList(), exerciseHistory = history, prefs = PrefsSnap()
    )

    private fun analyse(history: Map<String, List<ExerciseBout>>) =
        VolumeResponse.analyse(snapshot(history), minWeeks = 8, minPerTier = 3)[MuscleGroup.CHEST]

    // ── The audit scenario: exercise mix is not physiology ─────────────────────

    @Test
    fun mixedLiftWeeks_withNeitherLiftMoving_readAsZeroResponse() {
        // High-volume weeks: flat 300 lb bench (4 sets) + flat 50 lb fly (3 sets) = 7 sets.
        // Low-volume weeks: the same flat fly alone = 3 sets. Alternating, eight weeks.
        val bench = (0 until 8 step 2).map { bout(it, 300.0, 4) }
        val fly = (0 until 8).map { bout(it, 50.0, 3) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        // The bench is only ever in one of any two consecutive weeks, so it is never compared; the
        // fly is compared with itself and hasn't moved. Both tiers read exactly zero.
        assertEquals(0.0, r!!.highAvgPct, 1e-9)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
        assertEquals(0.0, r.gapPct, 1e-9)
        assertEquals(4, r.highWeeks)
        assertEquals(3, r.lowWeeks)
        assertEquals(5, r.splitSets)
    }

    @Test
    fun reversedLiftMix_withNeitherLiftMoving_readsAsZeroResponse() {
        // Now the heavy lift sits in the LOW-volume weeks: 8 sets of fly alone vs 2 bench + 1 fly.
        // The old max-e1RM read swung the other way here and raised the cap; this reads zero too.
        val fly = (0 until 8).map { bout(it, 50.0, if (it % 2 == 0) 8 else 1) }
        val bench = (1 until 8 step 2).map { bout(it, 300.0, 2) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        assertEquals(0.0, r!!.gapPct, 1e-9)
    }

    @Test
    fun liftsThatNeverShareAWeek_areNeverCompared() {
        // Bench on even weeks, fly on odd weeks, forever. No lift is ever read in two consecutive
        // weeks, so there is no comparable change at all — and no response, however many weeks.
        val bench = (0 until 12 step 2).map { bout(it, 300.0, 4) }
        val fly = (1 until 12 step 2).map { bout(it, 50.0, 4) }
        assertNull(analyse(mapOf("bench" to bench, "fly" to fly)))
    }

    // ── Slot-keyed history, joined to the assembler that produces it ──────────

    /**
     * The half the two suites proved separately and never together.
     *
     * [SnapshotAssembler] files history by the program SLOT — deliberately, so Coach can target a
     * plan row whatever was performed in it — and this used to read that slot key as a lift. Swap a
     * fly into a bench slot for one week and the "lift" collapses 83% and then recovers 500%,
     * numbers with no physiology in them at all, and the muscle's volume verdict and the Stats
     * insight beside it were computed from exactly those.
     *
     * So the fixture is a real swapped `LoggedExercise` row through the real assembler: a slot that
     * holds a dead-flat 300 lb lift for ten weeks, except week 4, where the user swapped in a 50 lb
     * fly. The true response is zero, because nothing moved.
     */
    @Test
    fun aSwappedWeekIsNotReadAsThatSlotsLiftCollapsingAndRecovering() {
        val seedDay = Program.seedDays.first()
        val slot = seedDay.exercises.first()
        val week = 7 * day

        val sessions = ArrayList<Session>()
        val les = ArrayList<LoggedExercise>()
        val setRows = ArrayList<LoggedSet>()
        for (w in 0 until 10) {
            val id = (w + 1).toLong()
            sessions += Session(id = id, dayKey = seedDay.key, startedAt = 30 * day + w * week,
                finishedAt = 30 * day + w * week + 1)
            val swapped = w == 4
            les += LoggedExercise(
                id = id, sessionId = id, orderIndex = 0,
                exerciseId = if (swapped) "db-fly" else slot.id,
                slotId = if (swapped) slot.id else null,
                swappedName = if (swapped) "DB Fly" else null
            )
            // Volume alternates so both tiers fill; the swap week keeps its week's set count so the
            // only thing that changes across it is WHICH lift was performed.
            val setCount = if (w % 2 == 0) 8 else 4
            repeat(setCount) { i ->
                setRows += LoggedSet(
                    id = id * 100 + i, loggedExerciseId = id, setIndex = i,
                    weightText = "x", weightLb = if (swapped) 50.0 else 300.0,
                    reps = 10, completedAt = 0L
                )
            }
        }

        val snap = SnapshotAssembler.assemble(
            nowMs = 400 * day,
            program = listOf(seedDay),
            swapCandidateIds = { emptyList() },
            sessions = sessions,
            loggedExercises = les,
            loggedSets = setRows,
            prefs = PrefsSnap(),
            zoneId = java.time.ZoneId.of("UTC")
        )

        // The bout is filed under the slot, as every engine consumer needs...
        assertEquals(10, snap.exerciseHistory.getValue(slot.id).size)
        // ...and still says which lift was actually performed in it.
        assertEquals("db-fly", snap.exerciseHistory.getValue(slot.id)[4].performedExerciseId)

        val r = VolumeResponse.analyse(snap, minWeeks = 8, minPerTier = 3)[slot.muscle]
        assertNotNull("ten trained weeks in both tiers is enough to analyse", r)
        assertEquals(
            "nothing moved, so neither tier may show a response",
            0.0, r!!.gapPct, 1e-9
        )
        assertEquals(0.0, r.highAvgPct, 1e-9)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
    }

    // ── A genuine within-lift change is still read ─────────────────────────────

    @Test
    fun withinLiftGain_afterHighVolumeWeeks_isAPositiveResponse() {
        // One lift, 8 sets on even weeks and 4 on odd; each high week is followed by ~+9%, each
        // low week by a flat week.
        val plan = listOf(8 to 50.0, 4 to 55.0, 8 to 55.0, 4 to 60.0, 8 to 60.0, 4 to 65.0, 8 to 65.0, 4 to 70.0, 8 to 70.0)
        val bench = plan.mapIndexed { i, (sets, w) -> bout(i, w, sets) }
        val r = analyse(mapOf("bench" to bench))
        assertNotNull(r)
        assertTrue(r!!.highAvgPct > 7.0)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
        assertTrue(r.gapPct > 7.0)
    }

    @Test
    fun withinLiftGain_afterLowVolumeWeeks_isANegativeResponse() {
        val plan = listOf(4 to 50.0, 8 to 55.0, 4 to 55.0, 8 to 60.0, 4 to 60.0, 8 to 65.0, 4 to 65.0, 8 to 70.0, 4 to 70.0)
        val bench = plan.mapIndexed { i, (sets, w) -> bout(i, w, sets) }
        val r = analyse(mapOf("bench" to bench))
        assertNotNull(r)
        assertTrue(r!!.gapPct < -7.0)
    }

    @Test
    fun changeIsNormalisedPerLift_soAHeavyLiftDoesNotOutweighALightOne() {
        // Both lifts present every week. Bench flat at 300; fly +10% after every high week. The
        // response is the mean of per-lift PERCENT changes, so the fly's move counts at its own scale
        // (10%) averaged with the bench's zero — not drowned by 300 lb of raw e1RM.
        val bench = (0 until 9).map { bout(it, 300.0, if (it % 2 == 0) 5 else 2) }
        val flyWeights = listOf(50.0, 55.0, 55.0, 60.5, 60.5, 66.55, 66.55, 73.205, 73.205)
        val fly = flyWeights.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 3 else 1) }
        val r = analyse(mapOf("bench" to bench, "fly" to fly))
        assertNotNull(r)
        assertEquals(5.0, r!!.highAvgPct, 1e-6)   // mean of (0%, +10%)
        assertEquals(0.0, r.lowAvgPct, 1e-9)
    }

    // ── Gates ──────────────────────────────────────────────────────────────────

    @Test
    fun belowTheWeekGate_saysNothing() {
        val bench = (0 until 7).map { bout(it, 50.0 + it * 5, if (it % 2 == 0) 8 else 4) }
        assertNull(analyse(mapOf("bench" to bench)))
    }

    @Test
    fun aLiftOutsideTheProgram_isIgnored() {
        val bench = (0 until 9).map { bout(it, 50.0 + it * 5, if (it % 2 == 0) 8 else 4) }
        assertTrue(VolumeResponse.analyse(snapshot(mapOf("not-in-program" to bench)), 8, 3).isEmpty())
    }
}
