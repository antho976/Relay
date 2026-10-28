package com.quietsoftware.relay.data

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.link.Handshake
import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.model.decode
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.db.RoomOutbox
import com.quietsoftware.relay.data.db.RoomReplica
import com.quietsoftware.relay.data.link.OkHttpDialer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * The app's link and sync against a real PC door: pair with the link `relay remote pair
 * --no-confirm` printed, let the door start the engine (`relay remote serve --start-engine`), then
 * make a project, edit with the PC in reach and with the link down, and see both sides agree.
 * Runs only when `RELAY_E2E_LINK` names a pairing link, and `RELAY_E2E_REPO` a scratch directory.
 */
@RunWith(RobolectricTestRunner::class)
class LiveDoorTest {
    @Test
    fun `the phone and a real PC converge`() = runBlocking {
        val linkText = System.getenv("RELAY_E2E_LINK")
        val scratch = System.getenv("RELAY_E2E_REPO")
        assumeTrue("set RELAY_E2E_LINK and RELAY_E2E_REPO to run against a real door", linkText != null && scratch != null)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val dialer = OkHttpDialer()
        val link = PairLink.parse(linkText!!) ?: error("not a pairing link: $linkText")

        val profile = Handshake.pair(dialer, link, "LiveDoorTest").getOrThrow()
        println("paired as ${profile.device} with ${profile.name} (${profile.instance}) via ${profile.lastRoute}")

        val db = Room.inMemoryDatabaseBuilder(ApplicationProvider.getApplicationContext<Context>(), RelayDb::class.java).allowMainThreadQueries().build()
        val replica = RoomReplica(db)
        val outbox = RoomOutbox(db)
        val seen = mutableListOf<LinkState>()
        val hub = Hub(scope, dialer, object : Link.ProfileSource {
            override suspend fun current() = profile
            override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) {
                println("greeting: engine=${greeting.engine} wake=${greeting.wake}")
            }
        }, replica, outbox)
        scope.launchStates(hub, seen)
        hub.start()
        withTimeout(60_000) { hub.state.first { it is LinkState.Online } }
        println("online; states seen: ${seen.map { it::class.simpleName }.distinct()}")

        // A repository for a project, made the way the person's would be.
        val repo = File(scratch, "demo").apply { mkdirs() }
        fun git(vararg args: String) = ProcessBuilder(listOf("git", "-C", repo.path) + args).redirectErrorStream(true).start().waitFor()
        if (!File(repo, ".git").exists()) {
            git("init", "-q", "-b", "main")
            File(repo, "README.md").writeText("demo\n")
            git("add", ".")
            git("-c", "user.email=e2e@relay", "-c", "user.name=e2e", "commit", "-q", "-m", "first")
        }
        val workspace = hub.call("workspace.create", buildJsonObject { put("path", scratch); put("name", "e2e") }) as JsonObject
        val project = hub.call("project.add", buildJsonObject { put("workspace_id", workspace.l("id")!!); put("path", repo.path) }) as JsonObject
        val projectId = project.l("id")!!
        withTimeout(10_000) { while (replica.all(Kind.Project).none { it.decode<Project>()?.num == projectId }) delay(50) }
        println("project $projectId reached the phone through events")

        // With the PC in reach: a task made on the phone is on the PC, under the PC's number.
        val made = hub.change("task.create", buildJsonObject { put("project_id", projectId); put("title", "From the phone") }, "New task")
        val entry = (made as Hub.Change.Queued).entry
        withTimeout(15_000) { while (outbox.get(entry.id)?.state != OutboxEntry.State.Done) delay(50) }
        val taskId = outbox.get(entry.id)!!.result!!.let { (it as JsonObject).l("id")!! }
        assertEquals("From the phone", (hub.call("task.get", buildJsonObject { put("task_id", taskId) }) as JsonObject).s("title"))

        // With the link down: the edit waits, shows at once, and reaches the PC when it is back.
        hub.link.stop()
        hub.change("task.update", buildJsonObject {
            put("task_id", taskId)
            put("title", "Edited on the bus")
            put("expected", buildJsonObject { put("title", "From the phone") })
        }, "Rename")
        assertEquals("Edited on the bus", replica.get(Kind.Task, taskId.toString())!!.decode<Task>()!!.title)
        assertEquals(1, outbox.open().size)
        hub.start()
        withTimeout(60_000) { while (outbox.open().isNotEmpty()) delay(100) }
        assertEquals("Edited on the bus", (hub.call("task.get", buildJsonObject { put("task_id", taskId) }) as JsonObject).s("title"))

        // A change made on the PC reaches the phone as an event.
        hub.call("task.move", buildJsonObject { put("task_id", taskId); put("column", "ready") })
        withTimeout(10_000) { while (replica.get(Kind.Task, taskId.toString())?.decode<Task>()?.column != "ready") delay(50) }

        // The ops this app added to the door answer.
        val threads = hub.call("thread.list") as JsonObject
        println("threads on the PC: ${threads.arr("threads").size}")
        // Aide's chats: the Chief of Staff's opens through the door, and its kind lists.
        val chief = (hub.call("aide.chat") as JsonObject).l("thread_id")
        assertTrue(chief != null)
        val aide = hub.call("thread.list", buildJsonObject { put("kind", "aide") }) as JsonObject
        assertTrue(aide.arr("threads").any { (it as JsonObject).l("id") == chief })
        val money = hub.query("money.summary")
        assertTrue(money.fresh)
        println("money.summary currency: ${(money.result as JsonObject).s("currency")}, empty: ${(money.result as JsonObject)["empty"]}")
        println("dashboard sessions_live: ${(hub.call("dashboard.get") as JsonObject).arr("sessions_live").size}")
        println("usage windows: ${(hub.call("usage.get") as JsonObject).o("usage")?.keys}")
        scope.cancel()
    }

    private fun CoroutineScope.launchStates(hub: Hub, into: MutableList<LinkState>) = launch {
        hub.state.collect { into += it }
    }
}
