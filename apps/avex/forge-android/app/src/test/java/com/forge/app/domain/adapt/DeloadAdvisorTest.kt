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

    private fun lowMoods(): List<MoodEntry> = listOf(
        MoodEntry(1, null, "upper-a", "drained", (now - 1 * day)),
        MoodEntry(2, null, "upper-a", "off", (now - 3 * day)),
        MoodEntry(3, null, "upper-a", "drained", (now - 5 * day)),
        MoodEntry(4, null, "upper-a", "good", (now - 7 * day)),
        MoodEntry(5, null, "upper-a", "fine", (now - 9 * day))
    )

    private fun soreCardio() = listOf(
        CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore")
    )

    private fun snapshot(
        sessions: List<Session> = baseSessions(),
        history: Map<String, List<ExerciseBout>> = mapOf("ua1" to brutalWindowBouts()),
        moods: List<MoodEntry> = emptyList(),
        cardio: List<CardioEntry> = emptyList()
    ) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = sessions,
        exerciseHistory = history, moods = moods, cardio = cardio, prefs = PrefsSnap()
    )

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun coldStart_tooFewSessions_staysSilent() {
        val few = baseSessions().take(7)
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = few, moods = lowMoods(), cardio = soreCardio())))
    }

    @Test
    fun coldStart_historyShorterThanOneWindow_staysSilent() {
        // 8 sessions but all inside the last 14 days — no prior window to trend against.
        val recentOnly = (0 until 8).map { session(it + 1L, startDay = 47 + it, volume = 1000.0) }
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = recentOnly, moods = lowMoods(), cardio = soreCardio())))
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun effortInflationPlusLowMoodPlusSoreness_firesAtMediumWithNamedDrivers() {
        // +2 effort inflation, +2 low mood, +1 sore = 5 — exactly the threshold.
        val rec = DeloadAdvisor.evaluate(snapshot(moods = lowMoods(), cardio = soreCardio()))
        assertNotNull(rec)
        assertEquals(5, rec!!.score)
        assertEquals(Confidence.MEDIUM, rec.confidence)
        assertEquals(3, rec.drivers.size)
        assertTrue(rec.reason.contains("hard/brutal at flat volume"))
        assertTrue(rec.reason.contains("mood low in 3 of the last 5"))
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
                moods = lowMoods(),
                cardio = soreCardio()
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
        assertNull(DeloadAdvisor.evaluate(snapshot(sessions = withDeload, moods = lowMoods(), cardio = soreCardio())))
    }

    @Test
    fun persistedDeloadMarker_mutesTheAdvisor_beforeAnyDeloadSessionExists() {
        // A deload was just applied (the pref marker is set) but no deload-tagged session has been
        // logged yet. The marker alone must suppress, or the very next pass re-proposes the deload
        // it just ran — the recurring-deload churn (seam fix #18).
        val snap = AdaptationSnapshot(
            nowMs = now, program = emptyList(), sessions = baseSessions(),
            exerciseHistory = mapOf("ua1" to brutalWindowBouts()),
            moods = lowMoods(), cardio = soreCardio(),
            prefs = PrefsSnap(lastDeloadAppliedMs = now - 2 * day)
        )
        assertNull(DeloadAdvisor.evaluate(snap))
    }

    @Test
    fun conflictingSignals_trainingGoingWell_calendarAloneNeverFires() {
        // Effort fine, mood good, no soreness — only time has passed. Score stays below
        // threshold: a date is not fatigue.
        val happyBouts = (0 until 6).map { bout(startDay = 48 + it * 2, effort = EffortRating.JUST_RIGHT) }
        val goodMoods = (1..5).map { MoodEntry(it.toLong(), null, "upper-a", "good", now - it * day) }
        // Spread history beyond 8 weeks so the "overdue" driver alone is in play.
        val longHistory = (0 until 8).map { session(it + 1L, startDay = 1 + it * 7, volume = 1000.0) }
        val rec = DeloadAdvisor.evaluate(snapshot(sessions = longHistory, history = mapOf("ua1" to happyBouts), moods = goodMoods))
        assertNull(rec)
    }

    @Test
    fun sickOutweighsSore() {
        val sickCardio = listOf(
            CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"),
            CardioEntry(2, date = now - 3 * day, type = "rest", durationMin = 0, restReason = "sick")
        )
        val rec = DeloadAdvisor.evaluate(snapshot(moods = lowMoods(), cardio = sickCardio))
        assertNotNull(rec)
        assertEquals(6, rec!!.score) // 2 + 2 + 2 (sick), the sore +1 must not stack
        assertTrue(rec.reason.contains("sick"))
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameSnapshotProducesTheSameSuggestion() {
        val snap = snapshot(moods = lowMoods(), cardio = soreCardio())
        assertEquals(DeloadAdvisor.evaluate(snap), DeloadAdvisor.evaluate(snap))
    }
}
