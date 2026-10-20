package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.testing.MemoryOutbox
import com.quietsoftware.relay.core.testing.MemoryReplica
import com.quietsoftware.relay.core.testing.obj
import com.quietsoftware.relay.core.wire.Event
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** What the phone asks the PC to send it, and what each event means for the screens. */
@OptIn(ExperimentalCoroutinesApi::class)
class EventRouterTest {
    private fun event(ev: String, payload: JsonObject = obj()) = Event(ev, "2026-10-20T00:00:00Z", "agent:painter", null, 1, payload)

    private fun designChanged(rev: Int) = event("design.changed", obj("id" to 4, "project_id" to 1, "title" to "Onboarding", "rev" to rev))

    @Test
    fun `the phone subscribes to the events of an open design`() {
        assertTrue("design.*" in EventRouter.SUBSCRIBE)
        assertTrue(EventRouter.subscribes("design.changed"))
        assertTrue(EventRouter.subscribes("design.deleted"))
    }

    @Test
    fun `subscribes reads the list as the door's filter does`() {
        // socket.rs `Filter::matches`: exact, `prefix.*` at a dot, or `*`.
        assertTrue(EventRouter.subscribes("git.changed"))
        assertTrue(EventRouter.subscribes("provider.update.changed"))
        assertFalse(EventRouter.subscribes("designer.changed"))
        assertFalse(EventRouter.subscribes("design"))
        assertFalse(EventRouter.subscribes("ui.toast"))
        assertTrue(EventRouter.subscribes("ui.toast", listOf("ui.toast")))
        assertTrue(EventRouter.subscribes("anything.at.all", listOf("*")))
        assertTrue(EventRouter.subscribes("anything", emptyList()))
    }

    @Test
    fun `a design event marks the design screens stale and writes nothing`() {
        for (ev in listOf(designChanged(7), event("design.deleted", obj("id" to 4, "project_id" to 1)))) {
            assertEquals(ev.ev, Routed(stale = setOf("design")), EventRouter.route(ev))
        }
    }

    /**
     * Every domain the door lets the phone call (wire.rs `PHONE_OPS`) is one the phone hears
     * about, or is named here with why it needs no events: a screen showing it would otherwise
     * stay as it was first read for as long as it is open (B4).
     */
    @Test
    fun `every domain the door opens to the phone is subscribed or has no events`() {
        val noEvents = mapOf(
            "bus" to "the link's own ops; bus.lagged reaches every subscriber whatever it asked for",
            "app" to "the PC sends no app events; the backups page reads its list again after each backup",
            "dashboard" to "the PC sends no dashboard events; the syncer reads it again on the events that move it",
            "ui" to "the phone switches the PC's page and shows none of its window state",
        )
        val wire = File("../../../crates/relay-remote/src/wire.rs")
        assertTrue("${wire.absolutePath} not found: run from apps/relay-android/core in the Relay tree", wire.isFile)
        val ops = phoneOps(wire.readText())
        assertTrue("PHONE_OPS not read from wire.rs", "design.get" in ops && "task.list" in ops)
        val domains = ops.map { it.substringBefore('.') }.toSortedSet()
        val missing = domains.filter { "$it.*" !in EventRouter.SUBSCRIBE && it !in noEvents }
        assertEquals("add \"<domain>.*\" to EventRouter.SUBSCRIBE, or say here why the domain has no events", emptyList<String>(), missing)
        // An exclusion for a domain the phone no longer calls, or now subscribes to, is stale.
        assertEquals(emptyList<String>(), noEvents.keys.filter { it !in domains || "$it.*" in EventRouter.SUBSCRIBE })
    }

    @Test
    fun `a burst of design events costs an open design at most two reads, the last after the last event`() = runTest {
        val syncer = syncer()
        val reads = mutableListOf<Set<String>>()
        val first = CompletableDeferred<Unit>()
        backgroundScope.launch {
            syncer.stale.rereadsOf("design").collect {
                reads += it
                if (reads.size == 1) first.await()
            }
        }
        runCurrent()
        syncer.onEvent(designChanged(1))
        runCurrent()
        assertEquals(1, reads.size)
        // An agent's run of design.apply while the first read is still on the wire.
        for (rev in 2..40) syncer.onEvent(designChanged(rev))
        runCurrent()
        first.complete(Unit)
        runCurrent()
        assertEquals(2, reads.size)
        syncer.onEvent(designChanged(41))
        runCurrent()
        assertEquals(3, reads.size)
    }

    @Test
    fun `a screen reads again for its own domain and after a gap, not for others`() = runTest {
        val syncer = syncer()
        val reads = mutableListOf<Set<String>>()
        backgroundScope.launch { syncer.stale.rereadsOf("design").collect { reads += it } }
        runCurrent()
        syncer.onEvent(event("git.changed"))
        syncer.onEvent(event("task.deleted", obj("id" to 3)))
        runCurrent()
        assertEquals(emptyList<Set<String>>(), reads)
        syncer.onEvent(event("bus.lagged"))
        runCurrent()
        assertEquals(listOf(setOf("*")), reads)
    }

    /**
     * The syncer's stale marks are a shared flow with a bounded buffer: a screen that held its
     * subscriber through a slow read would fill it, and every later mark, for every screen,
     * would be dropped.
     */
    @Test
    fun `a slow design read never makes the syncer drop another screen's mark`() = runTest {
        val syncer = syncer()
        val held = CompletableDeferred<Unit>()
        var designReads = 0
        val gitReads = mutableListOf<Set<String>>()
        backgroundScope.launch {
            syncer.stale.rereadsOf("design").collect {
                designReads++
                held.await()
            }
        }
        backgroundScope.launch { syncer.stale.rereadsOf("git").collect { gitReads += it } }
        runCurrent()
        // Events come off the link one at a time, each with its turn on the dispatcher.
        for (rev in 1..200) {
            syncer.onEvent(designChanged(rev))
            runCurrent()
        }
        syncer.onEvent(event("git.changed"))
        runCurrent()
        assertEquals(1, designReads)
        assertEquals(1, gitReads.size)
        assertTrue("git" in gitReads.single())
        held.complete(Unit)
    }

    private fun TestScope.syncer() = Syncer(backgroundScope, Ledger(MemoryReplica(), MemoryOutbox()) { 0 }, MemoryReplica())

    /** The ops of wire.rs `PHONE_OPS`, comments left out. */
    private fun phoneOps(source: String): List<String> {
        val body = source.substringAfter("pub const PHONE_OPS: &[&str] = &[", "").substringBefore("];")
        return body.lines().map { it.substringBefore("//") }
            .flatMap { line -> Regex("\"([a-z_]+(?:\\.[a-z_]+)+)\"").findAll(line).map { it.groupValues[1] }.toList() }
    }
}
