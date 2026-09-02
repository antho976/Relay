package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * M-33: a bodyweight goal created before the first weigh-in must still get a fixed starting point.
 *
 * The baseline is what makes the meter mean anything — the bar measures travel FROM it. Created
 * with no weights on file it stored none, and every read then substituted the LATEST weight as the
 * start. The claimed start rewrote itself downward with each weigh-in, the meter sat at 0% however
 * far the user had come, and then jumped straight to reached.
 *
 * The audit's regression is the one pinned first: 200, then 190, toward 180.
 */
@RunWith(RobolectricTestRunner::class)
class BodyweightGoalBaselineTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private var now = 1_767_600_000_000L
    private val clock = Clock { now }

    private val repo = ExtendedGoalRepository(
        dao = db.extendedGoalDao(),
        sessionDao = db.sessionDao(),
        cardioDao = db.cardioDao(),
        bodyweightDao = db.bodyweightDao(),
        clock = clock
    )

    @After
    fun tearDown() = db.close()

    private suspend fun weighIn(lb: Double, dayKey: String) {
        db.bodyweightDao().upsert(
            BodyweightEntry(dateKey = dayKey, weightLb = lb, recordedAt = now)
        )
        now += 86_400_000L
    }

    private suspend fun bodyweightGoal() =
        repo.goalsWithProgress().single { it.metric == GoalMetric.BODYWEIGHT }

    @Test
    fun theFirstWeighInBecomesTheBaselineAndThenStopsMoving() = runTest {
        repo.create(GoalMetric.BODYWEIGHT, GoalPeriod.ALL, targetValue = 180.0)
        assertNull("nothing to start from yet", bodyweightGoal().baselineValue)

        weighIn(200.0, "2026-01-05")
        assertEquals("the first real weigh-in IS the start", 200.0, bodyweightGoal().baselineValue!!, 0.001)

        weighIn(190.0, "2026-01-06")
        val halfway = bodyweightGoal()
        assertEquals("and it does not move afterwards", 200.0, halfway.baselineValue!!, 0.001)
        assertEquals("200 → 190 of 200 → 180 is halfway", 0.5f, halfway.fraction, 0.01f)
        assertTrue("not reached at 190", !halfway.achieved)

        weighIn(180.0, "2026-01-07")
        val done = bodyweightGoal()
        assertEquals(200.0, done.baselineValue!!, 0.001)
        assertTrue("reached at the target", done.achieved)
    }

    @Test
    fun theBaselineIsWrittenBackRatherThanRecomputedEveryRead() = runTest {
        repo.create(GoalMetric.BODYWEIGHT, GoalPeriod.ALL, targetValue = 180.0)
        weighIn(200.0, "2026-01-05")
        bodyweightGoal() // the read that adopts it

        val stored = db.extendedGoalDao().getAll().single()
        assertEquals(200.0, stored.stretchValue!!, 0.001)
    }

    @Test
    fun aGoalCreatedAfterAWeighInKeepsTheBaselineItWasCreatedWith() = runTest {
        weighIn(210.0, "2026-01-04")
        repo.create(GoalMetric.BODYWEIGHT, GoalPeriod.ALL, targetValue = 180.0)

        weighIn(195.0, "2026-01-05")

        assertEquals(210.0, bodyweightGoal().baselineValue!!, 0.001)
    }
}
