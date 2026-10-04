package com.tally.app.ui.common

import com.tally.app.data.FixedClock
import com.tally.app.data.RoomTestDb
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.planRepository
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

/**
 * The one snackbar's rules: the newest notice wins, an Undo reaches only the notice still on
 * screen and runs once, and a restore that fails is reported instead of crashing the app. The
 * notices run their restores in the test's own scope, so an escaped exception fails the test.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class NoticesTest {

    @Test fun theNewestNoticeReplacesTheOneOnScreenAndOnlyItsUndoRuns() = runTest {
        val notices = Notices(this)
        val restored = mutableListOf<String>()
        notices.showUndo("Coffee entry deleted") { restored += "Coffee" }
        val coffee = notices.current.value!!
        notices.showUndo("Lunch entry deleted") { restored += "Lunch" }
        val lunch = notices.current.value!!
        assertEquals("Lunch entry deleted", lunch.message)

        // A late tap on the snackbar that was replaced: Coffee's delete already stands.
        notices.settle(coffee, undone = true)
        advanceUntilIdle()
        assertTrue(restored.isEmpty())
        assertSame("The older notice cannot clear the newer one", lunch, notices.current.value)

        notices.settle(lunch, undone = true)
        advanceUntilIdle()
        assertEquals(listOf("Lunch"), restored)
        assertNull(notices.current.value)
    }

    @Test fun anUndoRunsOnceAndATimeoutRunsNone() = runTest {
        val notices = Notices(this)
        var runs = 0
        notices.showUndo("Rent deleted") { runs++ }
        val bill = notices.current.value!!
        notices.settle(bill, undone = true)
        notices.settle(bill, undone = true)
        advanceUntilIdle()
        assertEquals(1, runs)

        notices.showUndo("Lisbon trip deleted") { runs++ }
        notices.settle(notices.current.value!!, undone = false)
        advanceUntilIdle()
        assertEquals("A timeout or a swipe lets the delete stand", 1, runs)
        assertNull(notices.current.value)
    }

    @Test fun theNoticeStaysUntilItIsSettledSoANewHostCanShowItAgain() = runTest {
        val notices = Notices(this)
        notices.show("Copied to today")
        val copied = notices.current.value!!
        // A rotation cancels the host's wait and settles nothing.
        assertSame(copied, notices.current.value)

        notices.show("Copied to today")
        val again = notices.current.value!!
        assertTrue("The same words posted twice are two notices", again !== copied)
        notices.settle(again, undone = false)
        assertNull(notices.current.value)
    }

    @Test fun leavingTheAppLetsTheActOnScreenStand() = runTest {
        val notices = Notices(this)
        var runs = 0
        notices.showUndo("Rent deleted") { runs++ }
        val rent = notices.current.value!!
        notices.clear()
        // A tap that lands as the screen goes reaches nothing.
        notices.settle(rent, undone = true)
        advanceUntilIdle()
        assertEquals(0, runs)
        assertNull(notices.current.value)
    }

    @Test fun aRestoreThatThrowsIsReportedNotRaised() = runTest {
        val notices = Notices(this)
        notices.showUndo("Groceries entry deleted") { throw IllegalStateException("FOREIGN KEY constraint failed") }
        notices.settle(notices.current.value!!, undone = true)
        advanceUntilIdle()

        val shown = notices.current.value
        assertNotNull(shown)
        assertEquals(UNDO_FAILED, shown!!.message)
        assertNull("The report offers no Undo of its own", shown.undo)
    }

    @Test fun undoingAContributionWhoseGoalWasDeletedSaysSoInsteadOfCrashing() = runTest {
        val db = RoomTestDb.create()
        try {
            val plan = db.planRepository(FixedClock(LocalDate.of(2026, 5, 15)))
            val goal = plan.saveGoal(GoalEntity(name = "Lisbon trip", target = 1_000_00))
            val row = plan.deleteContribution(plan.contribute(goal, 250_00))!!
            val notices = Notices(this)
            notices.showUndo("Contribution removed") { plan.restoreContribution(row) }
            val visible = notices.current.value!!
            plan.deleteGoal(goal)

            notices.settle(visible, undone = true)
            val shown = notices.current.first { it != null }!!

            assertEquals(UNDO_FAILED, shown.message)
            assertTrue("Nothing was written for a goal that is gone", db.goals().allContributions().isEmpty())
        } finally {
            db.close()
        }
    }
}
