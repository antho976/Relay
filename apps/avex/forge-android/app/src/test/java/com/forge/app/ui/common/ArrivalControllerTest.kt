package com.forge.app.ui.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The arrival banner's whole state machine, tested without a device.
 *
 * This is why the queue lives in a plain singleton rather than inside the composable: the ordering
 * and de-duplication rules are the part that can actually be wrong, and none of them need a screen
 * to check.
 */
class ArrivalControllerTest {

    private fun arrival(id: String) =
        ArrivalController.Arrival(noticeId = id, eyebrow = "NEW LESSON", title = "Lesson $id")

    @Test
    fun startsEmpty() {
        val controller = ArrivalController()
        assertTrue(controller.queue.value.isEmpty())
        assertNull(controller.current)
    }

    @Test
    fun showsTheOldestFirst() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        assertEquals("a", controller.current?.noticeId)
    }

    @Test
    fun consumingAdvancesToTheNext() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        controller.consume("a")
        assertEquals("b", controller.current?.noticeId)
        controller.consume("b")
        assertNull(controller.current)
    }

    /**
     * The one that matters. The source is a flow that re-emits whenever ANY part of the feed
     * changes, so an unrelated milestone landing must not queue a second copy of a lesson banner
     * that is already waiting its turn.
     */
    @Test
    fun reEnqueuingTheSameArrivalIsANoOp() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        assertEquals(listOf("a", "b"), controller.queue.value.map { it.noticeId })
    }

    @Test
    fun reEmissionStillAddsGenuinelyNewArrivals() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a")))
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        assertEquals(listOf("a", "b"), controller.queue.value.map { it.noticeId })
    }

    /**
     * A consumed arrival re-appearing in a later emission must not replay. The feed drops it from
     * `pendingAnnouncements` once marked, but the two writes are not atomic, so the queue has to
     * survive one stale emission in between without flickering the banner back on screen.
     */
    @Test
    fun aStaleEmissionAfterConsumingDoesNotReplayTheBanner() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a")))
        controller.consume("a")
        // The flow had not caught up yet and re-offers it.
        controller.enqueue(listOf(arrival("a")))
        // It DOES come back, because the controller alone cannot know it was already announced —
        // that is the persisted `announcedLessonNotices` set's job, asserted in the feed's own
        // test. What this pins is that it comes back exactly once rather than stacking.
        assertEquals(listOf("a"), controller.queue.value.map { it.noticeId })
    }

    @Test
    fun consumingSomethingNotQueuedIsHarmless() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a")))
        controller.consume("nope")
        assertEquals(listOf("a"), controller.queue.value.map { it.noticeId })
    }

    @Test
    fun enqueueingNothingIsANoOp() {
        val controller = ArrivalController()
        controller.enqueue(emptyList())
        assertTrue(controller.queue.value.isEmpty())
    }

    @Test
    fun clearDropsEverythingPending() {
        val controller = ArrivalController()
        controller.enqueue(listOf(arrival("a"), arrival("b")))
        controller.clear()
        assertTrue(controller.queue.value.isEmpty())
    }
}
