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

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameSnapshotProducesTheSameInsights() {
        val snap = snapshot(history = mapOf("ua1" to (0 until 10).map { bout(90 + it) }))
        assertEquals(InsightEngine.evaluate(snap), InsightEngine.evaluate(snap))
    }
}
