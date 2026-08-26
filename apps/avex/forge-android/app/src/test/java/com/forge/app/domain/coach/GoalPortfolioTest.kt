package com.forge.app.domain.coach

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A2's Goal Portfolio: readings always, trajectories only when the data earns them. */
class GoalPortfolioTest {

    private val day = 24L * 60 * 60 * 1000
    private val week = 7 * day
    private val now = 200 * day

    /**
     * A mid-week "now" for the two WEEK-SCOPED goals (MUSCLE_VOLUME, CONDITIONING).
     *
     * Those two measure the ISO week the user sees on Stats — Monday 00:00 to now — not a rolling
     * 7 x 24 h window. Epoch day 200 lands on a MONDAY at 00:00 UTC, so with [now] as the clock
     * "this week" is zero seconds long and every fixture row falls into LAST week: the tests read
     * 0 while claiming to prove what this week counts. Anchoring them three days later gives the
     * week somewhere to put data, and makes the Monday boundary the thing under test.
     */
    private val midWeek = now + 3 * day

    private fun goal(
        kind: CoachGoalKind,
        target: Double? = null,
        targetKey: String = "",
        priority: Int = 0,
        note: String = "",
        id: Long = kind.ordinal + 1L
    ) = CoachGoal(
        id = id, kind = kind.code, targetKey = targetKey, targetValue = target,
        priority = priority, createdAt = now - 30 * day, note = note
    )

    private fun set(weight: Double, reps: Int = 8) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = reps, completedAt = 0
    )

    private fun bout(
        atDaysAgo: Int,
        weight: Double,
        sessionType: String = "normal",
        from: Long = now
    ) = ExerciseBout(
        sessionStartedAt = from - atDaysAgo * day, effort = null, hitFullTarget = true,
        skipped = false, swappedName = null, sets = listOf(set(weight)), sessionType = sessionType
    )

    private fun slot(id: String, muscle: MuscleGroup = MuscleGroup.CHEST) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(
        history: Map<String, List<ExerciseBout>> = emptyMap(),
        sessions: List<Session> = emptyList(),
        cardio: List<CardioEntry> = emptyList(),
        bodyweight: List<BodyweightEntry> = emptyList(),
        slots: List<ProgramSlotSnap> = listOf(slot("bench")),
        nowMs: Long = now
    ) = AdaptationSnapshot(
        nowMs = nowMs,
        program = listOf(ProgramDaySnap("upper", "Upper", slots)),
        sessions = sessions,
        exerciseHistory = history,
        cardio = cardio,
        bodyweight = bodyweight,
        prefs = PrefsSnap()
    )

    // ── Cold start ─────────────────────────────────────────────────────────────

    @Test
    fun liftGoalWithNoHistory_readsHonestly_withNoTrajectory() {
        val g = goal(CoachGoalKind.LIFT_1RM, target = 225.0, targetKey = "bench")
        val state = GoalPortfolio.evaluate(listOf(g), snapshot()).single()
        assertNull(state.current)
        assertNull(state.etaWeeks)
        assertEquals("no lifts logged yet", state.reading)
    }

    @Test
    fun completedGoalsAreNotEvaluated() {
        val done = goal(CoachGoalKind.LIFT_1RM, target = 225.0, targetKey = "bench")
            .copy(completedAt = now - day)
        assertTrue(GoalPortfolio.evaluate(listOf(done), snapshot()).isEmpty())
    }

    @Test
    fun unknownKindIsIgnored_notCrashed() {
        val weird = CoachGoal(id = 9, kind = "teleportation", createdAt = now)
        assertTrue(GoalPortfolio.evaluate(listOf(weird), snapshot()).isEmpty())
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun liftGoal_projectsAnEtaFromItsOwnTrend() {
        // Climbing e1RM over 8 weeks toward a target above the current best.
        val bouts = (0 until 9).map { i -> bout(atDaysAgo = 56 - i * 7, weight = 150.0 + i * 5) }
        val g = goal(CoachGoalKind.LIFT_1RM, target = 260.0, targetKey = "bench")
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(history = mapOf("bench" to bouts))).single()
        assertNotNull(state.current)
        assertNotNull(state.perWeek)
        assertTrue("rate should be positive", state.perWeek!! > 0)
        assertNotNull("a climbing lift with a target ahead has an ETA", state.etaWeeks)
        assertEquals(true, state.onTrack)
        assertTrue(state.reading.contains("of 260 lb"))
    }

    @Test
    fun liftGoal_reachedIsReported() {
        val bouts = (0 until 6).map { i -> bout(atDaysAgo = 35 - i * 7, weight = 200.0 + i) }
        val g = goal(CoachGoalKind.LIFT_1RM, target = 100.0, targetKey = "bench")
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(history = mapOf("bench" to bouts))).single()
        assertTrue(state.reachedNow)
    }

    @Test
    fun testDayBoutsDoNotInflateALiftGoal() {
        val training = (0 until 6).map { i -> bout(atDaysAgo = 35 - i * 7, weight = 150.0) }
        val withTest = training + bout(atDaysAgo = 1, weight = 400.0, sessionType = "test")
        val g = goal(CoachGoalKind.LIFT_1RM, target = 260.0, targetKey = "bench")
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(history = mapOf("bench" to withTest))).single()
        assertTrue("a test single must not become the current reading", state.current!! < 300.0)
    }

    @Test
    fun consistencyGoal_measuresOverFourWeeks() {
        val sessions = (0 until 12).map { i ->
            Session(id = i + 1L, dayKey = "upper", startedAt = now - i * 2 * day, finishedAt = now - i * 2 * day + 1)
        }
        val g = goal(CoachGoalKind.CONSISTENCY, target = 3.0)
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(sessions = sessions)).single()
        assertEquals(3.0, state.current!!, 0.01)
        assertEquals(true, state.onTrack)
    }

    @Test
    fun conditioningGoal_countsThisWeeksActiveMinutesOnly() {
        // Clock is Thursday; the week under test runs from Monday 00:00. See [midWeek].
        val cardio = listOf(
            CardioEntry(1, date = midWeek - 2 * day, type = "run", durationMin = 40),    // Tuesday
            CardioEntry(2, date = midWeek - 3 * day, type = "walk", durationMin = 30),   // Monday 00:00
            // A rest row and a row from before Monday must not count.
            CardioEntry(3, date = midWeek - day, type = "rest", durationMin = 0, restReason = "sore"),
            CardioEntry(4, date = midWeek - 4 * day, type = "run", durationMin = 60)     // last Sunday
        )
        val g = goal(CoachGoalKind.CONDITIONING, target = 150.0)
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(cardio = cardio, nowMs = midWeek)).single()
        assertEquals(70.0, state.current!!, 0.01)
        assertEquals(false, state.onTrack)
        assertTrue(state.reading.contains("70 of 150 min"))
    }

    @Test
    fun bodyweightGoal_readsSmoothedWeightAndDirection() {
        val entries = (0 until 10).map { i ->
            BodyweightEntry(dateKey = "d$i", weightLb = 200.0 - i, recordedAt = now - (9 - i) * 3 * day)
        }
        val g = goal(CoachGoalKind.BODYWEIGHT, target = 180.0)
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(bodyweight = entries)).single()
        assertNotNull(state.current)
        assertTrue("losing weight toward a lower target is on track", state.onTrack == true)
        assertNotNull(state.etaWeeks)
    }

    @Test
    fun bodyweightGoal_movingAwayHasNoEta() {
        // Gaining while the target is below — an ETA here would be a lie.
        val entries = (0 until 10).map { i ->
            BodyweightEntry(dateKey = "d$i", weightLb = 180.0 + i, recordedAt = now - (9 - i) * 3 * day)
        }
        val g = goal(CoachGoalKind.BODYWEIGHT, target = 170.0)
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(bodyweight = entries)).single()
        assertNull(state.etaWeeks)
        assertEquals(false, state.onTrack)
    }

    @Test
    fun muscleVolumeGoal_countsThisWeeksSetsOnTheMuscle() {
        // Clock is Thursday; the week under test runs from Monday 00:00. See [midWeek].
        val bouts = listOf(
            bout(atDaysAgo = 2, weight = 100.0, from = midWeek).copy(sets = List(4) { set(100.0) }),
            bout(atDaysAgo = 3, weight = 100.0, from = midWeek).copy(sets = List(3) { set(100.0) }),
            // Before Monday — the whole point of the week boundary.
            bout(atDaysAgo = 4, weight = 100.0, from = midWeek).copy(sets = List(9) { set(100.0) })
        )
        val g = goal(CoachGoalKind.MUSCLE_VOLUME, target = 12.0, targetKey = MuscleGroup.CHEST.code)
        val state = GoalPortfolio
            .evaluate(listOf(g), snapshot(history = mapOf("bench" to bouts), nowMs = midWeek))
            .single()
        assertEquals(7.0, state.current!!, 0.01)
        assertTrue(state.reading.contains("7 of 12 sets"))
    }

    @Test
    fun balanceGoal_readsTheRatioBetweenSides() {
        val slots = listOf(slot("bench", MuscleGroup.CHEST), slot("row", MuscleGroup.BACK))
        val history = mapOf(
            "bench" to listOf(bout(atDaysAgo = 3, weight = 100.0).copy(sets = List(12) { set(100.0) })),
            "row" to listOf(bout(atDaysAgo = 4, weight = 100.0).copy(sets = List(6) { set(100.0) }))
        )
        val g = goal(CoachGoalKind.BALANCE, targetKey = BalancePair.PUSH_PULL.code)
        val state = GoalPortfolio.evaluate(listOf(g), snapshot(history = history, slots = slots)).single()
        assertEquals(2.0, state.current!!, 0.01)
        assertEquals(false, state.onTrack)
    }

    // ── Conflicts ──────────────────────────────────────────────────────────────

    @Test
    fun cuttingWhileChasingAMax_isFlaggedAndSequenced() {
        val goals = listOf(
            goal(CoachGoalKind.BODYWEIGHT, target = 170.0, id = 1),
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", id = 2)
        )
        val c = GoalPortfolio.conflicts(goals).single()
        assertTrue(c.explanation.contains("recovery budget"))
        assertTrue(c.proposal.isNotBlank())
        assertEquals(GoalPortfolio.LESSON_GOALS_FIGHT, c.lessonId)
    }

    @Test
    fun compatibleGoalsRunInParallel() {
        val goals = listOf(
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", id = 1),
            goal(CoachGoalKind.CONSISTENCY, target = 4.0, id = 2),
            goal(CoachGoalKind.CONDITIONING, target = 120.0, id = 3)
        )
        assertTrue(GoalPortfolio.conflicts(goals).isEmpty())
    }

    @Test
    fun bigConditioningBlockAgainstAMax_isFlagged() {
        val goals = listOf(
            goal(CoachGoalKind.CONDITIONING, target = 300.0, id = 1),
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", id = 2)
        )
        assertEquals(1, GoalPortfolio.conflicts(goals).size)
    }

    @Test
    fun twoTargetsOnOneLift_areFlaggedAsADuplicate() {
        val goals = listOf(
            goal(CoachGoalKind.LIFT_1RM, target = 275.0, targetKey = "bench", id = 1),
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", id = 2)
        )
        val c = GoalPortfolio.conflicts(goals).single()
        assertNull("a duplicate isn't a physiology lesson", c.lessonId)
    }

    @Test
    fun archivedGoalsNeverConflict() {
        val goals = listOf(
            goal(CoachGoalKind.BODYWEIGHT, target = 170.0, id = 1).copy(archivedAt = now),
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", id = 2)
        )
        assertTrue(GoalPortfolio.conflicts(goals).isEmpty())
    }

    // ── Ordering + determinism ─────────────────────────────────────────────────

    @Test
    fun statesComeBackInPriorityOrder() {
        val goals = listOf(
            goal(CoachGoalKind.CONSISTENCY, target = 4.0, priority = 2, id = 1),
            goal(CoachGoalKind.LIFT_1RM, target = 315.0, targetKey = "bench", priority = 0, id = 2),
            goal(CoachGoalKind.CONDITIONING, target = 150.0, priority = 1, id = 3)
        )
        val kinds = GoalPortfolio.evaluate(goals, snapshot()).map { it.kind }
        assertEquals(
            listOf(CoachGoalKind.LIFT_1RM, CoachGoalKind.CONDITIONING, CoachGoalKind.CONSISTENCY),
            kinds
        )
    }

    @Test
    fun deterministic() {
        val goals = listOf(goal(CoachGoalKind.CONSISTENCY, target = 3.0))
        val snap = snapshot()
        assertEquals(GoalPortfolio.evaluate(goals, snap), GoalPortfolio.evaluate(goals, snap))
    }
}
