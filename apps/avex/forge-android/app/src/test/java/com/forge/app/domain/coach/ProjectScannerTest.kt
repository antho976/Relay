package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.domain.adapt.SleepNight
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** D's weakness hunter: one lever at a time, each with a finish line. */
class ProjectScannerTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 200 * day

    private fun set() = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "100", weightLb = 100.0, reps = 8, completedAt = 0
    )

    private fun bout(daysAgo: Int, sets: Int, skipped: Boolean = false) = ExerciseBout(
        sessionStartedAt = now - daysAgo * day, effort = null, hitFullTarget = true,
        skipped = skipped, swappedName = null, sets = List(sets) { set() }
    )

    private fun slot(id: String, muscle: MuscleGroup) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(
        slots: List<ProgramSlotSnap>,
        history: Map<String, List<ExerciseBout>>,
        cardio: List<CardioEntry> = listOf(CardioEntry(1, date = now - day, type = "run", durationMin = 200)),
        health: HealthSnap = HealthSnap(sleepNights = (1..14).map { SleepNight(now - it * day, 480) })
    ) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("full", "Full", slots)),
        sessions = (1..10).map {
            Session(id = it.toLong(), dayKey = "full", startedAt = now - it * 3 * day, finishedAt = now - it * 3 * day + 1)
        },
        exerciseHistory = history,
        cardio = cardio,
        prefs = PrefsSnap(),
        health = health
    )

    /** A snapshot with no lever worth naming: balanced, conditioned, rested, adherent. */
    private fun healthy() = snapshot(
        slots = listOf(slot("bench", MuscleGroup.CHEST), slot("row", MuscleGroup.BACK)),
        history = mapOf(
            "bench" to listOf(bout(2, 5), bout(5, 5)),
            "row" to listOf(bout(3, 5), bout(6, 5))
        )
    )

    // ── Nothing to fix ─────────────────────────────────────────────────────────

    @Test
    fun aBalancedAthleteGetsNoImbalanceProject() {
        assertTrue(ProjectScanner.scan(healthy()).none { it.kind == ProjectScanner.Kind.IMBALANCE })
    }

    // ── Each lever ─────────────────────────────────────────────────────────────

    @Test
    fun aRealImbalanceIsNamedWithItsGap() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST), slot("row", MuscleGroup.BACK)),
            history = mapOf(
                "bench" to listOf(bout(2, 10), bout(5, 10)),
                "row" to listOf(bout(3, 5), bout(6, 5))
            )
        )
        val c = ProjectScanner.scan(s).first { it.kind == ProjectScanner.Kind.IMBALANCE }
        assertTrue(c.why.contains("%"))
        assertTrue(c.finishLine.isNotBlank())
        assertEquals(BalancePair.PUSH_PULL.code, c.targetKey)
    }

    @Test
    fun missingConditioningIsALever() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST)),
            history = mapOf("bench" to listOf(bout(2, 5))),
            cardio = emptyList()
        )
        val c = ProjectScanner.scan(s).first { it.kind == ProjectScanner.Kind.MISSING_CONDITIONING }
        assertTrue(c.finishLine.contains("four weeks"))
    }

    @Test
    fun shortSleepOutranksMostThings() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST)),
            history = mapOf("bench" to listOf(bout(2, 5))),
            health = HealthSnap(sleepNights = (1..14).map { SleepNight(now - it * day, 330) })
        )
        val top = ProjectScanner.top(s)
        assertNotNull(top)
        assertEquals(ProjectScanner.Kind.SHORT_SLEEP, top!!.kind)
        assertTrue(top.why.contains("h a night"))
    }

    @Test
    fun repeatedlySkippedWorkIsNamed() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST)),
            history = mapOf(
                "bench" to listOf(bout(20, 5), bout(15, 3, skipped = true), bout(10, 3, skipped = true), bout(5, 3, skipped = true), bout(2, 3))
            )
        )
        val c = ProjectScanner.scan(s).first { it.kind == ProjectScanner.Kind.SKIPPED_WORK }
        assertTrue(c.name.contains("bench"))
        assertTrue(c.why.contains("skipped"))
    }

    // ── The discipline ─────────────────────────────────────────────────────────

    @Test
    fun everyCandidateStatesWhyPlanAndFinishLine() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST), slot("row", MuscleGroup.BACK)),
            history = mapOf("bench" to listOf(bout(2, 10)), "row" to listOf(bout(3, 4))),
            cardio = emptyList(),
            health = HealthSnap(sleepNights = (1..14).map { SleepNight(now - it * day, 330) })
        )
        val all = ProjectScanner.scan(s)
        assertTrue(all.isNotEmpty())
        all.forEach {
            assertTrue("${it.kind} needs a name", it.name.isNotBlank())
            assertTrue("${it.kind} needs a why", it.why.isNotBlank())
            assertTrue("${it.kind} needs a plan", it.plan.isNotBlank())
            assertTrue("${it.kind} needs a finish line", it.finishLine.isNotBlank())
        }
    }

    @Test
    fun theTopLeverIsTheBiggestOne() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST), slot("row", MuscleGroup.BACK)),
            history = mapOf("bench" to listOf(bout(2, 10)), "row" to listOf(bout(3, 4))),
            cardio = emptyList(),
            health = HealthSnap(sleepNights = (1..14).map { SleepNight(now - it * day, 330) })
        )
        val scanned = ProjectScanner.scan(s)
        assertEquals(scanned.first(), ProjectScanner.top(s))
    }

    @Test
    fun anAbandonedKindIsNotProposedAgain() {
        val s = snapshot(
            slots = listOf(slot("bench", MuscleGroup.CHEST)),
            history = mapOf("bench" to listOf(bout(2, 5))),
            health = HealthSnap(sleepNights = (1..14).map { SleepNight(now - it * day, 330) })
        )
        val next = ProjectScanner.top(s, excludeKinds = setOf(ProjectScanner.Kind.SHORT_SLEEP))
        assertTrue(next == null || next.kind != ProjectScanner.Kind.SHORT_SLEEP)
    }

    @Test
    fun deterministic() {
        val s = healthy()
        assertEquals(ProjectScanner.scan(s), ProjectScanner.scan(s))
    }
}
