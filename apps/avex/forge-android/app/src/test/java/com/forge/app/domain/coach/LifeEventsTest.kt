package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.entities.InjuryRestriction
import com.forge.app.data.db.entities.Session
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** B1's life events: illness, layoffs and injury — the adjustments v2 had no story for. */
class LifeEventsTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 200 * day

    private fun session(daysAgo: Int, id: Long = daysAgo.toLong()) = Session(
        id = id, dayKey = "upper", startedAt = now - daysAgo * day,
        finishedAt = now - daysAgo * day + 3_600_000
    )

    private fun checkin(daysAgo: Int, sick: Boolean = false, sore: Set<MuscleGroup> = emptySet()) =
        CheckinEntry(
            dateKey = "d$daysAgo", sick = sick,
            soreMuscles = sore.joinToString(",") { it.code },
            recordedAt = now - daysAgo * day
        )

    private fun assess(
        sessions: List<Session> = listOf(session(1), session(3), session(5)),
        checkins: List<CheckinEntry> = emptyList(),
        cardio: List<CardioEntry> = emptyList(),
        restrictions: List<InjuryRestriction> = emptyList()
    ) = LifeEvents.assess(sessions, checkins, cardio, restrictions, now)

    // ── Nothing happening ──────────────────────────────────────────────────────

    @Test
    fun ordinaryTrainingWeek_hasNoState() {
        val s = assess()
        assertFalse(s.sick)
        assertNull(s.layoff)
        assertTrue(s.soreMuscles.isEmpty())
        assertFalse(s.easeOff)
        assertEquals(1.0, s.loadScale, 0.0001)
        assertNull(LifeEvents.explain(s))
    }

    @Test
    fun noSessionsAtAll_isNotALayoff() {
        // A brand-new user hasn't taken time off, they haven't started.
        assertNull(LifeEvents.layoff(emptyList(), now))
    }

    // ── Sick ───────────────────────────────────────────────────────────────────

    @Test
    fun sickCheckin_setsTheFlagAndEasesOff() {
        val s = assess(checkins = listOf(checkin(daysAgo = 0, sick = true)))
        assertTrue(s.sick)
        assertTrue(s.easeOff)
        assertTrue(LifeEvents.explain(s)!!.contains("unwell"))
    }

    @Test
    fun sickFlagExpires() {
        assertFalse(assess(checkins = listOf(checkin(daysAgo = 5, sick = true))).sick)
    }

    @Test
    fun legacySickRestDay_feedsTheSameFlag() {
        // The rest-day reason predates the check-in and users still reach for it (plan M6).
        val cardio = listOf(
            CardioEntry(1, date = now - day, type = "rest", durationMin = 0, restReason = "sick")
        )
        assertTrue(assess(cardio = cardio).sick)
    }

    // ── Layoff ─────────────────────────────────────────────────────────────────

    @Test
    fun threeWeeksAway_readsAsAnOngoingLayoff() {
        val s = assess(sessions = listOf(session(21), session(24)))
        val layoff = s.layoff!!
        assertTrue(layoff.away)
        assertFalse(layoff.returning)
        assertEquals(21, layoff.days)
        assertTrue(s.easeOff)
        assertTrue(LifeEvents.explain(s)!!.contains("21 days"))
    }

    @Test
    fun aShortBreakIsNotALayoff() {
        assertNull(LifeEvents.layoff(listOf(session(6), session(9)), now))
    }

    @Test
    fun firstWeekBack_ramps() {
        // Trained 30 and 28 days ago, then a 20-day gap, back 2 days ago.
        val s = assess(sessions = listOf(session(30), session(28), session(2)))
        val layoff = s.layoff!!
        assertFalse(layoff.away)
        assertTrue(layoff.returning)
        assertEquals(26, layoff.days)
        assertEquals(LifeEvents.RAMP_SCALE, s.loadScale, 0.0001)
        assertTrue(LifeEvents.explain(s)!!.contains("First week back"))
    }

    @Test
    fun rampExpiresAfterAWeekOfTraining() {
        // Back 10 days ago after a long gap — the ramp window has passed.
        val s = assess(sessions = listOf(session(40), session(38), session(10), session(2)))
        assertTrue(s.layoff == null || !s.layoff!!.returning)
        assertEquals(1.0, s.loadScale, 0.0001)
    }

    // ── Verdict suppression ────────────────────────────────────────────────────

    @Test
    fun aWindowSpentAwayIsNotJudged() {
        val s = assess(sessions = listOf(session(30), session(28), session(2)))
        // A decision applied just before the gap, whose 14-day window ran through it.
        val appliedAt = now - 27 * day
        assertTrue(LifeEvents.suppressesVerdict(appliedAt, appliedAt + 14 * day, s))
    }

    @Test
    fun aWindowAfterTheReturnIsJudgedNormally() {
        val s = assess(sessions = listOf(session(30), session(28), session(2)))
        val appliedAt = now - day
        assertFalse(LifeEvents.suppressesVerdict(appliedAt, appliedAt + 14 * day, s))
    }

    @Test
    fun illnessSuppressesEveryOpenWindow() {
        val s = assess(checkins = listOf(checkin(daysAgo = 0, sick = true)))
        assertTrue(LifeEvents.suppressesVerdict(now - 10 * day, now + 4 * day, s))
    }

    // ── Soreness + injury ──────────────────────────────────────────────────────

    @Test
    fun checkinSorenessGatesNamedMusclesOnly() {
        val s = assess(checkins = listOf(checkin(0, sore = setOf(MuscleGroup.QUADS, MuscleGroup.GLUTES))))
        assertEquals(setOf(MuscleGroup.QUADS, MuscleGroup.GLUTES), s.soreMuscles)
        // Soreness gates, it doesn't ban — that's what a restriction is for.
        assertFalse(s.isRestricted(MuscleGroup.QUADS))
    }

    @Test
    fun sorenessExpiresWithinTwoDays() {
        assertTrue(assess(checkins = listOf(checkin(3, sore = setOf(MuscleGroup.QUADS)))).soreMuscles.isEmpty())
    }

    @Test
    fun activeRestrictionsBlockTheirTarget() {
        val restrictions = listOf(
            InjuryRestriction(scope = InjuryRestriction.SCOPE_MUSCLE, targetKey = MuscleGroup.SHOULDERS.code, startedAt = now - 3 * day),
            InjuryRestriction(scope = InjuryRestriction.SCOPE_EXERCISE, targetKey = "ohp", startedAt = now - 3 * day),
            // Cleared: kept in history, no longer applied.
            InjuryRestriction(scope = InjuryRestriction.SCOPE_MUSCLE, targetKey = MuscleGroup.BACK.code, startedAt = now - 40 * day, clearedAt = now - 10 * day)
        )
        val s = assess(restrictions = restrictions)
        assertTrue(s.isRestricted(MuscleGroup.SHOULDERS))
        assertTrue(s.isRestricted("ohp"))
        assertFalse(s.isRestricted(MuscleGroup.BACK))
        assertNotNull(LifeEvents.explain(s))
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun deterministic() {
        val sessions = listOf(session(30), session(28), session(2))
        assertEquals(LifeEvents.layoff(sessions, now), LifeEvents.layoff(sessions.shuffled(), now))
    }
}
