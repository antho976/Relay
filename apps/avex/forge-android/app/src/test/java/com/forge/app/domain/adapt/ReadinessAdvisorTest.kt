package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.MoodEntry
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

    private fun mood(code: String, recordedAt: Long, id: Long = recordedAt) =
        MoodEntry(id = id, sessionId = null, dayKey = "upper-a", mood = code, recordedAt = recordedAt)

    private fun threeFineMoods() = listOf(
        mood("fine", now - 3 * day), mood("fine", now - 5 * day), mood("fine", now - 7 * day)
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
        assertNull(ReadinessAdvisor.evaluate(sessions(2).take(2), threeFineMoods(), emptyList(), now))
    }

    @Test
    fun tooFewRecentMoods_neutral() {
        val staleMoods = listOf(mood("good", now - 20 * day), mood("good", now - 25 * day), mood("good", now - 30 * day))
        assertNull(ReadinessAdvisor.evaluate(sessions(2), staleMoods, emptyList(), now))
    }

    @Test
    fun netZeroSignals_staySilentInsteadOfEmittingZero() {
        // Fine mood (0), trained yesterday at normal volume (0) → no scale, no noise.
        val s = sessions(1)
        assertNull(ReadinessAdvisor.evaluate(s, threeFineMoods(), emptyList(), now))
    }

    // ── Happy paths ────────────────────────────────────────────────────────────

    @Test
    fun strongMoodAndFreshSpacing_readsPositive() {
        val moods = listOf(mood("strong", now - 6 * hour)) + threeFineMoods()
        val r = ReadinessAdvisor.evaluate(sessions(2), moods, emptyList(), now)
        assertNotNull(r)
        assertEquals(3, r!!.percent) // +2 strong, +1 fresh after 2 rest days
        assertTrue(r.reason.contains("felt strong"))
        assertTrue(r.reason.contains("fresh after 2 rest days"))
    }

    @Test
    fun drainedComebackAndSick_clampsAtTheNegativeBound() {
        val moods = listOf(mood("drained", now - 6 * hour)) + threeFineMoods()
        val cardio = listOf(CardioEntry(1, date = now - 12 * hour, type = "rest", durationMin = 0, restReason = "sick"))
        val r = ReadinessAdvisor.evaluate(sessions(6), moods, cardio, now)
        assertNotNull(r)
        assertEquals(-5, r!!.percent) // −3 drained, −3 comeback, −4 sick → clamped to −5
        assertTrue(r.reason.contains("ease in"))
        assertTrue(r.reason.contains("sick"))
    }

    @Test
    fun unusuallyHeavySessionYesterday_easesToday() {
        // Yesterday at 2000 lb vs a 1000 lb norm → −2; mood fine, spacing neutral.
        val s = listOf(
            session(1, now - 9 * day), session(2, now - 6 * day),
            session(3, now - 3 * day), session(4, now - 24 * hour, volume = 2000.0)
        )
        val r = ReadinessAdvisor.evaluate(s, threeFineMoods(), emptyList(), now)
        assertNotNull(r)
        assertEquals(-2, r!!.percent)
        assertTrue(r.reason.contains("heavy session yesterday"))
    }

    @Test
    fun staleMoodOlderThan48h_doesNotDescribeToday() {
        // STRONG five days ago must not inflate today; only the fresh-spacing +1 remains.
        val moods = listOf(mood("strong", now - 5 * day)) + threeFineMoods()
        val r = ReadinessAdvisor.evaluate(sessions(2), moods, emptyList(), now)
        assertNotNull(r)
        assertEquals(1, r!!.percent)
        assertTrue(!r.reason.contains("strong"))
    }

    // ── Vacation-aware spacing (#135) ──────────────────────────────────────────

    private fun date(ms: Long) = LocalDate.ofInstant(Instant.ofEpochMilli(ms), ZoneOffset.UTC)

    @Test
    fun comebackCautionFiresWithoutVacation() {
        val r = ReadinessAdvisor.evaluate(sessions(6), threeFineMoods(), emptyList(), now, ZoneOffset.UTC)
        assertNotNull(r)
        assertEquals(-3, r!!.percent) // fine mood (0) + 6-day comeback (−3)
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
        val r = ReadinessAdvisor.evaluate(sessions(6), threeFineMoods(), emptyList(), now, ZoneOffset.UTC, onVac)
        assertNull(r) // fine mood (0) + no comeback → net zero → silent
    }

    @Test
    fun onlyNonVacationDaysCountTowardTimeOff() {
        // 6-day gap, but 4 of the days were holiday → effective 2 days off → "fresh" bonus, not comeback.
        val today = date(now)
        val onVac = VacationCalendar.onVacation(
            listOf(VacationPeriod(startDate = date(now - 5 * day).toString(), endDate = date(now - 2 * day).toString()))
        )
        val r = ReadinessAdvisor.evaluate(sessions(6), threeFineMoods(), emptyList(), now, ZoneOffset.UTC, onVac)
        assertNotNull(r)
        assertEquals(1, r!!.percent) // fine (0) + fresh-after-2 (+1)
        assertTrue(r.reason.contains("fresh"))
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSameScale() {
        val moods = listOf(mood("strong", now - 6 * hour)) + threeFineMoods()
        assertEquals(
            ReadinessAdvisor.evaluate(sessions(2), moods, emptyList(), now),
            ReadinessAdvisor.evaluate(sessions(2), moods, emptyList(), now)
        )
    }
}
