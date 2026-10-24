package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.testing.FakeDoor
import com.quietsoftware.relay.core.testing.MemoryOutbox
import com.quietsoftware.relay.core.testing.MemoryReplica
import com.quietsoftware.relay.core.testing.obj
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The phone and a fake PC end to end: edits made with the PC away reach it once it is back,
 * a lost answer is resent under the same id and runs once, and the PC's own changes reach the
 * phone through events.
 */
class ConvergeTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val replica = MemoryReplica()
    private val outbox = MemoryOutbox()
    private val ledger = Ledger(replica, outbox)

    /** The PC's own task table. */
    private val pcTasks = LinkedHashMap<Long, JsonObject>()
    private var nextId = 100L

    private val door = FakeDoor(scope).also { d ->
        d.handle = { op, p -> pc(op, p) }
    }

    private val profile = PcProfile(
        hostId = "h1", name = "desk", instance = "dev", device = door.device, token = door.token,
        routes = listOf(Route("ws://10.9.9.9:7420", Route.Kind.Lan), Route("ws://192.168.1.20:7420", Route.Kind.Lan)),
    )

    private val link = Link(scope, door, object : Link.ProfileSource {
        override suspend fun current() = profile
        override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
    })
    private val syncer = Syncer(scope, ledger, replica)
    private val runner = OutboxRunner(outbox, onApplied = { e, r -> ledger.applied(e, r) })

    @After
    fun tearDown() = scope.cancel()

    private fun task(id: Long, title: String, column: String = "backlog") =
        obj("id" to id, "project_id" to 1, "title" to title, "body" to "", "column" to column, "labels" to emptyList<String>())

    private fun pc(op: String, p: JsonObject): JsonElement = when (op) {
        "bus.ping", "bus.subscribe" -> obj()
        "project.list" -> obj("projects" to listOf(obj("id" to 1, "name" to "Relay", "path" to "/r", "workspace_id" to 1)))
        "task.list" -> obj("tasks" to JsonArray(pcTasks.values.toList()))
        "task.get" -> pcTasks[p.l("task_id")] ?: throw FakeDoor.Refuse("not_found", "task.not_found")
        "task.create" -> task(nextId++, p.s("title")!!).also { pcTasks[it.l("id")!!] = it }
        "task.update" -> {
            val id = p.l("task_id")!!
            val cur = pcTasks[id] ?: throw FakeDoor.Refuse("not_found", "task.not_found")
            val expected = p["expected"] as? JsonObject
            if (expected != null && expected.any { (k, v) -> cur[k] != v }) throw FakeDoor.Refuse("conflict", "task.edit_conflict")
            JsonObject(cur + p.filterKeys { it in setOf("title", "body") }).also { pcTasks[id] = it }
        }
        "task.label.add" -> {
            val id = p.l("task_id")!!
            val cur = pcTasks[id]!!
            val labels = (cur["labels"] as JsonArray) + kotlinx.serialization.json.JsonPrimitive(p.s("label"))
            JsonObject(cur + ("labels" to JsonArray(labels))).also { pcTasks[id] = it }
        }
        "session.create" -> obj("name" to "brisk-otter", "state" to "created", "project_id" to 1)
        "session.spawn" -> throw FakeDoor.Refuse("unavailable", "provider.not_installed", "Codex is not installed on this PC")
        else -> obj()
    }

    private suspend fun online() = withTimeout(10_000) { link.state.first { it is LinkState.Online } }

    private fun connect() {
        link.onSession = { s ->
            syncer.attach(s)
            runner.flush(s)
        }
        link.start()
    }

    @Test
    fun `edits made with the PC away reach it when it is back`() = runBlocking {
        pcTasks[7] = task(7, "Fix the door")
        door.reachable = emptySet()
        ledger.replace(Kind.Task, Scope(projectId = 1), listOf(Row.of(Kind.Task, task(7, "Fix the door"))!!))

        // Offline: rename one task, make another and label it.
        ledger.enqueue("task.update", obj("task_id" to 7, "title" to "Fix the door, from the train", "expected" to obj("title" to "Fix the door")), "Rename")
        val create = ledger.enqueue("task.create", obj("project_id" to 1, "title" to "Written offline"), "New")
        ledger.enqueue("task.label.add", obj("task_id" to Refs.ref(create.id), "label" to "phone"), "Label")

        connect()
        delay(300)
        assertTrue(link.state.value !is LinkState.Online)

        door.reachable = setOf("ws://192.168.1.20:7420")
        link.kick()
        online()
        withTimeout(10_000) { while (outbox.open().isNotEmpty()) delay(20) }

        assertEquals("Fix the door, from the train", pcTasks[7]!!.s("title"))
        val made = pcTasks.values.single { it.s("title") == "Written offline" }
        assertTrue(made.toString().contains("phone"))
        // And the phone shows the PC's rows, not its guesses.
        withTimeout(5_000) { while (replica.get(Kind.Task, made.l("id").toString())?.pending != false) delay(20) }
        assertTrue(replica.all(Kind.Task).none { Optimistic.isTemp(it.id) })
    }

    @Test
    fun `a change made on the PC meanwhile parks the phone's edit instead of overwriting it`() = runBlocking {
        pcTasks[7] = task(7, "Renamed on the PC")
        ledger.upsert(listOf(Row.of(Kind.Task, task(7, "Original"))!!))
        val e = ledger.enqueue("task.update", obj("task_id" to 7, "title" to "Renamed on the phone", "expected" to obj("title" to "Original")), "Rename")
        val other = ledger.enqueue("task.create", obj("project_id" to 1, "title" to "Unrelated"), "New")
        connect()
        online()
        withTimeout(10_000) { while (outbox.get(other.id)!!.state != OutboxEntry.State.Done) delay(20) }
        assertEquals(OutboxEntry.State.Conflict, outbox.get(e.id)!!.state)
        assertEquals("Renamed on the PC", pcTasks[7]!!.s("title"))
        // Unrelated work went through behind it.
        assertTrue(pcTasks.values.any { it.s("title") == "Unrelated" })
    }

    @Test
    fun `a lasting refusal parks its edit, and later edits still reach the PC`() = runBlocking {
        // "New agent" for a provider the PC cannot run, then a task: the spawn's `unavailable` is
        // the PC's answer, not the PC being busy.
        val create = ledger.enqueue("session.create", obj("project_id" to 1, "provider" to "codex"), "New agent")
        val spawn = ledger.enqueue("session.spawn", obj("session" to Refs.ref(create.id, "name")), "Start agent")
        val task = ledger.enqueue("task.create", obj("project_id" to 1, "title" to "Queued after"), "New task")
        connect()
        online()
        withTimeout(10_000) { while (outbox.get(task.id)!!.state != OutboxEntry.State.Done) delay(20) }
        assertEquals(OutboxEntry.State.Failed, outbox.get(spawn.id)!!.state)
        assertEquals("provider.not_installed", outbox.get(spawn.id)!!.error!!.code)
        assertTrue(pcTasks.values.any { it.s("title") == "Queued after" })
    }

    @Test
    fun `the PC's changes arrive as events`() = runBlocking {
        connect()
        online()
        withTimeout(5_000) { while (replica.all(Kind.Project).isEmpty()) delay(20) }
        door.emit("task.changed", task(55, "Made on the PC", column = "ready"))
        withTimeout(5_000) { while (replica.get(Kind.Task, "55") == null) delay(20) }
        pcTasks[56] = task(56, "Hinted")
        door.emit("task.changed", obj("task_id" to 56, "state" to "running"))
        withTimeout(5_000) { while (replica.get(Kind.Task, "56") == null) delay(20) }
        assertEquals("Hinted", replica.get(Kind.Task, "56")!!.json.s("title"))
    }

    @Test
    fun `a resend after a lost answer runs once`() = runBlocking {
        pcTasks[7] = task(7, "Original")
        connect()
        online()
        val s = link.require()
        val id = java.util.UUID.randomUUID().toString()
        val first = s.call("task.create", obj("project_id" to 1, "title" to "Once"), id = id)
        val again = s.call("task.create", obj("project_id" to 1, "title" to "Once"), id = id)
        assertTrue(first.ok && again.ok && again.replayed)
        assertEquals(1, pcTasks.values.count { it.s("title") == "Once" })
    }

    @Test
    fun `an edit a guardrail held is done once someone allows it`() = runBlocking {
        val hub = com.quietsoftware.relay.core.Hub(scope, door, object : Link.ProfileSource {
            override suspend fun current() = profile
            override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
        }, replica, outbox)
        door.handle = { op, p ->
            if (op == "task.delete") throw FakeDoor.Refuse("held", "guardrail.destructive_write", confirm = obj("op" to "guardrail.confirm", "payload" to obj("hold_id" to 9)))
            pc(op, p)
        }
        hub.start()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        val e = (hub.change("task.delete", obj("task_id" to 7), "Delete #7") as com.quietsoftware.relay.core.Hub.Change.Queued).entry
        withTimeout(10_000) { while (outbox.get(e.id)?.state != OutboxEntry.State.Held) delay(20) }
        door.emit("guardrail.resolved", obj("hold_id" to 9, "state" to "confirmed", "by" to "user"))
        withTimeout(10_000) { while (outbox.get(e.id)?.state != OutboxEntry.State.Done) delay(20) }
    }

    @Test
    fun `a revoked phone stops dialing`() = runBlocking {
        val wrong = Link(scope, door, object : Link.ProfileSource {
            override suspend fun current() = profile.copy(token = "stale")
            override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
        })
        wrong.start()
        withTimeout(10_000) { wrong.state.first { it is LinkState.Revoked } }
        val dials = door.dials
        delay(1_500)
        assertEquals(dials, door.dials)
    }
}
