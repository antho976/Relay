package com.forge.app.domain.engine

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.LifeEvents
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Engine's domain (E-A → E-D). The rule this suite exists to protect: at rung one — no watch,
 * no health data — every surface stays complete and makes NO zone claims.
 */
class ConditioningTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 100 * day

    private fun cardio(
        daysAgo: Int,
        minutes: Int,
        effort: String? = "easy",
        zone: String? = null,
        type: String = "run",
        km: Double? = null,
        rest: String? = null,
        conditions: String? = null,
        id: Long = daysAgo.toLong()
    ) = CardioEntry(
        id = id, date = now - daysAgo * day, type = type, durationMin = minutes,
        distanceKm = km, effort = effort, restReason = rest, hrZone = zone, conditions = conditions
    )

    // ── E-A: zones ─────────────────────────────────────────────────────────────

    @Test
    fun withNoAgeAndNoOverride_thereAreNoZoneClaims() {
        val p = ConditioningProfile()
        assertFalse(p.hasZones)
        assertNull(p.maxHr)
        assertNull(p.zoneFor(140))
        assertNull(p.bandFor(2))
    }

    @Test
    fun anAgeGivesTanakaZones() {
        val p = ConditioningProfile(ageYears = 30, restingHr = 60)
        assertEquals(187, p.maxHr)
        assertTrue(p.hasZones)
        val z2 = p.bandFor(2)!!
        assertTrue("zone 2 sits in a sane place", z2.first in 110..145)
    }

    @Test
    fun anExplicitMaxWinsOverTheEstimate() {
        val p = ConditioningProfile(maxHrOverride = 200, ageYears = 30)
        assertEquals(200, p.maxHr)
    }

    @Test
    fun zonesRiseWithHeartRate() {
        val p = ConditioningProfile(maxHrOverride = 190, restingHr = 50)
        val easy = p.zoneFor(110)!!
        val hard = p.zoneFor(175)!!
        assertTrue(easy < hard)
        assertEquals(5, p.zoneFor(189))
    }

    @Test
    fun oneHrSpikeNeverShiftsEveryZone() {
        val p = ConditioningProfile(maxHrOverride = 185)
        val spiked = p.refinedWith(sustainedPeakBpm = 210, heldSeconds = 3)
        assertEquals("a 3-second artifact must not recalibrate anything", 185, spiked.maxHr)
        val sustained = p.refinedWith(sustainedPeakBpm = 192, heldSeconds = 45)
        assertEquals(192, sustained.maxHr)
    }

    @Test
    fun implausibleReadingsAreRejected() {
        val p = ConditioningProfile(maxHrOverride = 185)
        assertEquals(185, p.refinedWith(sustainedPeakBpm = 400, heldSeconds = 60).maxHr)
    }

    @Test
    fun effortWordsAreTheRungOneProxy() {
        assertEquals(2, EffortZones.zoneForEffort("easy"))
        assertEquals(4, EffortZones.zoneForEffort("hard"))
        assertNull(EffortZones.zoneForEffort(null))
        assertTrue(EffortZones.talkTestFor(2).contains("full sentences"))
    }

    // ── E-A: load ──────────────────────────────────────────────────────────────

    @Test
    fun loadUsesTheBestEvidenceAvailable() {
        // A logged zone beats the effort word; both beat nothing.
        assertEquals(3, ConditioningLoad.zoneOf(cardio(1, 30, effort = "easy", zone = "3")))
        assertEquals(4, ConditioningLoad.zoneOf(cardio(1, 30, effort = "hard")))
        assertEquals(2, ConditioningLoad.zoneOf(cardio(1, 30, effort = null)))
    }

    @Test
    fun harderWorkCostsMorePerMinute() {
        val easy = ConditioningLoad.of(cardio(1, 30, effort = "easy"))
        val hard = ConditioningLoad.of(cardio(1, 30, effort = "hard"))
        assertTrue(hard > easy)
    }

    @Test
    fun aRestDayCostsNothing() {
        assertEquals(0.0, ConditioningLoad.of(cardio(1, 0, rest = "sore")), 0.0001)
    }

    @Test
    fun interferenceIsBoundedAndOnlyFiresOnRealLoad() {
        val light = listOf(cardio(0, 20, effort = "easy"))
        val heavy = listOf(cardio(0, 60, effort = "hard"))
        assertEquals(0, ConditioningLoad.interferencePenalty(light, now))
        assertTrue(ConditioningLoad.interferencePenalty(heavy, now) in 1..2)
    }

    @Test
    fun rampRateComparesThisWeekToTheLastThree() {
        val steady = (1..21).map { cardio(it, 30, id = it.toLong()) }
        val ramp = ConditioningLoad.rampRate(steady, now)
        assertNotNull(ramp)
        assertEquals(1.0, ramp!!, 0.35)
    }

    // ── E-B: the coached week ──────────────────────────────────────────────────

    @Test
    fun anAthleteWhoDoesNothingGetsAStartingWeek() {
        val plan = ConditioningPlanner.planWeek(
            profile = ConditioningProfile(), weeklyTargetMinutes = 150,
            loggedThisWeek = emptyList(), nowMs = now, liftingDaysAhead = 4, weekdayMode = true
        )
        assertTrue(plan.isNotEmpty())
        assertTrue("rung one still gets an effort word", plan.all { it.effortWord.isNotBlank() })
        assertTrue("and no zone claims", plan.all { it.zoneBand == null })
    }

    @Test
    fun withZonesThePrescriptionCarriesTheBand() {
        val plan = ConditioningPlanner.planWeek(
            profile = ConditioningProfile(ageYears = 35, restingHr = 55), weeklyTargetMinutes = 150,
            loggedThisWeek = emptyList(), nowMs = now, liftingDaysAhead = 4, weekdayMode = true
        )
        assertTrue(plan.all { it.zoneBand != null })
    }

    @Test
    fun aFinishedWeekIsNotPaddedWithMoreWork() {
        val done = (1..5).map { cardio(it, 40, id = it.toLong()) }
        val plan = ConditioningPlanner.planWeek(
            profile = ConditioningProfile(), weeklyTargetMinutes = 150,
            loggedThisWeek = done, nowMs = now, liftingDaysAhead = 2, weekdayMode = true
        )
        assertTrue(plan.isEmpty())
    }

    @Test
    fun aDeloadWeekHalvesConditioningToo() {
        val normal = ConditioningPlanner.planWeek(
            ConditioningProfile(), 150, emptyList(), now, 4, weekdayMode = true
        ).sumOf { it.minutes }
        val deload = ConditioningPlanner.planWeek(
            ConditioningProfile(), 150, emptyList(), now, 4, weekdayMode = true,
            blockPhase = BlockPhase.DELOAD
        ).sumOf { it.minutes }
        assertTrue(deload < normal)
    }

    @Test
    fun illnessAndLayoffsSuspendConditioningEntirely() {
        val sick = ConditioningPlanner.planWeek(
            ConditioningProfile(), 150, emptyList(), now, 4, weekdayMode = true,
            life = LifeEvents.State.NONE.copy(sick = true)
        )
        assertTrue(sick.isEmpty())
    }

    @Test
    fun intervalsAlwaysCarryAWarmUpAndCoolDown() {
        val base = (1..3).map { cardio(it, 40, id = it.toLong()) }
        val plan = ConditioningPlanner.planWeek(
            ConditioningProfile(ageYears = 30), 250, base, now, liftingDaysAhead = 2, weekdayMode = true
        )
        plan.filter { it.structure == ConditioningPlanner.Structure.INTERVALS }.forEach {
            assertTrue("intervals need a warm-up", it.warmUpMinutes > 0)
            assertTrue("intervals need a cool-down", it.coolDownMinutes > 0)
            assertTrue(it.totalMinutes > it.minutes)
        }
    }

    @Test
    fun sequenceModeNeverClaimsToKnowYourLegDay() {
        val plan = ConditioningPlanner.planWeek(
            ConditioningProfile(), 150, emptyList(), now, 4, weekdayMode = false
        )
        assertTrue(plan.none { it.reason.contains("your next lower-body day") })
    }

    @Test
    fun stepMinutesNeverDoubleCountALoggedWalk() {
        val walk = listOf(cardio(1, 30, type = "walk"))
        // 30 logged minutes and 30 ambient step-minutes are the SAME half hour.
        assertEquals(30, ConditioningPlanner.healthFloorMinutes(walk, ambientStepMinutes = 30, nowMs = now))
    }

    // ── E-C: live zone coaching ────────────────────────────────────────────────

    @Test
    fun withoutZonesTheCoachStaysSilentForever() {
        var r = ZoneCoach.Reading.START
        repeat(20) { r = ZoneCoach.update(r, bpm = 180, band = null, elapsedSeconds = 10) }
        assertNull(r.alert)
    }

    @Test
    fun aBriefExcursionDoesNotNag() {
        var r = ZoneCoach.Reading.START
        r = ZoneCoach.update(r, bpm = 150, band = 120..140, elapsedSeconds = 10)
        assertNull("10 seconds high is not a drift", r.alert)
    }

    @Test
    fun sustainedDriftSpeaksOnce() {
        var r = ZoneCoach.Reading.START
        var alerts = 0
        repeat(12) {
            r = ZoneCoach.update(r, bpm = 155, band = 120..140, elapsedSeconds = 10)
            if (r.alert != null) alerts++
        }
        assertEquals("exactly one alert per episode", 1, alerts)
        assertEquals(ZoneCoach.State.DRIFTING_HIGH, r.state)
    }

    @Test
    fun comingBackResetsSoTheNextDriftCanSpeak() {
        var r = ZoneCoach.Reading.START
        repeat(8) { r = ZoneCoach.update(r, bpm = 155, band = 120..140, elapsedSeconds = 10) }
        repeat(5) { r = ZoneCoach.update(r, bpm = 130, band = 120..140, elapsedSeconds = 10) }
        assertEquals(ZoneCoach.State.IN_ZONE, r.state)
        var second = 0
        repeat(8) {
            r = ZoneCoach.update(r, bpm = 158, band = 120..140, elapsedSeconds = 10)
            if (r.alert != null) second++
        }
        assertEquals(1, second)
    }

    @Test
    fun aDroppedStreamIsNotADrift() {
        var r = ZoneCoach.Reading.START
        repeat(10) { r = ZoneCoach.update(r, bpm = null, band = 120..140, elapsedSeconds = 10) }
        assertNull(r.alert)
    }

    @Test
    fun anIntervalSessionBecomesRunnableSegments() {
        val p = ConditioningPlanner.Prescription(
            minutes = 16, zone = 4, zoneBand = null, effortWord = "hard",
            structure = ConditioningPlanner.Structure.INTERVALS, reason = "r", serves = "work capacity",
            warmUpMinutes = 8, coolDownMinutes = 5, intervals = 4
        )
        val segments = ZoneCoach.segmentsFor(p)
        assertEquals("Warm-up", segments.first().label)
        assertEquals("Cool-down", segments.last().label)
        assertEquals(4, segments.count { it.label.startsWith("Interval") })
    }

    // ── E-D: the base loop ─────────────────────────────────────────────────────

    @Test
    fun withoutComparableSessionsTheBaseIsUnknown() {
        val read = AerobicBase.assess(emptyList(), emptyList(), now)
        assertEquals(AerobicBase.Trend.UNKNOWN, read.trend)
        assertFalse(read.confident)
    }

    @Test
    fun gettingFasterAtTheSameEffortReadsAsImproving() {
        val older = (1..5).map { cardio(60 - it, 30, km = 5.0, id = it.toLong()) }
        val newer = (1..5).map { cardio(10 - it, 27, km = 5.0, id = 50L + it) }
        val read = AerobicBase.assess(older + newer, emptyList(), now)
        assertEquals(AerobicBase.Trend.IMPROVING, read.trend)
        assertTrue(read.confident)
    }

    @Test
    fun weatherConfoundedSessionsAreExcluded() {
        // Identical sessions, but the recent ones were logged in heat — which inflates HR and pace.
        val older = (1..5).map { cardio(60 - it, 30, km = 5.0, id = it.toLong()) }
        val hotRecent = (1..5).map { cardio(10 - it, 36, km = 5.0, conditions = "hot", id = 50L + it) }
        val read = AerobicBase.assess(older + hotRecent, emptyList(), now)
        assertEquals("a hot week is not lost fitness", AerobicBase.Trend.UNKNOWN, read.trend)
    }

    @Test
    fun restingHrAloneIsAWeakerRead() {
        val rhr = (1..8).map { RestingHrSample(now - it * day, 52) } +
            (15..30).map { RestingHrSample(now - it * day, 58) }
        val read = AerobicBase.assess(emptyList(), rhr, now)
        assertEquals(AerobicBase.Trend.IMPROVING, read.trend)
        assertFalse("one soft signal is not a confident verdict", read.confident)
    }

    @Test
    fun everyTrendCarriesAdvice() {
        AerobicBase.Trend.entries.forEach { trend ->
            val advice = AerobicBase.volumeAdvice(AerobicBase.BaseRead(trend, "r", true))
            assertTrue(advice.isNotBlank())
        }
    }
}
