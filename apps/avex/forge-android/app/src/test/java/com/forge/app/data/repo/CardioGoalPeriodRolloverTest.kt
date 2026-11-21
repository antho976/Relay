package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDateTime
import java.time.ZoneId

/**
 * M-32: a periodic goal's progress is derived from the instant it is read at, so the instant is an
 * input and the screen has to be told when it moves.
 *
 * Nothing about a goal ROW changes at midnight. `cardioGoalsFlow` recomputed only when a goal or a
 * cardio entry was written, so a tab left open from Sunday into Monday kept rendering last week's
 * completed 4 / 4 in a week where nothing had happened yet — and the deadline caption beneath it
 * counted down to a deadline that had already passed. Home's goal flow was given the day signal in
 * the same pass and this one was not.
 *
 * Parameterising the read is what makes that expressible at all: with the clock read inside, this
 * could only be tested on a day the suite happened to run.
 */
@RunWith(RobolectricTestRunner::class)
class CardioGoalPeriodRolloverTest {

    private val zone: ZoneId = ZoneId.systemDefault()
    private val db: ForgeDatabase = inMemoryForgeDb()

    private fun at(y: Int, m: Int, d: Int, hour: Int = 12): Long =
        LocalDateTime.of(y, m, d, hour, 0).atZone(zone).toInstant().toEpochMilli()

    // Monday 2026-06-22 … Sunday 2026-06-28; the next week starts Monday 2026-06-29.
    private val sunday = at(2026, 6, 28)
    private val nextMonday = at(2026, 6, 29)

    private val repo = ExtendedGoalRepository(
        dao = db.extendedGoalDao(),
        sessionDao = db.sessionDao(),
        cardioDao = db.cardioDao(),
        bodyweightDao = db.bodyweightDao(),
        clock = Clock { sunday }
    )

    @After
    fun tearDown() = db.close()

    private suspend fun logRun(atMs: Long, minutes: Int) {
        db.cardioDao().insert(
            CardioEntry(date = atMs, type = CardioType.RUN.code, durationMin = minutes)
        )
    }

    private suspend fun minutesGoal(period: GoalPeriod, target: Double) =
        repo.create(GoalMetric.CARDIO_MINUTES, period, targetValue = target)

    private suspend fun progressAt(nowMs: Long) = repo.goalsWithProgress(nowMs).single()

    @Test
    fun aWeeklyGoalResetsAtTheWeekBoundaryWithNoRowChanging() = runTest {
        minutesGoal(GoalPeriod.WEEK, target = 90.0)
        logRun(at(2026, 6, 23), minutes = 50)
        logRun(at(2026, 6, 25), minutes = 40)

        val onSunday = progressAt(sunday)
        assertEquals(90.0, onSunday.currentValue, 0.001)
        assertEquals("cleared, on the week it was cleared in", true, onSunday.achieved)

        // Not one row has changed. Only the instant has.
        val onMonday = progressAt(nextMonday)
        assertEquals(
            "last week's minutes belong to last week",
            0.0, onMonday.currentValue, 0.001
        )
        assertEquals(false, onMonday.achieved)
    }

    @Test
    fun aMonthlyGoalResetsAtTheMonthBoundaryWithNoRowChanging() = runTest {
        minutesGoal(GoalPeriod.MONTH, target = 100.0)
        logRun(at(2026, 6, 10), minutes = 60)
        logRun(at(2026, 6, 20), minutes = 40)

        assertEquals(100.0, progressAt(at(2026, 6, 30)).currentValue, 0.001)
        assertEquals(
            "June's minutes are not July's",
            0.0, progressAt(at(2026, 7, 1)).currentValue, 0.001
        )
    }

    @Test
    fun anAllTimeGoalIsUnmovedByTheCalendar() = runTest {
        minutesGoal(GoalPeriod.ALL, target = 100.0)
        logRun(at(2026, 6, 23), minutes = 90)

        assertEquals(90.0, progressAt(sunday).currentValue, 0.001)
        assertEquals(
            "an all-time total has no window to roll over",
            90.0, progressAt(at(2027, 1, 1)).currentValue, 0.001
        )
    }
}
