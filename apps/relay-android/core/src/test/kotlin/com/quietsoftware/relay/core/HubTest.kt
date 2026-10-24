package com.quietsoftware.relay.core

import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.testing.FakeDoor
import com.quietsoftware.relay.core.testing.MemoryOutbox
import com.quietsoftware.relay.core.testing.MemoryReplica
import com.quietsoftware.relay.core.testing.obj
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
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

/** The hub's own rules around the outbox: what a failed first read and a lost answer leave behind. */
class HubTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val replica = MemoryReplica()
    private val outbox = MemoryOutbox()
    private val route = "ws://192.168.1.20:7420"
    private val projectLists = AtomicInteger()
    private val taskGets = AtomicInteger()

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
