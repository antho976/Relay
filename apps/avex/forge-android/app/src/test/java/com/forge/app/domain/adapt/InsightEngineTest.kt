package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * System 4 of the adaptation engine. Fixtures pin "now" to day 100 (UTC zone — hour
 * arithmetic in tests is then exact). Each test builds the minimal snapshot for its rule
 * and asserts by insight key, so unrelated rules firing can't break it.
 */
class InsightEngineTest {

    private val hour = 60L * 60 * 1000
    private val day = 24 * hour
    private val now = 100 * day

    private fun slot(
        id: String = "ua1",
        name: String = "DB Bench Press",
        muscle: MuscleGroup = MuscleGroup.CHEST,
        reps: String = "8-10"
    ) = ProgramSlotSnap(
        exerciseId = id, name = name, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = reps
    )

    private fun sets(weight: Double, n: Int = 3) = List(n) { i ->
        LoggedSet(
            loggedExerciseId = 1, setIndex = i, weightText = "$weight",
            weightLb = weight, reps = 10, completedAt = 0
        )
    }

    private fun bout(
        atDay: Int,
        weight: Double = 45.0,
        setCount: Int = 3,
        skipped: Boolean = false,
        swappedName: String? = null,
        effort: EffortRating? = EffortRating.JUST_RIGHT
    ) = ExerciseBout(
        sessionStartedAt = atDay * day, effort = effort, hitFullTarget = false,
        skipped = skipped, swappedName = swappedName, sets = sets(weight, setCount)
    )

    private fun snapshot(
        slots: List<ProgramSlotSnap> = listOf(slot()),
        history: Map<String, List<ExerciseBout>> = emptyMap(),
        sessions: List<Session> = emptyList(),
        moods: List<MoodEntry> = emptyList(),
        cardio: List<CardioEntry> = emptyList()
    ) = AdaptationSnapshot(
        nowMs = now, program = listOf(ProgramDaySnap("upper-a", "Upper A", slots)),
        sessions = sessions, exerciseHistory = history, moods = moods, cardio = cardio,
        prefs = PrefsSnap()
    )

    private fun keyOf(insights: List<Recommendation.Insight>, key: String) =
        insights.firstOrNull { it.key == key }

    private fun keyStartingWith(insights: List<Recommendation.Insight>, prefix: String) =
        insights.firstOrNull { it.key.startsWith(prefix) }

    // ── Cold start: a fresh install says nothing at all ───────────────────────

    @Test
    fun emptySnapshot_producesNoInsights() {
        assertTrue(InsightEngine.evaluate(snapshot()).isEmpty())
    }

    // ── Best time-of-day (ported #40, now gated) ──────────────────────────────

    @Test
    fun timeOfDay_firesOnlyAboveTheSetGate() {
        // 10 evening bouts × 3 sets = 30 sets — exactly at the gate.
        val evening = (0 until 10).map { i ->
            bout(atDay = 90 + i).copy(sessionStartedAt = (90 + i) * day + 18 * hour)
        }
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to evening)))
        assertNotNull(keyOf(fired, "timeofday"))
        assertTrue(keyOf(fired, "timeofday")!!.body.contains("evening"))

        // One bout fewer (27 sets) → silent.
        val sparse = evening.dropLast(1)
        assertNull(keyOf(InsightEngine.evaluate(snapshot(history = mapOf("ua1" to sparse))), "timeofday"))
    }

    // ── Most improved (ported #41, gate raised to 4 sessions) ─────────────────

    @Test
    fun mostImproved_reportsTheBiggestGain() {
        val rising = listOf(bout(80, 40.0), bout(85, 40.0), bout(90, 50.0), bout(95, 50.0))
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to rising)))
        val insight = keyOf(fired, "improved")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("DB Bench Press"))
        assertTrue(insight.body.contains("25%"))
    }

    @Test
    fun mostImproved_threeSessionsIsNotEnough() {
        val rising = listOf(bout(85, 40.0), bout(90, 45.0), bout(95, 50.0))
        assertNull(keyOf(InsightEngine.evaluate(snapshot(history = mapOf("ua1" to rising))), "improved"))
    }

    // ── Weekly muscle dominance (ported) ──────────────────────────────────────

    @Test
    fun muscleDominance_flagsAMuscleOverHalfTheWeeksVolume() {
        val slots = listOf(slot("ua1", "DB Bench Press", MuscleGroup.CHEST), slot("ua6", "Hammer Curl", MuscleGroup.BICEPS))
        val history = mapOf(
            "ua1" to (0 until 3).map { bout(97 + it, weight = 45.0) },  // 4050 lb of chest
            "ua6" to listOf(bout(97, weight = 5.0))                      // 150 lb of biceps
        )
        val fired = InsightEngine.evaluate(snapshot(slots = slots, history = history))
        val insight = keyOf(fired, "dominance")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("Chest"))
    }

    // ── Push/pull structural ratio ─────────────────────────────────────────────

    @Test
    fun pushPullRatio_flagsAPushHeavyMonth() {
        val slots = listOf(slot("ua1", "DB Bench Press", MuscleGroup.CHEST), slot("ub1", "DB Row", MuscleGroup.BACK))
        val history = mapOf(
            "ua1" to (0 until 10).map { bout(75 + it * 2) },  // 30 push sets
            "ub1" to (0 until 4).map { bout(75 + it * 2) }    // 12 pull sets
        )
        val fired = InsightEngine.evaluate(snapshot(slots = slots, history = history))
        val insight = keyOf(fired, "pushpull")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("push-heavy"))
    }

    @Test
    fun pushPullRatio_balancedTrainingStaysSilent() {
        val slots = listOf(slot("ua1", "DB Bench Press", MuscleGroup.CHEST), slot("ub1", "DB Row", MuscleGroup.BACK))
        val history = mapOf(
            "ua1" to (0 until 5).map { bout(80 + it * 2) },
            "ub1" to (0 until 5).map { bout(80 + it * 2) }
        )
        assertNull(keyOf(InsightEngine.evaluate(snapshot(slots = slots, history = history)), "pushpull"))
    }

    // ── Adherence ──────────────────────────────────────────────────────────────

    @Test
    fun mostSkipped_flagsAChronicallySkippedExercise() {
        val history = mapOf("ua1" to listOf(
            bout(80), bout(85, skipped = true), bout(90, skipped = true), bout(93), bout(96, skipped = true)
        ))
        val insight = keyStartingWith(InsightEngine.evaluate(snapshot(history = history)), "skip.")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("skipped 3 of the last 5"))
    }

    @Test
    fun repeatedSessionSwaps_suggestsMakingItPermanent() {
        val history = mapOf("ua1" to listOf(
            bout(80, swappedName = "Incline DB Press"), bout(85),
            bout(90, swappedName = "Incline DB Press"), bout(93),
            bout(96, swappedName = "Incline DB Press")
        ))
        val insight = keyStartingWith(InsightEngine.evaluate(snapshot(history = history)), "swap.")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("Incline DB Press"))
    }

    // ── Conflicting signals: sub-threshold fatigue is an insight, firing fatigue is NOT ──

    @Test
    fun recoverySignals_fireOnlyBelowTheDeloadThreshold() {
        // Effort inflation (+2) + sore (+1) = 3 → insight territory. Prior-window sessions
        // (days 74-83) sit inside [86-14, 86); window sessions (88-97) inside [86, 100].
        fun sessions() = (0 until 4).map {
            Session(it + 1L, "upper-a", (74 + it * 3) * day, (74 + it * 3) * day + hour, totalVolumeLb = 1100.0)
        } + (0 until 4).map {
            Session(it + 5L, "upper-a", (88 + it * 3) * day, (88 + it * 3) * day + hour, totalVolumeLb = 1000.0)
        }
        val brutalBouts = (0 until 6).map { bout(88 + it * 2, effort = EffortRating.BRUTAL, setCount = 1) }
        val sore = listOf(CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"))

        val sub = InsightEngine.evaluate(
            snapshot(history = mapOf("ua1" to brutalBouts), sessions = sessions(), cardio = sore)
        )
        assertNotNull(keyOf(sub, "recovery"))

        // Add low moods (+2 → score 5): the deload call owns it — the insight must yield.
        val lowMoods = listOf(
            MoodEntry(1, null, "upper-a", "drained", now - 1 * day),
            MoodEntry(2, null, "upper-a", "off", now - 3 * day),
            MoodEntry(3, null, "upper-a", "drained", now - 5 * day)
        )
        val firing = InsightEngine.evaluate(
            snapshot(history = mapOf("ua1" to brutalBouts), sessions = sessions(), cardio = sore, moods = lowMoods)
        )
        assertNull(keyOf(firing, "recovery"))
    }

    // ── Mood × volume link ─────────────────────────────────────────────────────

    @Test
    fun moodVolumeLink_reportsTheGap() {
        val sessions = (0 until 10).map { i ->
            val vol = if (i < 6) 2000.0 else 1000.0
            Session(i + 1L, "upper-a", (60 + i * 3) * day, (60 + i * 3) * day + hour, totalVolumeLb = vol)
        }
        val moods = (0 until 10).map { i ->
            MoodEntry(i + 1L, sessionId = i + 1L, dayKey = "upper-a",
                mood = if (i < 6) "good" else "drained", recordedAt = (60 + i * 3) * day)
        }
        val insight = keyOf(InsightEngine.evaluate(snapshot(sessions = sessions, moods = moods)), "moodvolume")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("100%"))
    }

    // ── Session-estimate calibration ───────────────────────────────────────────

    @Test
    fun estimateCalibration_flagsADayThatRunsLongerThanEstimated() {
        // Upper A's single 3-set slot estimates tiny; actual sessions run 90 minutes.
        val sessions = (0 until 5).map { i ->
            Session(i + 1L, "upper-a", (80 + i * 4) * day, (80 + i * 4) * day + 90 * 60_000, totalVolumeLb = 1000.0)
        }
        val insight = keyStartingWith(InsightEngine.evaluate(snapshot(sessions = sessions)), "estimate.")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("~90 min"))
    }

    // ── Sweet-spot rep range (A2) ──────────────────────────────────────────────

    private fun repSet(weight: Double, reps: Int, idx: Int) = LoggedSet(
        loggedExerciseId = 1, setIndex = idx, weightText = "$weight", weightLb = weight, reps = reps, completedAt = 0
    )

    private fun mixedBout(atDay: Int, sets: List<LoggedSet>) = ExerciseBout(
        sessionStartedAt = atDay * day, effort = null, hitFullTarget = false,
        skipped = false, swappedName = null, sets = sets
    )

    @Test
    fun sweetSpotRepRange_namesTheStrongestBucket() {
        // 6 sets at 100×8 (e1RM ~127) clearly beat 6 sets at 70×12 (e1RM ~98) → the 6-10 bucket wins.
        val sets = List(6) { repSet(100.0, 8, it) } + List(6) { repSet(70.0, 12, it) }
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to listOf(mixedBout(90, sets)))))
        val insight = keyStartingWith(fired, "sweetspot")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("DB Bench Press"))
        assertTrue(insight.body.contains("6-10"))
    }

    @Test
    fun sweetSpotRepRange_silentWithoutTwoFullBuckets() {
        // Only one bucket clears the per-bucket sample gate — nothing to compare.
        val sets = List(6) { repSet(100.0, 8, it) } + List(2) { repSet(70.0, 12, it) }
        assertNull(keyStartingWith(InsightEngine.evaluate(snapshot(history = mapOf("ua1" to listOf(mixedBout(90, sets))))), "sweetspot"))
    }

    // ── Lagging lift (A6) ───────────────────────────────────────────────────────

    @Test
    fun laggingLift_flagsTheStalledLiftWhenAnotherClimbs() {
        // Same muscle: Bench climbs (e1RM 53→73), Incline flat — Incline is the laggard.
        val slots = listOf(slot("ua1", "DB Bench Press", MuscleGroup.CHEST), slot("ua2", "Incline Press", MuscleGroup.CHEST))
        val history = mapOf(
            "ua1" to listOf(bout(80, 40.0), bout(85, 40.0), bout(90, 55.0), bout(95, 55.0)),
            "ua2" to listOf(bout(80, 40.0), bout(85, 40.0), bout(90, 40.0), bout(95, 40.0))
        )
        val insight = keyStartingWith(InsightEngine.evaluate(snapshot(slots = slots, history = history)), "lagging")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("Incline Press"))
        assertTrue(insight.body.contains("DB Bench Press"))
    }

    @Test
    fun laggingLift_silentWhenBothLiftsProgress() {
        val slots = listOf(slot("ua1", "DB Bench Press", MuscleGroup.CHEST), slot("ua2", "Incline Press", MuscleGroup.CHEST))
        val history = mapOf(
            "ua1" to listOf(bout(80, 40.0), bout(85, 40.0), bout(90, 55.0), bout(95, 55.0)),
            "ua2" to listOf(bout(80, 40.0), bout(85, 40.0), bout(90, 52.0), bout(95, 52.0))
        )
        assertNull(keyStartingWith(InsightEngine.evaluate(snapshot(slots = slots, history = history)), "lagging"))
    }

    // ── Tier 5 · session-start-time vs performance ─────────────────────────────

    @Test
    fun timeOfDayPerformance_flagsTheStrongerWindow() {
        // One lift trained both morning (8:00, 60 lb) and evening (18:00, 50 lb), 12 bouts each.
        val am = (0 until 12).map { bout(60 + it, weight = 60.0).copy(sessionStartedAt = (60 + it) * day + 8 * hour) }
        val pm = (0 until 12).map { bout(60 + it, weight = 50.0).copy(sessionStartedAt = (60 + it) * day + 18 * hour) }
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to (am + pm))))
        val insight = keyOf(fired, "timeofdayperf")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("earlier"))
    }

    @Test
    fun timeOfDayPerformance_silentWithoutBothWindows() {
        // Only ever trained in the morning — nothing to compare, so it stays silent.
        val amOnly = (0 until 14).map { bout(60 + it, weight = 60.0).copy(sessionStartedAt = (60 + it) * day + 8 * hour) }
        assertNull(keyOf(InsightEngine.evaluate(snapshot(history = mapOf("ua1" to amOnly))), "timeofdayperf"))
    }

    // ── Tier 5 · volume tolerance (A1) ─────────────────────────────────────────

    /** 9 distinct ISO weeks (7 days apart), each one bout: (setCount, weight). */
    private fun weeklyHistory(plan: List<Pair<Int, Double>>) =
        plan.mapIndexed { i, p -> bout(atDay = 30 + i * 7, weight = p.second, setCount = p.first) }

    @Test
    fun volumeResponse_flagsAMuscleThatRespondsToVolume() {
        // The gain follows the high-volume (8-set) weeks: each 8-set week is followed by a heavier
        // week, while the low-volume (4-set) weeks lead into a flat week — high volume → strength.
        val plan = listOf(8 to 50.0, 4 to 55.0, 8 to 55.0, 4 to 60.0, 8 to 60.0, 4 to 65.0, 8 to 65.0, 4 to 70.0, 8 to 70.0)
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to weeklyHistory(plan))))
        val insight = keyStartingWith(fired, "volumeresponse")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("Chest"))
        assertTrue(insight.body.contains("responding to more volume"))
    }

    @Test
    fun volumeResponse_flagsAVolumeCeiling() {
        // Inverted: the gain follows the LOW-volume (4-set) weeks; the extra sets in the 8-set weeks
        // lead into a flat week — past the split, more volume isn't paying off.
        val plan = listOf(4 to 50.0, 8 to 55.0, 4 to 55.0, 8 to 60.0, 4 to 60.0, 8 to 65.0, 4 to 65.0, 8 to 70.0, 4 to 70.0)
        val fired = InsightEngine.evaluate(snapshot(history = mapOf("ua1" to weeklyHistory(plan))))
        val insight = keyStartingWith(fired, "volumeresponse")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("volume ceiling"))
    }

    @Test
    fun volumeResponse_silentBelowTheWeekGate() {
        val few = (0 until 4).map { bout(atDay = 30 + it * 7, weight = 50.0 + it * 5, setCount = 5) }
        assertNull(keyStartingWith(InsightEngine.evaluate(snapshot(history = mapOf("ua1" to few))), "volumeresponse"))
    }

    // ── Tier 5 · recovery curve (rest gap vs performance) ──────────────────────

    private fun restSessions(days: List<Int>, vols: List<Double>) =
        days.indices.map { Session(it + 1L, "upper-a", days[it] * day, days[it] * day + hour, totalVolumeLb = vols[it]) }

    @Test
    fun restResponse_flagsTheProductiveSpacing() {
        // Sessions after 3 rest days move 2000 lb; after 1 rest day, 1000 lb. Same workout (one dayKey),
        // so the day-type normalisation is neutral and the rest gap is the only thing varying.
        val days = listOf(20, 23, 24, 27, 28, 31, 32, 35, 36, 39, 40, 43, 44, 47)
        val vols = listOf(1500.0, 2000.0, 1000.0, 2000.0, 1000.0, 2000.0, 1000.0, 2000.0, 1000.0, 2000.0, 1000.0, 2000.0, 1000.0, 2000.0)
        val insight = keyOf(InsightEngine.evaluate(snapshot(sessions = restSessions(days, vols))), "restresponse")
        assertNotNull(insight)
        assertTrue(insight!!.body.contains("recovery"))
    }

    @Test
    fun restResponse_silentBelowTheSessionGate() {
        val days = listOf(20, 23, 24, 27, 28, 31, 32, 35)
        val vols = List(8) { if (it % 2 == 0) 2000.0 else 1000.0 }
        assertNull(keyOf(InsightEngine.evaluate(snapshot(sessions = restSessions(days, vols))), "restresponse"))
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameSnapshotProducesTheSameInsights() {
        val snap = snapshot(history = mapOf("ua1" to (0 until 10).map { bout(90 + it) }))
        assertEquals(InsightEngine.evaluate(snap), InsightEngine.evaluate(snap))
    }
}
