package com.quietsoftware.relay.ui.threads

import android.content.Context
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.navigation.compose.rememberNavController
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.ui.AppNav
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.shell.ShellModel
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.RelayTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** A message the PC refused offers Send again, with its pictures, in its own place. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class RefusedSendTest {
    @get:Rule val compose = createComposeRule()

    /** Until [text] shows, the screen's clock moved on as the phone's would (the shell's state follows the outbox on it). */
    private fun waitFor(text: String) {
        val until = System.currentTimeMillis() + 10_000
        while (compose.onAllNodesWithText(text, substring = true).fetchSemanticsNodes().isEmpty()) {
            check(System.currentTimeMillis() < until) { "“$text” never showed" }
            compose.mainClock.advanceTimeBy(250)
            compose.waitForIdle()
            Thread.sleep(20)
        }
    }

    @Test
    fun sendAgainReplacesTheRefusedMessage() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val refused = runBlocking {
            relay.hub.ledger.upsert(listOf(Row.of(Kind.Thread, Wire.parse("""{"id":301,"title":"Receipts","provider":"claude","model":"opus","created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T21:00:00Z","working":false,"live":false,"preview":"Lunch"}""")!!)!!))
            // Sent with a picture while the thread ran on one of the PC's own models, which the PC refused.
            val payload = buildJsonObject { put("id", 301); put("text", "Lunch"); put("images", JsonArray(listOf(JsonPrimitive("pic1")))) }
            val entry = (relay.change("thread.send", payload, "Message") as Hub.Change.Queued).entry
            relay.hub.outbox.update(entry.copy(state = OutboxEntry.State.Failed, error = BusError("invalid", "thread.images_unsupported", "This model cannot read pictures")))
            entry.id
        }
        val shell = ShellModel(relay, Settings(context))
        compose.setContent {
            val controller = rememberNavController()
            val nav = remember { Nav(controller, shell) }
            RelayTheme(Palette.Matte) {
                CompositionLocalProvider(LocalNav provides nav) { AppNav(nav, "thread/301") }
            }
        }
        waitFor("Not sent")
        // The thread is on a Claude model now: the message opens to Send again.
        compose.onNodeWithText("Lunch").performClick()
        waitFor("Send again")
        compose.onNodeWithText("Send again").performClick()

        val until = System.currentTimeMillis() + 10_000
        while (runBlocking { relay.hub.outbox.get(refused) } != null) {
            check(System.currentTimeMillis() < until) { "the refused message stayed: " + runBlocking { relay.hub.outbox.open() }.map { "${it.state} ${it.payload}" } }
            compose.mainClock.advanceTimeBy(250)
            compose.waitForIdle()
            Thread.sleep(20)
        }
        val sends = runBlocking { relay.hub.outbox.open() }.filter { it.op == "thread.send" }
        assertEquals(1, sends.size)
        assertEquals(OutboxEntry.State.Pending, sends.single().state)
        assertEquals(listOf("pic1"), sends.single().payload.arr("images").map { (it as JsonPrimitive).content })
    }
}
