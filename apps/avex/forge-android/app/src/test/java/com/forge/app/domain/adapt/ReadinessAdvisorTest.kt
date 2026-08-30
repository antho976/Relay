package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.VacationPeriod
import com.forge.app.domain.coach.LifeEvents
import com.forge.app.domain.vacation.VacationCalendar
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/** System 6 of the adaptation engine. "Now" pinned to day 30. */
class ReadinessAdvisorTest {

    private val hour = 60L * 60 * 1000
    private val day = 24 * hour
    private val now = 30 * day

    private fun session(id: Long, startedAt: Long, volume: Double = 1000.0) = Session(
        id = id, dayKey = "upper-a", startedAt = startedAt,
        finishedAt = startedAt + hour, totalVolumeLb = volume
    )

    /** Three sessions, the most recent [lastSessionDaysAgo] days back. */
    private fun sessions(lastSessionDaysAgo: Int) = listOf(
        session(1, now - (lastSessionDaysAgo + 6) * day),
        session(2, now - (lastSessionDaysAgo + 3) * day),
        session(3, now - lastSessionDaysAgo * day)
    )

    // ── Cold start gates ───────────────────────────────────────────────────────

    @Test
    fun tooFewSessions_neutral() {
        assertNull(ReadinessAdvisor.evaluate(sessions(2).take(2), emptyList(), now))
    }

    @Test
    fun netZeroSignals_staySilentInsteadOfEmittingZero() {
        // Trained yesterday at normal volume, spacing neutral → no scale, no noise.
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now))
    }

    // ── Happy paths ────────────────────────────────────────────────────────────

    @Test
    fun freshSpacing_readsPositive() {
        val r = ReadinessAdvisor.evaluate(sessions(2), emptyList(), now)
        assertNotNull(r)
        assertEquals(1, r!!.percent) // +1 fresh after 2 rest days
        assertTrue(r.reason.contains("fresh after 2 rest days"))
    }

    @Test
    fun comebackAndIllness_clampsAtTheNegativeBound() {
        // B1 (plan M6): illness arrives through LifeEvents, not a second cardio-flag deduction —
        // one signal, one computation. The cardio row that used to carry it is still logged; it
        // simply no longer speaks twice.
        val cardio = listOf(CardioEntry(1, date = now - 12 * hour, type = "rest", durationMin = 0, restReason = "sick"))
        val life = LifeEvents.State.NONE.copy(sick = true)
        val r = ReadinessAdvisor.evaluate(sessions(6), cardio, now, lifeEvents = life)
        assertNotNull(r)
        assertEquals(-5, r!!.percent) // −5 sick, −3 comeback → clamped to −5
        assertTrue(r.reason.contains("unwell"))
    }

    @Test
    fun aSickCardioRowAloneNoLongerDeductsTwice() {
        // Without the life-events flag the advisor treats the row as what it is: a rest day.
        val cardio = listOf(CardioEntry(1, date = now - 12 * hour, type = "rest", durationMin = 0, restReason = "sick"))
        val r = ReadinessAdvisor.evaluate(sessions(1), cardio, now)
        assertNull("a rest row with no life flag is not a readiness deduction of its own", r)
    }

    @Test
    fun unusuallyHeavySessionYesterday_easesToday() {
        // Yesterday at 2000 lb vs a 1000 lb norm → −2; spacing neutral.
        val s = listOf(
            session(1, now - 9 * day), session(2, now - 6 * day),
            session(3, now - 3 * day), session(4, now - 24 * hour, volume = 2000.0)
        )
        val r = ReadinessAdvisor.evaluate(s, emptyList(), now)
        assertNotNull(r)
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("heavy session yesterday"))
    }

    // ── "Yesterday" is a calendar day, not a fixed span of hours ──────────────
    //
    // `now` is midnight UTC on day 30, so these pin an explicit hour of the day. Both cases were
    // decided by a fixed 12-36 h window, which lines up with "yesterday" from exactly one hour of
    // the morning and drifts from every other one.

    /** 20:00 today. Yesterday's 07:00 session is 37 h back — outside any 36 h window. */
    private val evening = now + 20 * hour

    @Test
    fun aHeavySessionYesterdayMorningIsStillSeenFromTheEvening() {
        val s = listOf(
            session(1, now - 9 * day), session(2, now - 6 * day), session(3, now - 3 * day),
            // Yesterday at 07:00 — 37 h before 20:00 today.
            session(4, now - 17 * hour, volume = 2000.0)
        )
        val r = ReadinessAdvisor.evaluate(s, emptyList(), evening)
        assertNotNull("a morning session yesterday is still yesterday at 20:00", r)
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("heavy session yesterday"))
    }

    @Test
    fun aHeavySessionEarlierTodayIsNotCalledYesterday() {
        val s = listOf(
            session(1, now - 9 * day), session(2, now - 6 * day), session(3, now - 3 * day),
            // 08:00 TODAY — 12 h before 20:00, which the old window read as yesterday.
            session(4, now + 8 * hour, volume = 2000.0)
        )
        // A check-in anchors the read so this asserts on the reason rather than on silence.
        val sleptBadly = com.forge.app.data.db.entities.CheckinEntry(
            dateKey = "today", sleepQuality = 1, soreness = null, stress = null,
            motivation = null, recordedAt = evening - 2 * hour
        )
        val r = ReadinessAdvisor.evaluate(s, emptyList(), evening, checkins = listOf(sleptBadly))
        assertNotNull(r)
        assertTrue("the check-in still speaks", r!!.reason.contains("slept badly"))
        assertTrue(
            "today's session must not be deducted for as yesterday's",
            !r.reason.contains("heavy session yesterday")
        )
        assertEquals("only the check-in's -2, not a second -2 for today", -2, r.percent)
    }

    // ── Off-gym movement reads YESTERDAY's steps, not the day in progress ─────

    private fun steps(dayStartMs: Long, count: Int) = HealthSnap(
        dailySteps = listOf(DailySteps(dayStartMs = dayStartMs, steps = count))
    )

    @Test
    fun aLongDayOnYourFeetYesterdayCountsAgainstToday() {
        val r = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now, health = steps(now - day, 20_000)
        )
        assertNotNull(r)
        assertEquals(-1, r!!.percent)
        assertTrue(r.reason.contains("long day on your feet"))
    }

    @Test
    fun todaysStepsSoFarAreNotYesterdaysTotal() {
        // The bucket for the day in progress. It was admitted by the window and then always won the
        // `maxByOrNull`, because today's bucket is by definition the latest one there is — so this
        // read the day the athlete is being advised about, using it as evidence about that same day.
        val r = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now + 20 * hour, health = steps(now, 20_000)
        )
        assertNull("today's step count is not yesterday's evidence", r)
    }

    // ── Vacation-aware spacing (#135) ──────────────────────────────────────────

    private fun date(ms: Long) = LocalDate.ofInstant(Instant.ofEpochMilli(ms), ZoneOffset.UTC)

    @Test
    fun comebackCautionFiresWithoutVacation() {
        val r = ReadinessAdvisor.evaluate(sessions(6), emptyList(), now, ZoneOffset.UTC)
        assertNotNull(r)
        assertEquals(-3, r!!.percent) // 6-day comeback (−3)
        assertTrue(r.reason.contains("ease in"))
    }

    @Test
    fun vacationGapSuppressesComebackCaution() {
        // Last session 6 days ago, but the whole gap is a logged holiday → no "ease in".
        val lastDate = date(now - 6 * day)
        val today = date(now)
        val onVac = VacationCalendar.onVacation(
            listOf(VacationPeriod(startDate = lastDate.plusDays(1).toString(), endDate = today.toString()))
        )
        val r = ReadinessAdvisor.evaluate(sessions(6), emptyList(), now, ZoneOffset.UTC, onVac)
        assertNull(r) // no comeback → net zero → silent
    }

    @Test
    fun onlyNonVacationDaysCountTowardTimeOff() {
        // 6-day gap, but 4 of the days were holiday → effective 2 days off → "fresh" bonus, not comeback.
        val onVac = VacationCalendar.onVacation(
            listOf(VacationPeriod(startDate = date(now - 5 * day).toString(), endDate = date(now - 2 * day).toString()))
        )
        val r = ReadinessAdvisor.evaluate(sessions(6), emptyList(), now, ZoneOffset.UTC, onVac)
        assertNotNull(r)
        assertEquals(1, r!!.percent) // fresh-after-2 (+1)
        assertTrue(r.reason.contains("fresh"))
    }

    // ── Post-session mood (A1) ─────────────────────────────────────────────────

    private fun mood(code: String, hoursAgo: Int) =
        MoodEntry(dayKey = "upper-a", mood = code, recordedAt = now - hoursAgo * hour)

    @Test
    fun drainedLastSession_easesToday() {
        // Trained yesterday (spacing neutral, normal volume) but rated it drained → −2.
        val r = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, moods = listOf(mood("drained", 20)))
        assertNotNull(r)
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("drained"))
    }

    @Test
    fun strongLastSession_addsAPoint() {
        val r = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, moods = listOf(mood("strong", 20)))
        assertNotNull(r)
        assertEquals(1, r!!.percent)
        assertTrue(r.reason.contains("strong"))
    }

    @Test
    fun onlyTheMostRecentMoodInsideTheWindowSpeaks() {
        // Rough Monday, strong Wednesday: the fresher rating wins outright, no averaging.
        val moods = listOf(mood("drained", 40), mood("strong", 10))
        val r = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, moods = moods)
        assertNotNull(r)
        assertEquals(1, r!!.percent)
    }

    @Test
    fun staleMoodsAreIgnored() {
        // Outside the 48h window → no contribution → back to net-zero silence.
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, moods = listOf(mood("drained", 72))))
    }

    @Test
    fun neutralMood_changesNothing() {
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, moods = listOf(mood("fine", 10))))
    }

    @Test
    fun moodStacksWithOtherSignals_withinTheBound() {
        // 6-day comeback (−3) + drained (−2) = −5, exactly the bound.
        val r = ReadinessAdvisor.evaluate(sessions(6), emptyList(), now, moods = listOf(mood("off", 10)))
        assertNotNull(r)
        assertEquals(-5, r!!.percent)
        assertTrue(r.reason.contains("ease in"))
        assertTrue(r.reason.contains("off"))
    }

    // ── Check-in + measured health (B1) ────────────────────────────────────────

    private fun checkin(
        hoursAgo: Int = 2,
        sleep: Int? = null,
        soreness: Int? = null,
        stress: Int? = null,
        motivation: Int? = null
    ) = com.forge.app.data.db.entities.CheckinEntry(
        dateKey = "today", sleepQuality = sleep, soreness = soreness, stress = stress,
        motivation = motivation, recordedAt = now - hoursAgo * hour
    )

    @Test
    fun badNightOnTheCheckin_easesToday() {
        val r = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, checkins = listOf(checkin(sleep = 1)))
        assertNotNull(r)
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("slept badly"))
    }

    @Test
    fun checkinAnswersStack_withinTheBound() {
        // Bad sleep (−2), sore (−2), stressed (−1), low drive (−1) = −6 → clamped to −5.
        val r = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now,
            checkins = listOf(checkin(sleep = 2, soreness = 5, stress = 5, motivation = 1))
        )
        assertEquals(-5, r!!.percent)
    }

    @Test
    fun aGoodMorningReadsPositive() {
        val r = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now,
            checkins = listOf(checkin(sleep = 5, motivation = 5))
        )
        assertEquals(2, r!!.percent)
        assertTrue(r.reason.contains("slept well"))
    }

    @Test
    fun yesterdaysCheckinIsIgnored() {
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, checkins = listOf(checkin(hoursAgo = 30, sleep = 1))))
    }

    @Test
    fun skippedCheckinsSayNothing() {
        val skipped = com.forge.app.data.db.entities.CheckinEntry(
            dateKey = "today", skipped = true, recordedAt = now - 2 * hour
        )
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, checkins = listOf(skipped)))
    }

    @Test
    fun measuredSleepSpeaksOnlyWhenTheCheckinDidNot() {
        val health = HealthSnap(sleepNights = listOf(SleepNight(endedAtMs = now - 3 * hour, durationMin = 300)))
        val measured = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, health = health)
        assertEquals(-2, measured!!.percent)
        assertTrue(measured.reason.contains("short night"))

        // The user answered for sleep, so the watch doesn't answer again (§4.3 one home).
        val both = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now, checkins = listOf(checkin(sleep = 5)), health = health
        )
        assertEquals(1, both!!.percent)
    }

    @Test
    fun elevatedRestingHrAgainstOwnBaseline_easesToday() {
        val recent = listOf(RestingHrSample(timeMs = now - hour, bpm = 63))
        val baseline = (3..14).map { RestingHrSample(timeMs = now - it * day, bpm = 55) }
        val r = ReadinessAdvisor.evaluate(
            sessions(1), emptyList(), now, health = HealthSnap(restingHr = recent + baseline)
        )
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("resting HR up"))
    }

    @Test
    fun restingHrStaysSilentBelowItsSampleGate() {
        val recent = listOf(RestingHrSample(timeMs = now - hour, bpm = 70))
        val baseline = (3..4).map { RestingHrSample(timeMs = now - it * day, bpm = 55) }
        assertNull(ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, health = HealthSnap(restingHr = recent + baseline)))
    }

    @Test
    fun aLongDayOnYourFeetCounts() {
        val health = HealthSnap(dailySteps = listOf(DailySteps(dayStartMs = now - day, steps = 22_000)))
        val r = ReadinessAdvisor.evaluate(sessions(1), emptyList(), now, health = health)
        assertEquals(-1, r!!.percent)
        assertTrue(r.reason.contains("on your feet"))
    }

    // ── Life events (B1) ───────────────────────────────────────────────────────

    @Test
    fun theReturnRampEasesAndSuppressesTheSpacingRule() {
        // One "you've been away" line, not two (§4.3): the layoff speaks, spacing stays quiet.
        val life = LifeEvents.State.NONE.copy(
            layoff = LifeEvents.Layoff(days = 21, away = false, returning = true, returnedAtMs = now - day, gapStartMs = now - 22 * day)
        )
        val r = ReadinessAdvisor.evaluate(sessions(6), emptyList(), now, lifeEvents = life)
        assertEquals(-3, r!!.percent)
        assertTrue(r.reason.contains("first week back"))
        assertTrue(!r.reason.contains("ease in"))
    }

    @Test
    fun assessCarriesSorenessGatesAndLifeState() {
        val life = LifeEvents.State.NONE.copy(soreMuscles = setOf(com.forge.app.program.MuscleGroup.QUADS))
        val read = ReadinessAdvisor.assess(sessions(1), emptyList(), now, lifeEvents = life)
        assertEquals(setOf(com.forge.app.program.MuscleGroup.QUADS), read.soreMuscles)
        assertEquals(life, read.lifeEvents)
    }

    @Test
    fun theScaleCarriesItsLesson() {
        val r = ReadinessAdvisor.evaluate(sessions(2), emptyList(), now)
        assertEquals(ReadinessAdvisor.LESSON_READINESS, r!!.lessonId)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSameScale() {
        assertEquals(
            ReadinessAdvisor.evaluate(sessions(2), emptyList(), now),
            ReadinessAdvisor.evaluate(sessions(2), emptyList(), now)
        )
    }
}
