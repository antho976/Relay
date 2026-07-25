package com.forge.app.domain.coach

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.HrvSample
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A2's declared-slot registry: the contract later phases satisfy without rearchitecting. */
class SignalRegistryTest {

    private fun snapshot(
        health: HealthSnap = HealthSnap(),
        moods: List<MoodEntry> = emptyList(),
        bodyweight: List<BodyweightEntry> = emptyList()
    ) = AdaptationSnapshot(
        nowMs = 0L, program = emptyList(), sessions = emptyList(), exerciseHistory = emptyMap(),
        moods = moods, bodyweight = bodyweight, prefs = PrefsSnap(), health = health
    )

    @Test
    fun everySlotHasAStableIdAndCopy() {
        val ids = SignalRegistry.slots.map { it.id }
        assertEquals("slot ids must be unique", ids.size, ids.toSet().size)
        SignalRegistry.slots.forEach {
            assertTrue("${it.id} needs a label", it.label.isNotBlank())
            assertTrue("${it.id} needs a 'reads' line", it.reads.isNotBlank())
        }
    }

    @Test
    fun theFutureIsDeclared_notHidden() {
        val coming = SignalRegistry.slots.filter { it.availability == SignalRegistry.Availability.COMING_SOON }
        assertTrue("the registry's whole point is declaring what's next", coming.size >= 4)
        listOf(
            SignalRegistry.PROTEIN_NUTRITION,
            SignalRegistry.HYDRATION_SUPPLEMENTS,
            SignalRegistry.CYCLE_READINESS,
            SignalRegistry.CONDITIONING
        ).forEach { assertNotNull("$it must be declared", SignalRegistry.slot(it)) }
    }

    @Test
    fun anActiveSlotWithoutDataReadsAsForming() {
        val empty = snapshot()
        val sleep = SignalRegistry.slot(SignalRegistry.SLEEP)!!
        assertEquals(SignalRegistry.Availability.AWAITING_DATA, SignalRegistry.statusFor(sleep, empty))
    }

    @Test
    fun anActiveSlotWithDataReadsAsActive() {
        val snap = snapshot(
            health = HealthSnap(
                sleepNights = listOf(SleepNight(endedAtMs = 1, durationMin = 400)),
                restingHr = listOf(RestingHrSample(timeMs = 1, bpm = 55)),
                hrv = listOf(HrvSample(timeMs = 1, rmssdMs = 60.0)),
                dailySteps = listOf(DailySteps(dayStartMs = 1, steps = 8000))
            ),
            moods = listOf(MoodEntry(dayKey = "d", mood = "good", recordedAt = 1)),
            bodyweight = listOf(BodyweightEntry(dateKey = "d", weightLb = 180.0, recordedAt = 1))
        )
        val live = SignalRegistry.statuses(snap)
            .filter { it.second == SignalRegistry.Availability.ACTIVE }
            .map { it.first.id }
        assertTrue(SignalRegistry.SLEEP in live)
        assertTrue(SignalRegistry.MOOD in live)
        assertTrue(SignalRegistry.BODYWEIGHT_GOAL in live)
        assertTrue(SignalRegistry.STRESS_HRV in live)
        assertTrue(SignalRegistry.DAILY_STEPS in live)
    }

    @Test
    fun aComingSoonSlotIsNeverPromotedByData() {
        val slot = SignalRegistry.slot(SignalRegistry.PROTEIN_NUTRITION)!!
        assertEquals(
            SignalRegistry.Availability.COMING_SOON,
            SignalRegistry.statusFor(slot, snapshot(moods = listOf(MoodEntry(dayKey = "d", mood = "good", recordedAt = 1))))
        )
    }

    @Test
    fun statusesCoverEverySlot() {
        assertEquals(SignalRegistry.slots.size, SignalRegistry.statuses(snapshot()).size)
    }
}
