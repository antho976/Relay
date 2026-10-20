package com.quietsoftware.relay.ui.threads

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.navigation.compose.rememberNavController
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.wire.Wire
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
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.ByteArrayOutputStream

/** A picture open full screen stays open while a reply scrolls its message away. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class PictureViewerTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun theViewerOutlivesItsBubble() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val settings = Settings(context)
        // The picture is on this phone already, as one sent from here is.
        val jpeg = ByteArrayOutputStream().also { Bitmap.createBitmap(40, 30, Bitmap.Config.ARGB_8888).compress(Bitmap.CompressFormat.JPEG, 90, it) }.toByteArray()
        ThreadPictures.keep(context, JsonPrimitive("pic1"), jpeg)
        runBlocking {
            val ledger = relay.hub.ledger
            ledger.upsert(listOf(Row.of(Kind.Thread, Wire.parse("""{"id":301,"title":"Receipts","provider":"claude","created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T21:00:00Z","working":false,"live":false,"preview":"Picture"}""")!!)!!))
            val picture = """{"id":401,"thread_id":301,"role":"user","body":{"text":"","images":[{"id":"pic1","media_type":"image/jpeg","width":40,"height":30}]},"created_at":"2026-10-09T21:00:00Z"}"""
            ledger.upsert(listOf(Row.of(Kind.Message, Wire.parse(picture)!!)!!))
        }
        val shell = ShellModel(relay, settings)
        compose.setContent {
            val controller = rememberNavController()
            val nav = remember { Nav(controller, shell) }
            RelayTheme(Palette.Matte) {
                CompositionLocalProvider(LocalNav provides nav) { AppNav(nav, "thread/301") }
            }
        }
        compose.waitUntil(10_000) { compose.onAllNodesWithContentDescription("Picture you sent").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithContentDescription("Picture you sent").performClick()
        compose.waitUntil(10_000) { compose.onAllNodesWithContentDescription("Close the picture").fetchSemanticsNodes().isNotEmpty() }

        // A long answer arrives: the list follows it, and the picture's message leaves the composition.
        runBlocking {
            relay.hub.ledger.upsert((0 until 40).map { i ->
                Row.of(Kind.Message, Wire.parse("""{"id":${500 + i},"thread_id":301,"role":"assistant","body":{"blocks":[{"type":"text","text":"Line $i of the answer, long enough to take a row of its own on the screen."}]},"created_at":"2026-10-09T21:01:${"%02d".format(i)}Z"}""")!!)!!
            })
        }
        compose.waitUntil(10_000) { compose.onAllNodesWithText("Line 39 of the answer", substring = true).fetchSemanticsNodes().isNotEmpty() }
        compose.waitForIdle()
        // Only the viewer's: the bubble has left.
        compose.onAllNodesWithContentDescription("Picture you sent").assertCountEquals(1)
        compose.onAllNodesWithContentDescription("Close the picture").assertCountEquals(1)
    }
}
