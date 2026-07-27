package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * System 5 of the adaptation engine. Fixtures pin "now" to day 60 and build the fatigue
 * window (days 46–60) and the prior window (days 32–46) explicitly.
 */
class DeloadAdvisorTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 60 * day

    private fun session(
        id: Long,
        startDay: Int,
        volume: Double = 1000.0,
        sessionType: String = "normal"
    ) = Session(
        id = id, dayKey = "upper-a", startedAt = startDay * day,
        finishedAt = startDay * day + 3_600_000, totalVolumeLb = volume,
        sessionType = sessionType
    )

    /** 4 sessions in the fatigue window at flat volume + 4 in the prior window — 8 total. */
    private fun baseSessions(): List<Session> =
        (0 until 4).map { session(it + 1L, startDay = 34 + it * 3, volume = 1100.0) } +
            (0 until 4).map { session(it + 5L, startDay = 48 + it * 3, volume = 1000.0) }

    private fun set(weight: Double, reps: Int) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = reps, completedAt = 0
    )

    private fun bout(startDay: Int, effort: EffortRating?, sets: List<LoggedSet> = listOf(set(45.0, 8))) =
        ExerciseBout(
            sessionStartedAt = startDay * day, effort = effort, hitFullTarget = false,
            skipped = false, swappedName = null, sets = sets
        )

    /** 6 brutal-rated bouts inside the window — the effort-inflation driver's fuel (+2). */
    private fun brutalWindowBouts(): List<ExerciseBout> =
        (0 until 6).map { bout(startDay = 48 + it * 2, effort = EffortRating.BRUTAL) }

    private fun soreCardio() = listOf(
        CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore")
    )

    private fun snapshot(
        sessions: List<Session> = baseSessions(),
        history: Map<String, List<ExerciseBout>> = mapOf("ua1" to brutalWindowBouts()),
        cardio: List<CardioEntry> = emptyList(),
        health: HealthSnap = HealthSnap()
    ) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = sessions,
        exerciseHistory = history, cardio = cardio, prefs = PrefsSnap(), health = health
    )

    /** Calm, single-set bouts that fire NO driver on their own — a clean stage for one recovery signal. */
    private fun calmBouts(): List<ExerciseBout> =
        (0 until 6).map { bout(startDay = 48 + it * 2, effort = EffortRating.JUST_RIGHT) }

    private fun nights(count: Int, durationMin: Int): List<SleepNight> =
        (0 until count).map { SleepNight(endedAtMs = now - (1 + it) * day, durationMin = durationMin) }

    /** [windowBpm] readings inside the fatigue window, [priorBpm] in the prior month. */
    private fun hr(windowBpm: Int, priorBpm: Int, count: Int = 5): List<RestingHrSample> =
        (0 until count).map { RestingHrSample(timeMs = now - (1 + it) * day, bpm = windowBpm) } +
            (0 until count).map { RestingHrSample(timeMs = (20 + it) * day, bpm = priorBpm) }

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun coldStart_tooFewSessions_staysSilent() {
        val few = baseSessions().take(7)
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = few, cardio = soreCardio())))
    }

    @Test
    fun coldStart_historyShorterThanOneWindow_staysSilent() {
        // 8 sessions but all inside the last 14 days — no prior window to trend against.
        val recentOnly = (0 until 8).map { session(it + 1L, startDay = 47 + it, volume = 1000.0) }
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = recentOnly, cardio = soreCardio())))
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun effortInflationPlusSleepDebtPlusSoreness_firesAtMediumWithNamedDrivers() {
        // +2 effort inflation, +2 sleep debt (HC), +1 sore = 5 — exactly the threshold.
        val rec = DeloadAdvisor.evaluate(
            snapshot(cardio = soreCardio(), health = HealthSnap(sleepNights = nights(6, 360)))
        )
        assertNotNull(rec)
        assertEquals(5, rec!!.score)
        assertEquals(Confidence.MEDIUM, rec.confidence)
        assertEquals(3, rec.drivers.size)
        assertTrue(rec.reason.contains("hard/brutal at flat volume"))
        assertTrue(rec.reason.contains("sleep"))
        assertTrue(rec.reason.contains("soreness"))
    }

    @Test
    fun addingE1rmRegression_raisesToHighConfidence() {
        // Two lifts whose window-best e1RM sits below 95% of the prior month's best: +2 → 7.
        fun regressing(): List<ExerciseBout> = listOf(
            bout(startDay = 36, effort = null, sets = listOf(set(50.0, 8))),  // prior window best
            bout(startDay = 50, effort = null, sets = listOf(set(42.5, 8)))   // window best — regressed
        )
        val rec = DeloadAdvisor.evaluate(
            snapshot(
                history = mapOf("ua1" to brutalWindowBouts(), "ua2" to regressing(), "ua3" to regressing()),
                cardio = soreCardio(),
                health = HealthSnap(sleepNights = nights(6, 360))
            )
        )
        assertNotNull(rec)
        assertEquals(7, rec!!.score)
        assertEquals(Confidence.HIGH, rec.confidence)
        assertTrue(rec.reason.contains("2 lifts below last month's strength"))
    }

    // ── Conflicting / suppressing signals ──────────────────────────────────────

    @Test
    fun recentDeload_mutesTheAdvisorEvenWithFatigueSignals() {
        val withDeload = baseSessions() + session(9, startDay = 56, sessionType = "deload")
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = withDeload, cardio = soreCardio())))
    }

    @Test
    fun persistedDeloadMarker_mutesTheAdvisor_beforeAnyDeloadSessionExists() {
        // A deload was just applied (the pref marker is set) but no deload-tagged session has been
        // logged yet. The marker alone must suppress, or the very next pass re-proposes the deload
        // it just ran — the recurring-deload churn (seam fix #18).
        val snap = AdaptationSnapshot(
            nowMs = now, program = emptyList(), sessions = baseSessions(),
            exerciseHistory = mapOf("ua1" to brutalWindowBouts()),
            cardio = soreCardio(),
            prefs = PrefsSnap(lastDeloadAppliedMs = now - 2 * day)
        )
        assertNull(DeloadAdvisor.evaluate(snap))
    }

    @Test
    fun conflictingSignals_trainingGoingWell_calendarAloneNeverFires() {
        // Effort fine, no soreness — only time has passed. Score stays below threshold: a date
        // is not fatigue.
        val happyBouts = (0 until 6).map { bout(startDay = 48 + it * 2, effort = EffortRating.JUST_RIGHT) }
        // Spread history beyond 8 weeks so the "overdue" driver alone is in play.
        val longHistory = (0 until 8).map { session(it + 1L, startDay = 1 + it * 7, volume = 1000.0) }
        val rec = DeloadAdvisor.evaluate(snapshot(sessions = longHistory, history = mapOf("ua1" to happyBouts)))
        assertNull(rec)
    }

    @Test
    fun sickOutweighsSore() {
        val sickCardio = listOf(
            CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"),
            CardioEntry(2, date = now - 3 * day, type = "rest", durationMin = 0, restReason = "sick")
        )
        val rec = DeloadAdvisor.evaluate(
            snapshot(cardio = sickCardio, health = HealthSnap(sleepNights = nights(6, 360)))
        )
        assertNotNull(rec)
        assertEquals(6, rec!!.score) // 2 effort + 2 sleep + 2 sick, the sore +1 must not stack
        assertTrue(rec.reason.contains("sick"))
    }

    // ── Health Connect recovery drivers (additive, gated) ───────────────────────

    @Test
    fun noHealthData_addsNoRecoveryDrivers() {
        // The clean stage with empty HealthSnap must score 0 — HC is purely additive.
        val f = DeloadAdvisor.fatigue(snapshot(history = mapOf("ua1" to calmBouts())))
        assertNotNull(f)
        assertEquals(0, f!!.score)
        assertTrue(f.drivers.isEmpty())
    }

    @Test
    fun sleepDebt_firesWhenAveragingBelowTarget() {
        // 6 nights at 6h (360 min ≤ 390 ceiling) → +2, on an otherwise-silent snapshot.
        val f = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(sleepNights = nights(6, 360)))
        )
        assertNotNull(f)
        assertEquals(2, f!!.score)
        assertTrue(f.drivers.any { it.contains("6.0h sleep over 6 nights") })
    }

    @Test
    fun sleepDebt_silentWhenWellRested_orTooFewNights() {
        // Enough nights but a healthy average — no driver.
        val rested = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(sleepNights = nights(6, 450)))
        )
        assertEquals(0, rested!!.score)
        // Short nights but below the count gate (4 < 5) — too sparse to judge, stays silent.
        val sparse = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(sleepNights = nights(4, 330)))
        )
        assertEquals(0, sparse!!.score)
    }

    @Test
    fun restingHr_firesWhenElevatedAboveOwnBaseline() {
        // Window 62 bpm vs prior 55 bpm = +7 ≥ 5 threshold → +2.
        val f = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(restingHr = hr(windowBpm = 62, priorBpm = 55)))
        )
        assertNotNull(f)
        assertEquals(2, f!!.score)
        assertTrue(f.drivers.any { it.contains("resting HR up 7 bpm vs your baseline") })
    }

    @Test
    fun restingHr_silentWhenWithinNormalDrift() {
        // Only +3 bpm over baseline — under the 5 bpm threshold, no driver.
        val f = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(restingHr = hr(windowBpm = 58, priorBpm = 55)))
        )
        assertEquals(0, f!!.score)
    }

    @Test
    fun recoveryDriversStackWithTrainingSignalsToCrossThreshold() {
        // sleep debt (+2) + elevated resting HR (+2) + soreness (+1) = 5 → fires at MEDIUM.
        val rec = DeloadAdvisor.evaluate(
            snapshot(
                history = mapOf("ua1" to calmBouts()),
                cardio = soreCardio(),
                health = HealthSnap(sleepNights = nights(6, 360), restingHr = hr(windowBpm = 62, priorBpm = 55))
            )
        )
        assertNotNull(rec)
        assertEquals(5, rec!!.score)
        assertEquals(Confidence.MEDIUM, rec.confidence)
        assertTrue(rec.reason.contains("sleep"))
        assertTrue(rec.reason.contains("resting HR"))
    }

    // ── A1 drivers: mood, HRV, daily steps (additive, gated) ───────────────────

    private fun moods(low: Int, good: Int): List<MoodEntry> =
        (0 until low).map { MoodEntry(dayKey = "upper-a", mood = "drained", recordedAt = now - (1 + it) * day) } +
            (0 until good).map { MoodEntry(dayKey = "upper-a", mood = "good", recordedAt = now - (8 + it) * day) }

    private fun hrv(windowMs: Double, priorMs: Double, count: Int = 6): List<HrvSample> =
        (0 until count).map { HrvSample(timeMs = now - (1 + it) * day, rmssdMs = windowMs) } +
            (0 until count).map { HrvSample(timeMs = (20 + it) * day, rmssdMs = priorMs) }

    private fun steps(perDay: Int, days: Int = 8): List<DailySteps> =
        (0 until days).map { DailySteps(dayStartMs = now - (1 + it) * day, steps = perDay) }

    private fun moodSnapshot(moods: List<MoodEntry>) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = baseSessions(),
        exerciseHistory = mapOf("ua1" to calmBouts()), moods = moods, prefs = PrefsSnap()
    )

    @Test
    fun mood_firesWhenMostRecentSessionsFeltRough() {
        // 4 of 6 window ratings drained (67% ≥ 50%) → +1 on an otherwise-silent snapshot.
        val f = DeloadAdvisor.fatigue(moodSnapshot(moods(low = 4, good = 2)))
        assertNotNull(f)
        assertEquals(1, f!!.score)
        assertTrue(f.drivers.any { it.contains("felt drained or off") })
    }

    @Test
    fun mood_silentWhenSessionsFeelFine_orTooFewRatings() {
        assertEquals(0, DeloadAdvisor.fatigue(moodSnapshot(moods(low = 1, good = 5)))!!.score)
        // Below the 4-rating gate: sparse data must not speak.
        assertEquals(0, DeloadAdvisor.fatigue(moodSnapshot(moods(low = 3, good = 0)))!!.score)
    }

    @Test
    fun hrv_firesWhenSuppressedBelowOwnBaseline() {
        // Window 44 ms vs prior 55 ms = −20% ≥ 12% threshold → +2.
        val f = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(hrv = hrv(44.0, 55.0)))
        )
        assertNotNull(f)
        assertEquals(2, f!!.score)
        assertTrue(f.drivers.any { it.contains("HRV down 20%") })
    }

    @Test
    fun hrv_silentWithinNormalNoise_orTooFewReadings() {
        // −5% is night-to-night noise, not fatigue.
        assertEquals(0, DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(hrv = hrv(52.0, 55.0)))
        )!!.score)
        // Below the 5-sample gate on each side.
        assertEquals(0, DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(hrv = hrv(40.0, 55.0, count = 3)))
        )!!.score)
    }

    @Test
    fun dailySteps_fireOnlyOnSustainedHighMovement() {
        val busy = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(dailySteps = steps(16_000)))
        )
        assertEquals(1, busy!!.score)
        assertTrue(busy.drivers.any { it.contains("steps a day") })
        // An ordinary week of walking is not fatigue.
        assertEquals(0, DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(dailySteps = steps(7_000)))
        )!!.score)
        // Too few days to judge.
        assertEquals(0, DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts()), health = HealthSnap(dailySteps = steps(16_000, days = 4)))
        )!!.score)
    }

    @Test
    fun a1Drivers_areAbsentWithoutTheirData() {
        // The pre-A1 clean stage must still score 0 — every new driver is additive.
        val f = DeloadAdvisor.fatigue(snapshot(history = mapOf("ua1" to calmBouts())))
        assertEquals(0, f!!.score)
        assertTrue(f.checks.any { it.name == "Session mood" && it.reading == "no data" })
        assertTrue(f.checks.any { it.name == "Heart-rate variability" && it.reading == "no data" })
        assertTrue(f.checks.any { it.name == "Daily movement" && it.reading == "no data" })
    }

    // ── Session types that aren't ordinary training (A1) ───────────────────────

    @Test
    fun techniqueAndTestBouts_doNotFeedFatigueDrivers() {
        // Same brutal bouts that normally fire effort inflation (+2), but logged on technique days.
        val techniqueBouts = brutalWindowBouts().map { it.copy(sessionType = "technique") }
        val f = DeloadAdvisor.fatigue(snapshot(history = mapOf("ua1" to techniqueBouts)))
        assertNotNull(f)
        assertEquals(0, f!!.score)
    }

    @Test
    fun testDayPr_doesNotMaskARealRegression() {
        // Prior month best 50 lb; the window's only *training* bout regressed to 42.5, but a test
        // day hit 55. Counting the test day would hide the regression on both lifts.
        fun lift() = listOf(
            bout(startDay = 36, effort = null, sets = listOf(set(50.0, 8))),
            bout(startDay = 50, effort = null, sets = listOf(set(42.5, 8))),
            bout(startDay = 52, effort = null, sets = listOf(set(55.0, 1))).copy(sessionType = "test")
        )
        val f = DeloadAdvisor.fatigue(
            snapshot(history = mapOf("ua1" to calmBouts(), "ua2" to lift(), "ua3" to lift()))
        )
        assertNotNull(f)
        assertEquals(2, f!!.score)
        assertTrue(f.drivers.any { it.contains("2 lifts below last month's strength") })
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameSnapshotProducesTheSameSuggestion() {
        val snap = snapshot(cardio = soreCardio(), health = HealthSnap(sleepNights = nights(6, 360)))
        assertEquals(DeloadAdvisor.evaluate(snap), DeloadAdvisor.evaluate(snap))
    }
}
