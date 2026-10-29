package com.quietsoftware.relay.core

import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.OutboxRunner
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.core.testing.FakeDoor
import com.quietsoftware.relay.core.testing.MemoryOutbox
import com.quietsoftware.relay.core.testing.MemoryReplica
import com.quietsoftware.relay.core.testing.obj
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/** The hub's own rules around the outbox: what a failed first read and a lost answer leave behind. */
class HubTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val replica = MemoryReplica()
    private val outbox = MemoryOutbox()
    private val route = "ws://192.168.1.20:7420"
    private val projectLists = AtomicInteger()
    private val taskGets = AtomicInteger()
    @Volatile private var answering = false
    private val sent = java.util.concurrent.CopyOnWriteArrayList<String>()

    private val door = FakeDoor(scope, ops = Wire.OPS_PC).also { d ->
        d.handle = { op, p -> pc(op, p) }
    }

    private fun pc(op: String, p: JsonObject): JsonElement = when (op) {
        // The first project list fails, as a busy engine just started by the door can.
        "project.list" -> if (projectLists.incrementAndGet() == 1) throw FakeDoor.Refuse("internal", "store.busy")
            else obj("projects" to listOf(obj("id" to 1, "name" to "Relay", "path" to "/r", "workspace_id" to 1)))
        "task.create" -> obj("id" to 100, "project_id" to 1, "title" to "Queued offline", "column" to "backlog")
        "task.get" -> obj("id" to 7, "project_id" to 1, "title" to "Read again", "column" to "backlog").also { taskGets.incrementAndGet() }
        "task.update" -> throw FakeDoor.Refuse("conflict", "bus.replay_unavailable", details = obj("applied" to true))
        "thread.send" -> {
            // One message at a time, as threads.rs: the agent is answering until the PC says it is done.
            if (answering) throw FakeDoor.Refuse("conflict", "thread.busy", "The agent is still answering in this thread")
            answering = true
            sent += (p["text"] as kotlinx.serialization.json.JsonPrimitive).content
            obj("id" to 900 + sent.size, "thread_id" to 31, "role" to "user", "body" to obj("text" to sent.last()), "created_at" to "2026-10-24T10:00:00.000000000Z")
        }
        else -> obj()
    }

    private val profile = PcProfile("h1", "desk", "dev", door.device, door.token, listOf(Route(route, Route.Kind.Lan)))
    private val source = object : Link.ProfileSource {
        override suspend fun current() = profile
        override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
    }

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `a failed first read still sends the outbox, then dials again and reads it all`() = runBlocking {
        val hub = Hub(scope, door, source, replica, outbox)
        hub.change("task.create", obj("project_id" to 1, "title" to "Queued offline"), "New task")
        hub.start()
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }
        withTimeout(15_000) { while (replica.all(Kind.Project).isEmpty()) delay(50) }
        assertTrue("dialled again after the failed read: ${door.dials}", door.dials >= 2)
        assertTrue(projectLists.get() >= 2)
    }

    @Test
    fun `two messages queued for one thread both go, the second once the first is answered`() = runBlocking {
        val hub = Hub(scope, door, source, replica, outbox)
        hub.change("thread.send", obj("id" to 31, "text" to "Am I on pace?"), "Message")
        hub.change("thread.send", obj("id" to 31, "text" to "And for groceries?"), "Message")
        hub.start()
        withTimeout(10_000) { while (sent.isEmpty()) delay(20) }
        delay(500)
        assertEquals("the second waits while the agent answers", listOf("Am I on pace?"), sent.toList())
        assertTrue("nothing parked", outbox.open().none { it.state == OutboxEntry.State.Failed })
        // The turn ends on the PC.
        answering = false
        door.emit("thread.changed", obj("thread" to obj("id" to 31, "title" to "Budget", "working" to false)))
        withTimeout(10_000) { while (sent.size < 2) delay(20) }
        assertEquals(listOf("Am I on pace?", "And for groceries?"), sent.toList())
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }
    }

    @Test
    fun `a turn that ends while a message is on its way does not leave the message waiting`() = runBlocking {
        // The agent is answering when the message goes; its turn ends while the PC's thread.busy is
        // on its way back, so the phone reads the end before the refusal it would have waited on.
        answering = true
        val handled = door.handle
        door.handle = { op, p ->
            if (op == "thread.send" && answering) {
                answering = false
                door.emit("thread.changed", obj("thread" to obj("id" to 31, "title" to "Budget", "working" to false)))
                Thread.sleep(300)
                throw FakeDoor.Refuse("conflict", "thread.busy", "The agent is still answering in this thread")
            }
            handled(op, p)
        }
        val hub = Hub(scope, door, source, replica, outbox)
        hub.start()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        withTimeout(10_000) { while (replica.all(Kind.Project).isEmpty()) delay(20) }
        hub.change("thread.send", obj("id" to 31, "text" to "Am I on pace?"), "Message")
        withTimeout(10_000) { while (sent.isEmpty()) delay(20) }
        assertEquals(listOf("Am I on pace?"), sent.toList())
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }
    }

    @Test
    fun `a shell opened while the PC was away is dropped after two minutes, and its start with it`() = runBlocking {
        val clock = AtomicLong(1_000_000)
        val hub = Hub(scope, door, source, replica, outbox, now = clock::get)
        val dropped = async(start = CoroutineStart.UNDISPATCHED) { hub.expired.first() }
        val create = hub.change("session.create", obj("project_id" to 1, "provider" to "shell"), "New shell") as Hub.Change.Queued
        hub.change("session.spawn", obj("session" to Refs.ref(create.entry.id, "name")), "Open shell")
        // A fresh one, opened just now, still goes.
        clock.addAndGet(OutboxRunner.SHELL_OPEN_MS + 1)
        hub.change("session.create", obj("project_id" to 1, "provider" to "shell"), "New shell")
        hub.start()
        assertEquals(create.entry.id, withTimeout(10_000) { dropped.await() }.id)
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }
        assertEquals("the old create and its start never reached the PC", 1, door.requests.count { it.first == "session.create" })
        assertEquals(0, door.requests.count { it.first == "session.spawn" })
    }

    @Test
    fun `an answer the PC could not keep counts as applied, and the row is read again`() = runBlocking {
        val hub = Hub(scope, door, source, replica, outbox)
        hub.start()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        withTimeout(10_000) { while (replica.all(Kind.Project).isEmpty()) delay(20) }
        val change = hub.change("task.update", obj("task_id" to 7, "title" to "Big"), "Rename")
        val entry = (change as? Hub.Change.Queued)?.entry
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }
        entry?.let { assertEquals(OutboxEntry.State.Done, outbox.get(it.id)!!.state) }
        withTimeout(10_000) { while (taskGets.get() == 0) delay(20) }
        withTimeout(10_000) { while (replica.get(Kind.Task, "7")?.json?.get("title")?.toString() != "\"Read again\"") delay(20) }
    }
}
