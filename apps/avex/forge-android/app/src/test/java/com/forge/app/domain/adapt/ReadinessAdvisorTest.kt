package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.VacationPeriod
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
    fun comebackAndSick_clampsAtTheNegativeBound() {
        val cardio = listOf(CardioEntry(1, date = now - 12 * hour, type = "rest", durationMin = 0, restReason = "sick"))
        val r = ReadinessAdvisor.evaluate(sessions(6), cardio, now)
        assertNotNull(r)
        assertEquals(-5, r!!.percent) // −3 comeback, −4 sick → clamped to −5
        assertTrue(r.reason.contains("ease in"))
        assertTrue(r.reason.contains("sick"))
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

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSameScale() {
        assertEquals(
            ReadinessAdvisor.evaluate(sessions(2), emptyList(), now),
            ReadinessAdvisor.evaluate(sessions(2), emptyList(), now)
        )
    }
}
