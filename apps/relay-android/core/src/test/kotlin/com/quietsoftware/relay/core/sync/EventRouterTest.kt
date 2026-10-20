package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.wire.Event
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertTrue
import org.junit.Test

/** The events the phone asks for reach the screens that read their ops. */
class EventRouterTest {
    private fun event(ev: String) = Event(ev, "2026-10-19T00:00:00Z", "user", null, 1, JsonObject(emptyMap()))

    @Test
    fun aCanvasChangedOnThePcRedrawsTheDesignScreens() {
        assertTrue("design.*" in EventRouter.SUBSCRIBE)
        val stale = EventRouter.route(event("design.changed")).stale
        assertTrue(QueryKey.domain(QueryKey.of("design.list")) in stale)
        assertTrue(QueryKey.domain(QueryKey.of("design.get")) in EventRouter.route(event("design.deleted")).stale)
    }

    @Test
    fun everyDomainThePhoneReadsAndKeepsIsSubscribedTo() {
        // Each op family a screen reads live, and so must hear changes to.
        for (domain in listOf("session", "task", "notes", "git", "file", "thread", "money", "arbiter", "gym", "design", "skill", "plugin")) {
            assertTrue("$domain.* is not subscribed", "$domain.*" in EventRouter.SUBSCRIBE)
        }
    }
}
