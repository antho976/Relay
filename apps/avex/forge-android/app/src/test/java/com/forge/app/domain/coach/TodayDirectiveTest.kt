package com.forge.app.domain.coach

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.Confidence
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * B2's one answer. The bar this suite holds: the directive is NEVER blank, in any mode, at any
 * data level — that promise is the whole feature.
 */
class TodayDirectiveTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 100 * day

    private fun session(daysAgo: Int, id: Long = daysAgo.toLong(), dayKey: String = "push") = Session(
        id = id, dayKey = dayKey, startedAt = now - daysAgo * day,
        finishedAt = now - daysAgo * day + 3_600_000, totalVolumeLb = 1000.0
    )

    private fun slot(id: String, muscle: MuscleGroup = MuscleGroup.CHEST) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(
        sessions: List<Session> = listOf(session(2), session(4), session(6))
    ) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("push", "Push day", listOf(slot("bench")))),
        sessions = sessions,
        exerciseHistory = emptyMap(),
        prefs = PrefsSnap()
    )

    private fun readiness(percent: Int, reason: String = "slept badly") =
        Recommendation.ReadinessScale(percent, reason, Confidence.MEDIUM)

    private fun compute(
        s: AdaptationSnapshot = snapshot(),
        readiness: Recommendation.ReadinessScale? = null,
        life: LifeEvents.State = LifeEvents.State.NONE,
        nextUp: String? = "push",
        trainedToday: Boolean = false,
        weekdayMode: Boolean = true,
        sessionsThisWeek: Int = 1,
        weeklyTarget: Int? = null,
        freestyle: Boolean = false
    ) = TodayDirective.compute(
        s = s, readiness = readiness, life = life, nextUpDayKey = nextUp,
        dayName = { if (it == "push") "Push day" else it },
        trainedToday = trainedToday, weekdayMode = weekdayMode,
        sessionsThisWeek = sessionsThisWeek, weeklyTarget = weeklyTarget, freestyle = freestyle
    )

    // ── The promise: never blank ───────────────────────────────────────────────

    @Test
    fun everyModeProducesAnAnswerWithAReason() {
        val cases = listOf(
            compute(),
            compute(freestyle = true, nextUp = null),
            compute(s = snapshot(sessions = emptyList())),
            compute(trainedToday = true),
            compute(life = LifeEvents.State.NONE.copy(sick = true)),
            compute(nextUp = null),
            compute(readiness = readiness(-5))
        )
        cases.forEach { d ->
            assertTrue("headline must never be blank", d.headline.isNotBlank())
            assertTrue("reason must never be blank", d.reason.isNotBlank())
        }
    }

    // ── Life outranks the schedule ─────────────────────────────────────────────

    @Test
    fun illnessOverridesAScheduledDay() {
        val d = compute(life = LifeEvents.State.NONE.copy(sick = true))
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertNull(d.dayKey)
        assertTrue(d.reason.contains("unwell"))
    }

    @Test
    fun comingBackFromALayoff_trainsButSaysWhy() {
        val life = LifeEvents.State.NONE.copy(
            layoff = LifeEvents.Layoff(days = 21, away = true, returning = false, returnedAtMs = null, gapStartMs = now - 22 * day)
        )
        val d = compute(life = life)
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
        assertTrue(d.reason.contains("21 days"))
    }

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun aFreshInstallIsToldWhatToDo_notLeftSilent() {
        val d = compute(s = snapshot(sessions = emptyList()))
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
        assertEquals("Push day", d.headline)
        assertEquals(TodayDirective.LESSON_HOW_THE_COACH_WORKS, d.lessonId)
    }

    @Test
    fun aFreshInstallWithNoProgramLearnsInstead() {
        val d = compute(s = snapshot(sessions = emptyList()), nextUp = null)
        assertEquals(TodayDirective.Kind.LEARN, d.kind)
        assertTrue(d.reason.isNotBlank())
    }

    // ── Ordinary days ──────────────────────────────────────────────────────────

    @Test
    fun aScheduledDayTrains() {
        val d = compute()
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
        assertEquals("push", d.dayKey)
        assertTrue(d.reason.contains("schedule"))
    }

    @Test
    fun sequenceModeNeverClaimsAWeekday() {
        val d = compute(weekdayMode = false)
        assertTrue(d.reason.contains("rotation"))
        assertTrue(!d.reason.contains("schedule"))
    }

    @Test
    fun trainedTodayClosesTheDay() {
        val d = compute(trainedToday = true)
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertTrue(d.reason.contains("trained"))
    }

    @Test
    fun lowReadinessTheDayAfterTraining_suggestsMovingInstead() {
        val d = compute(s = snapshot(sessions = listOf(session(0), session(3), session(5))), readiness = readiness(-4))
        assertEquals(TodayDirective.Kind.CARDIO, d.kind)
        assertTrue(d.secondary!!.contains("walk"))
        assertEquals(TodayDirective.LESSON_READINESS, d.lessonId)
    }

    @Test
    fun hittingYourOwnWeeklyTargetEarnsARestDay() {
        val d = compute(
            s = snapshot(sessions = listOf(session(0), session(2), session(4))),
            sessionsThisWeek = 4, weeklyTarget = 4
        )
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertTrue(d.reason.contains("4 sessions"))
    }

    @Test
    fun soreMusclesOnDeckAreNamed() {
        val life = LifeEvents.State.NONE.copy(soreMuscles = setOf(MuscleGroup.CHEST))
        val d = compute(life = life)
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
        assertTrue(d.reason.contains("chest"))
        assertEquals(setOf(MuscleGroup.CHEST), d.soreMuscles)
    }

    @Test
    fun sorenessElsewhereIsNotMentioned() {
        val life = LifeEvents.State.NONE.copy(soreMuscles = setOf(MuscleGroup.CALVES))
        assertTrue(!compute(life = life).reason.contains("calves"))
    }

    // ── Freestyle: the cohort with no program still gets an answer ─────────────

    @Test
    fun freestyleStillAnswers() {
        val d = compute(freestyle = true, nextUp = null)
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
        assertTrue(d.freestyle)
        assertTrue(d.reason.contains("No fixed program"))
        assertNull("nothing to prep without a program", d.dayKey)
    }

    @Test
    fun freestyleRestsWhenTheBodySaysSo() {
        val d = compute(freestyle = true, nextUp = null, life = LifeEvents.State.NONE.copy(sick = true))
        assertEquals(TodayDirective.Kind.REST, d.kind)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        assertEquals(compute(), compute())
    }
}
