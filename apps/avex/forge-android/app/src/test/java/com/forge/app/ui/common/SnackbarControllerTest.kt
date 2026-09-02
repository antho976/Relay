package com.forge.app.ui.common

import com.forge.app.core.time.Clock
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Undo snackbar's whole lifecycle, tested without a device.
 *
 * The event is state with an id and an expiry rather than a channel send because a host that is
 * disposed mid-snackbar (Activity recreation on rotation) must acknowledge nothing: the event has
 * to still be there for the next host to replay. Only the two real outcomes clear it — the action
 * taken, or the window closing on the clock — and each names the id it means.
 */
class SnackbarControllerTest {

    private var now = 1_000L
    private val controller = SnackbarController(Clock { now })

    @Test
    fun startsWithNothingLive() {
        assertNull(controller.current.value)
    }

    @Test
    fun anUndoEventCarriesItsActionAndAWindowMeasuredFromTheClock() {
        var undone = false
        controller.showUndo("Entry deleted") { undone = true }
        val event = controller.current.value
        assertNotNull(event)
        assertEquals("Entry deleted", event!!.message)
        assertEquals("Undo", event.actionLabel)
        assertEquals(now + SnackbarController.WINDOW_MS, event.expiresAtMs)
        assertEquals(SnackbarController.WINDOW_MS, controller.remainingMs(event))
        assertNotNull(event.onAction)
        assertTrue("posting never runs the action", !undone)
    }

    @Test
    fun aPlainLineHasNoAction() {
        controller.show("Saved")
        val event = controller.current.value!!
        assertNull(event.actionLabel)
        assertNull(event.onAction)
    }

    /** The rotation case: nothing acknowledged the event, so it is still there to replay. */
    @Test
    fun anEventSurvivesUntilAnOutcomeNamesIt() {
        controller.showUndo("Goal deleted") {}
        val posted = controller.current.value!!
        now += 1_500
        assertSame("same event, same id, same expiry", posted, controller.current.value)
        assertEquals(SnackbarController.WINDOW_MS - 1_500, controller.remainingMs(posted))
    }

    @Test
    fun theWindowClosesOnTheClockNotOnTheHost() {
        controller.showUndo("Goal deleted") {}
        val posted = controller.current.value!!
        now += SnackbarController.WINDOW_MS
        assertTrue("nothing left to replay", controller.remainingMs(posted) <= 0L)
        // ...but the controller does not clear itself; the host that observes the closed window does.
        assertSame(posted, controller.current.value)
        controller.dismiss(posted.id)
        assertNull(controller.current.value)
    }

    @Test
    fun newestWins() {
        controller.showUndo("first") {}
        val first = controller.current.value!!
        controller.showUndo("second") {}
        val second = controller.current.value!!
        assertEquals("second", second.message)
        assertTrue("ids are distinct and increasing", second.id > first.id)
    }

    @Test
    fun dismissingAReplacedPredecessorLeavesTheSuccessorAlone() {
        controller.showUndo("first") {}
        val first = controller.current.value!!
        controller.showUndo("second") {}
        val second = controller.current.value!!

        controller.dismiss(first.id)   // the first host's late "timed out"
        assertSame(second, controller.current.value)

        controller.dismiss(second.id)
        assertNull(controller.current.value)
    }

    @Test
    fun takeHandsTheActionBackExactlyOnce() {
        var undos = 0
        controller.showUndo("Entry deleted") { undos++ }
        val posted = controller.current.value!!

        val taken = controller.take(posted.id)
        assertNotNull(taken)
        assertSame(posted, taken)
        assertNull("taking clears it", controller.current.value)
        assertNull("a second tap finds nothing", controller.take(posted.id))
        assertEquals("the controller never runs the action itself", 0, undos)
    }

    @Test
    fun takingAStaleIdIsANoOp() {
        controller.showUndo("first") {}
        val first = controller.current.value!!
        controller.showUndo("second") {}
        val second = controller.current.value!!

        assertNull("the tap landed on a snackbar a newer event replaced", controller.take(first.id))
        assertSame(second, controller.current.value)
    }
}
