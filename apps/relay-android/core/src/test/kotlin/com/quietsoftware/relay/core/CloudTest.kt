package com.quietsoftware.relay.core

import com.quietsoftware.relay.core.link.Dialer
import com.quietsoftware.relay.core.link.Handshake
import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.link.Refusal
import com.quietsoftware.relay.core.link.RefusalException
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.testing.FakeDoor
import com.quietsoftware.relay.core.testing.MemoryOutbox
import com.quietsoftware.relay.core.testing.MemoryReplica
import com.quietsoftware.relay.core.testing.obj
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.util.concurrent.CopyOnWriteArrayList

/**
 * The server's copy (docs/CLOUD.md) standing in for the PC: it answers the ledger's and the
 * training history's reads while the PC is away, is never sent anything else, and steps back
 * when the PC is in reach again.
 */
class CloudTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val replica = MemoryReplica()
    private val outbox = MemoryOutbox()

    private val pcRoute = "ws://192.168.1.20:7420"
    private val cloudRoute = "wss://server.example/relay/join/r00m"

    private var pcSpent = 9_000L
    private val pc = FakeDoor(scope, ops = Wire.OPS_PC).also { d ->
        d.reachable = emptySet()
        d.handle = { op, _ ->
            when (op) {
                "money.summary" -> obj("spent" to pcSpent)
                "task.create" -> obj("id" to 100, "project_id" to 1, "title" to "On the train", "column" to "backlog")
                else -> obj()
            }
        }
    }
    private val server = FakeDoor(scope, device = "cloud1", token = "cl0ud", ops = Wire.OPS_CLOUD).also { d ->
        d.reachable = setOf(cloudRoute)
        d.handle = { op, _ -> answerAsCloud(op) }
    }

    /** What a cloud engine's door does: the data, and `remote.op` for everything else. */
    private fun answerAsCloud(op: String): JsonElement = when (op) {
        "bus.ping", "bus.subscribe" -> obj()
        "money.summary" -> obj("spent" to 4_250)
        "gym.summary" -> obj("imported" to obj("exported_at" to "2026-10-08 21:14"))
        else -> throw FakeDoor.Refuse("refused", "remote.op")
    }

    private val dialer = Dialer { url, timeoutMs -> if (url == cloudRoute) server.dial(url, timeoutMs) else pc.dial(url, timeoutMs) }

    private fun source(profile: PcProfile) = object : Link.ProfileSource {
        override suspend fun current() = profile
        override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
    }

    private val pcProfile = PcProfile("h1", "desk", "dev", pc.device, pc.token, listOf(Route(pcRoute, Route.Kind.Lan)))
    private val cloudProfile = PcProfile("c1", "server", "stable", server.device, server.token, listOf(Route(cloudRoute, Route.Kind.Rendezvous)))

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `with the PC away the server's copy answers its reads and is sent nothing else`() = runBlocking {
        val hub = Hub(scope, dialer, source(pcProfile), replica, outbox, cloudProfiles = source(cloudProfile), cloudGraceMs = 0)
        val stale = CopyOnWriteArrayList<Set<String>>()
        scope.launch { hub.stale.collect { stale += it } }
        hub.start()
        withTimeout(10_000) { hub.cloud.state.first { it is LinkState.Online } }
        assertFalse(hub.state.value is LinkState.Online)

        // An edit waits for the PC; it is not the server's to take.
        val edit = (hub.change("task.create", obj("project_id" to 1, "title" to "On the train"), "New task") as Hub.Change.Queued).entry

        // The ledger's overview comes from the server's copy, cached as its, not as the PC's.
        val money = hub.query("money.summary")
        assertEquals(4_250L, (money.result as JsonObject).l("spent"))
        assertTrue(money.cloud && !money.fresh)
        assertTrue(hub.cached("money.summary")!!.cloud)

        // A read the copy does not hold is never asked of it, and an act needs the PC.
        try {
            hub.query("project.list")
            fail("project.list answered with the PC away")
        } catch (e: BusException) {
            assertEquals("link.down", e.error.code)
        }
        try {
            hub.call("session.input", obj("session" to "brisk-otter", "data" to "y"))
            fail("an act went somewhere with the PC away")
        } catch (e: BusException) {
            assertEquals("link.down", e.error.code)
        }

        // Tally syncs on the server: the ledger's screens read again.
        withTimeout(5_000) { while (server.requests.none { it.first == "bus.subscribe" }) delay(20) }
        withTimeout(5_000) {
            while (setOf("money") !in stale) {
                server.emit("money.changed", obj("device" to "Pixel"))
                delay(100)
            }
        }
        val subscribed = server.requests.first { it.first == "bus.subscribe" }.second.arr("events").map { it.toString().trim('"') }
        assertEquals(listOf("money.*", "gym.*"), subscribed)

        // The background catch-up reads the overviews again from the copy.
        assertTrue(hub.catchUpFromCloud(waitMs = 5_000) >= 2)
        assertTrue(hub.cached("gym.summary")!!.cloud)

        // The PC is back: the copy steps back, the edit goes to the PC, and the PC answers again.
        pc.reachable = setOf(pcRoute)
        hub.link.kick()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        withTimeout(10_000) { hub.cloud.state.first { it is LinkState.Stopped } }
        withTimeout(10_000) { while (outbox.get(edit.id)?.state != OutboxEntry.State.Done) delay(20) }
        val back = hub.query("money.summary")
        assertEquals(9_000L, (back.result as JsonObject).l("spent"))
        assertTrue(back.fresh && !back.cloud)
        assertFalse(hub.cached("money.summary")!!.cloud)

        // Over all of it, the copy was asked its reads and nothing else: no resync, no outbox.
        val asked = server.requests.map { it.first }.toSet()
        assertEquals(setOf("bus.ping", "bus.subscribe", "money.summary", "gym.summary"), asked)
        assertTrue(pc.requests.any { it.first == "project.list" } && pc.requests.any { it.first == "task.create" })
    }

    @Test
    fun `with the PC in reach the server's copy is never dialled, a kick included`() = runBlocking {
        pc.reachable = setOf(pcRoute)
        val hub = Hub(scope, dialer, source(pcProfile), replica, outbox, cloudProfiles = source(cloudProfile), cloudGraceMs = 1_000)
        hub.start()
        // The app coming to the front kicks both links before the PC has answered.
        hub.link.kick()
        hub.cloud.kick()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        delay(1_500)
        assertEquals(0, server.dials)
        assertEquals(LinkState.Stopped, hub.cloud.state.value)
    }

    @Test
    fun `a PC disconnected here is not stood in for`() = runBlocking {
        pc.reachable = setOf(pcRoute)
        val hub = Hub(scope, dialer, source(pcProfile), replica, outbox, cloudProfiles = source(cloudProfile), cloudGraceMs = 0)
        hub.start()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        withTimeout(10_000) { hub.cloud.state.first { it is LinkState.Stopped } }
        val dialled = server.dials
        // Disconnect.
        hub.link.stop()
        delay(500)
        assertEquals(dialled, server.dials)
        assertEquals(LinkState.Stopped, hub.cloud.state.value)
        assertEquals(0, hub.catchUpFromCloud(waitMs = 500))
        assertEquals(dialled, server.dials)
    }

    @Test
    fun `a copy that refused this phone is not asked again as the PC comes and goes`() = runBlocking {
        val hub = Hub(scope, dialer, source(pcProfile), replica, outbox, cloudProfiles = source(cloudProfile.copy(token = "spent")), cloudGraceMs = 0)
        hub.start()
        withTimeout(10_000) { hub.cloud.state.first { it is LinkState.Revoked } }
        val dialled = server.dials

        pc.reachable = setOf(pcRoute)
        hub.link.kick()
        withTimeout(10_000) { hub.state.first { it is LinkState.Online } }
        assertTrue(hub.cloud.state.value is LinkState.Revoked)

        pc.reachable = emptySet()
        pc.dropAll()
        withTimeout(10_000) { hub.state.first { it !is LinkState.Online } }
        delay(500)
        assertEquals(dialled, server.dials)
        assertTrue(hub.cloud.state.value is LinkState.Revoked)
        assertEquals(0, hub.catchUpFromCloud(waitMs = 500))
    }

    @Test
    fun `the PC's link never takes the server's copy, nor the copy's link a PC`() = runBlocking {
        // The PC's credential, presented at the server's copy: let in, and refused by the phone.
        server.reachable = setOf(pcRoute, cloudRoute)
        val wrongForPc = Link(scope, Dialer { url, t -> server.dial(url, t) }, source(pcProfile.copy(device = server.device, token = server.token)))
        wrongForPc.start()
        val refused = withTimeout(10_000) { wrongForPc.state.first { it is LinkState.Revoked } } as LinkState.Revoked
        assertEquals(Refusal.WrongDoor(Wire.OPS_PC).text, refused.reason)

        pc.reachable = setOf(cloudRoute)
        val wrongForCloud = Link(scope, Dialer { url, t -> pc.dial(url, t) }, source(cloudProfile.copy(device = pc.device, token = pc.token)), ops = Wire.OPS_CLOUD)
        wrongForCloud.start()
        withTimeout(10_000) { wrongForCloud.state.first { it is LinkState.Revoked } }

        // A door too old to say which it is is taken at its word.
        pc.ops = null
        val older = Link(scope, Dialer { url, t -> pc.dial(url, t) }, source(cloudProfile.copy(device = pc.device, token = pc.token)), ops = Wire.OPS_CLOUD)
        older.start()
        withTimeout(10_000) { older.state.first { it is LinkState.Online } }

        // Pairing says so too, before anything is saved.
        val link = PairLink("ABCD-EFGH", "server", "c1", "stable", direct = emptyList(), via = cloudRoute)
        val asPc = Handshake.pair(dialer, link, "Pixel")
        assertTrue((asPc.exceptionOrNull() as? RefusalException)?.refusal is Refusal.WrongDoor)
        val asCloud = Handshake.pair(dialer, link, "Pixel", ops = Wire.OPS_CLOUD).getOrThrow()
        assertEquals(server.token, asCloud.token)
    }

    @Test
    fun `only the copy's own domains go stale from its events`() {
        assertEquals(setOf("money"), Hub.staleOnCloud("money.changed"))
        assertEquals(setOf("gym"), Hub.staleOnCloud("gym.changed"))
        assertEquals(setOf("money", "gym"), Hub.staleOnCloud("bus.lagged"))
        assertTrue(Hub.staleOnCloud("task.changed").isEmpty())
        // Every read the copy answers is one of its domains'.
        assertTrue(Hub.CLOUD_READS.all { it.substringBefore('.') in setOf("money", "gym") })
    }
}
