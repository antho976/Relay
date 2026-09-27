package com.forge.app.ui.coach

import com.forge.app.data.repo.CoachRecord
import com.forge.app.data.repo.CoachWatch
import com.forge.app.data.repo.TrackedLift
import com.forge.app.domain.adapt.Confidence
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.Recommendation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The wording of WHERE YOU STAND: every line from real counts, and a gate reads as progress. */
class CoachNowTest {

    private fun watch(
        sessions: Int = 20,
        score: Int? = 3,
        threshold: Int = 7,
        checks: List<DeloadAdvisor.FatigueCheck> = emptyList(),
        lifts: List<TrackedLift> = emptyList(),
        historyDays: Int = 28
    ) = CoachWatch(
        sessionsLogged = sessions, minSessions = 4, sessionsToGo = 0, autopilot = false,
        fatigueScore = score, fatigueThreshold = threshold,
        recoveryGateSessions = 6, historyDays = historyDays, recoveryWindowDays = 28,
        fatigueChecks = checks, trackedLifts = lifts, recoverySignals = emptyList(),
        learnedBiases = emptyList()
    )

    private fun state(
        watch: CoachWatch? = watch(),
        readiness: Recommendation.ReadinessScale? = null,
        advanced: Boolean = false,
        e1rm: Map<String, List<Double>> = emptyMap()
    ) = CoachViewModel.UiState(
        loading = false,
        advanced = advanced,
        watch = watch,
        readiness = readiness,
        e1rmBySlot = e1rm,
        readinessGateSessions = 3
    )

    @Test
    fun readinessNamesTheChangeAndItsReasons() {
        val line = readinessLine(
            state(readiness = Recommendation.ReadinessScale(-4, "slept badly · sore", Confidence.MEDIUM))
        )!!
        assertEquals("Targets 4% lighter", line.value)
        assertEquals("Slept badly · sore", line.sub)
    }

    @Test
    fun readinessPastItsGateWithNothingMovingItIsAsPlanned() {
        assertEquals("As planned", readinessLine(state())!!.value)
    }

    @Test
    fun readinessBelowItsGateCountsTowardIt() {
        val line = readinessLine(state(watch = watch(sessions = 1)))!!
        assertEquals("Forming", line.value)
        assertEquals("1 of 3 sessions logged, first read after the 3rd.", line.sub)
    }

    @Test
    fun recoveryUsesTheStatsPulseBands() {
        assertEquals("Fresh", recoveryLine(state(watch = watch(score = 2)))!!.value)
        assertEquals("Building", recoveryLine(state(watch = watch(score = 5)))!!.value)
        assertEquals("Deload soon", recoveryLine(state(watch = watch(score = 7)))!!.value)
    }

    @Test
    fun recoveryGivesTheFiredChecksAsItsReason() {
        val checks = listOf(
            DeloadAdvisor.FatigueCheck("Hard sets", "38%", 2, fired = true),
            DeloadAdvisor.FatigueCheck("Sleep", "6.1h", 2, fired = true),
            DeloadAdvisor.FatigueCheck("Session rating", "6.8", 0, fired = false)
        )
        assertEquals("5 of 7 toward a deload · Hard sets, Sleep", recoveryLine(state(watch = watch(score = 5, checks = checks)))!!.sub)
        assertEquals("3 of 7 toward a deload.", recoveryLine(state())!!.sub)
    }

    @Test
    fun recoveryBelowItsGateCountsTowardIt() {
        val line = recoveryLine(state(watch = watch(sessions = 2, score = null)))!!
        assertEquals("Forming", line.value)
        assertEquals("2 of 6 sessions logged.", line.sub)
    }

    @Test
    fun liftsCountTheClimbAndNameTheStall() {
        val lifts = listOf(
            TrackedLift("Bench Press", 9, true, slotId = "b"),
            TrackedLift("Back Squat", 8, true, slotId = "s"),
            TrackedLift("DB Row", 7, true, stalling = true, slotId = "r"),
            TrackedLift("Overhead Press", 1, false, slotId = "o")
        )
        val line = liftsLine(
            state(
                watch = watch(lifts = lifts),
                e1rm = mapOf(
                    "b" to listOf(200.0, 210.0),
                    "s" to listOf(300.0, 300.5),
                    "r" to listOf(150.0, 150.0)
                )
            )
        )!!
        assertEquals("1 of 3 climbing", line.value)
        assertEquals("DB Row stalled · 1 flat · 1 still forming", line.sub)
    }

    @Test
    fun liftsWithNoTrendYetSayWhenTheyRead() {
        val lifts = listOf(TrackedLift("Bench Press", 1, false, slotId = "b"))
        val line = liftsLine(state(watch = watch(lifts = lifts)))!!
        assertEquals("Forming", line.value)
        assertEquals("1 lift building history, first read after two sessions.", line.sub)
    }

    @Test
    fun theRecordSplitsAppliedChangesByVerdict() {
        val line = recordLine(CoachRecord(applied = 9, held = 6, missed = 1, watching = 2, anyProposed = true))!!
        assertEquals("6 of 9 held", line.value)
        assertEquals("1 missed · 2 still proving out", line.sub)
    }

    @Test
    fun theRecordIsAbsentUntilTheCoachHasProposedAnything() {
        assertNull(recordLine(CoachRecord()))
        assertEquals("None yet", recordLine(CoachRecord(anyProposed = true))!!.value)
    }

    @Test
    fun advancedTrackingKeepsOnlyWhatSignalsDoesNotDraw() {
        val labels = standingLines(
            state(advanced = true).copy(
                timeline = com.forge.app.data.repo.CoachTimeline(
                    trust = emptyList(), milestones = emptyList(), weeks = emptyList(),
                    record = CoachRecord(applied = 1, held = 1, anyProposed = true)
                )
            )
        ).map { it.label }
        assertEquals(listOf("Readiness today", "Changes applied"), labels)
    }

    @Test
    fun noLineCarriesABannedCharacter() {
        val lines = standingLines(
            state(readiness = Recommendation.ReadinessScale(2, "slept well", Confidence.MEDIUM))
        )
        assertTrue(lines.isNotEmpty())
        lines.forEach { l ->
            listOf(l.label, l.value, l.sub.orEmpty()).forEach { text ->
                assertFalse("em dash in \"$text\"", text.contains('—'))
                assertFalse("exclamation in \"$text\"", text.contains('!'))
            }
        }
    }
}
