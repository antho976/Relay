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

    /**
     * `trainedToday` is DERIVED from the snapshot, never passed in.
     *
     * It used to be a parameter defaulting to `false`, which let a case hand `compute` a session
     * started today while also swearing nothing had been trained today. `DirectiveRepository` reads
     * both from the same finished, tracked sessions, so that pairing cannot occur in production —
     * and two rules stayed green for months while being unreachable there. A test that can only
     * pass on an input the app cannot produce is not covering the branch; it is hiding that the
     * branch is dead. Want a session logged today? Add `session(0)`.
     */
    private fun compute(
        s: AdaptationSnapshot = snapshot(),
        readiness: Recommendation.ReadinessScale? = null,
        life: LifeEvents.State = LifeEvents.State.NONE,
        nextUp: String? = "push",
        weekdayMode: Boolean = true,
        sessionsThisWeek: Int = 1,
        weeklyTarget: Int? = null,
        freestyle: Boolean = false,
        upcomingDayKey: String? = null,
        upcomingInDays: Int = 0
    ) = TodayDirective.compute(
        s = s, readiness = readiness, life = life, nextUpDayKey = nextUp,
        dayName = { if (it == "push") "Push day" else it },
        trainedToday = TodayDirective.trainedToday(s), weekdayMode = weekdayMode,
        sessionsThisWeek = sessionsThisWeek, weeklyTarget = weeklyTarget, freestyle = freestyle,
        upcomingDayKey = upcomingDayKey, upcomingInDays = upcomingInDays
    )

    /** A snapshot whose most recent session was logged today. */
    private fun trainedTodaySnapshot() = snapshot(sessions = listOf(session(0), session(3), session(5)))

    /** A snapshot whose most recent session was logged yesterday — the recency rules' actual case. */
    private fun trainedYesterdaySnapshot() = snapshot(sessions = listOf(session(1), session(3), session(5)))

    // ── The promise: never blank ───────────────────────────────────────────────

    @Test
    fun everyModeProducesAnAnswerWithAReason() {
        val cases = listOf(
            compute(),
            compute(freestyle = true, nextUp = null),
            compute(s = snapshot(sessions = emptyList())),
            compute(s = trainedTodaySnapshot()),
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
        val d = compute(s = trainedTodaySnapshot())
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertTrue(d.reason.contains("trained"))
    }

    /**
     * YESTERDAY, not today. Today is already answered by "Done for today" two rules earlier, so a
     * case built on `session(0)` proves nothing about this branch — which is how it went unnoticed
     * that the branch read `< 1` and could therefore never run.
     */
    @Test
    fun lowReadinessTheDayAfterTraining_suggestsMovingInstead() {
        val d = compute(s = trainedYesterdaySnapshot(), readiness = readiness(-4))
        assertEquals(TodayDirective.Kind.CARDIO, d.kind)
        assertTrue(d.secondary!!.contains("walk"))
        assertEquals(TodayDirective.LESSON_READINESS, d.lessonId)
    }

    /** Low readiness two days out is spacing the athlete has already taken; it trains. */
    @Test
    fun lowReadinessAfterTwoDaysOffStillTrains() {
        val d = compute(s = snapshot(sessions = listOf(session(2), session(4), session(6))), readiness = readiness(-4))
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
    }

    @Test
    fun hittingYourOwnWeeklyTargetEarnsARestDay() {
        val d = compute(
            s = snapshot(sessions = listOf(session(1), session(2), session(4))),
            sessionsThisWeek = 4, weeklyTarget = 4
        )
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertTrue(d.reason.contains("4 sessions"))
    }

    /** The budget rule is about not training on back-to-back days, not a hard weekly stop. */
    @Test
    fun hittingTheWeeklyTargetDoesNotBlockTrainingAfterARestDay() {
        val d = compute(
            s = snapshot(sessions = listOf(session(2), session(3), session(5))),
            sessionsThisWeek = 4, weeklyTarget = 4
        )
        assertEquals(TodayDirective.Kind.TRAIN, d.kind)
    }

    /**
     * The guard against the class of bug finding 15 was: a rule that can never fire.
     *
     * `trainedToday` and `daysSinceLast` are read from the same sessions, so `daysSinceLast == 0`
     * implies `trainedToday`, and any rule gated on `daysSinceLast < 1` below that early return is
     * unreachable. Assert the equivalence rather than the rules, so it holds for rules added later.
     */
    @Test
    fun trainedTodayAndZeroDaysSinceLastAreTheSameState() {
        listOf(0, 1, 2, 5).forEach { daysAgo ->
            val s = snapshot(sessions = listOf(session(daysAgo), session(daysAgo + 2), session(daysAgo + 4)))
            assertEquals(
                "a session $daysAgo day(s) ago: trainedToday must agree with daysSinceLast == 0",
                daysAgo == 0,
                TodayDirective.trainedToday(s)
            )
        }
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

    // ── A scheduled rest day is a rest day, even when a workout is next up ────

    @Test
    fun weekdayRestSlot_restsAndNamesTomorrowsSession() {
        val d = compute(nextUp = null, upcomingDayKey = "push", upcomingInDays = 1)
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertEquals("Rest today", d.headline)
        assertEquals("Today is a rest day in your schedule.", d.reason)
        assertEquals("Push day is next, tomorrow.", d.secondary)
        assertNull(d.dayKey)
    }

    @Test
    fun weekdayRestSlot_namesTheWeekdayWhenFurtherOut() {
        val d = compute(nextUp = null, upcomingDayKey = "push", upcomingInDays = 2)
        assertEquals(TodayDirective.Kind.REST, d.kind)
        val expected = java.time.Instant.ofEpochMilli(now).atZone(java.time.ZoneOffset.UTC).toLocalDate()
            .plusDays(2).dayOfWeek
            .getDisplayName(java.time.format.TextStyle.FULL, java.util.Locale.getDefault())
        assertEquals("Push day is next, on $expected.", d.secondary)
    }

    @Test
    fun nothingScheduledAnywhere_keepsThePlainRest() {
        val d = compute(nextUp = null)
        assertEquals(TodayDirective.Kind.REST, d.kind)
        assertEquals("Nothing scheduled today.", d.reason)
        assertNull(d.secondary)
    }

    @Test
    fun anUpcomingDayNeverBecomesTodaysSession() {
        // The directive only ever opens `nextUpDayKey`; an upcoming key is copy, not a target.
        val d = compute(nextUp = null, upcomingDayKey = "push", upcomingInDays = 1)
        assertNull(d.dayKey)
        assertTrue(d.kind != TodayDirective.Kind.TRAIN)
    }
}
