package com.quietsoftware.relay.ui.pages

import android.content.Context
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
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
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Create task from note, tapped twice in a row, makes one task: the second tap is the first again. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class NoteToTaskTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun aDoubleTapMakesOneTask() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        runBlocking {
            relay.hub.ledger.upsert(listOf(Row.of(Kind.Project, Wire.parse("""{"id":1,"workspace_id":1,"path":"/w/RelayV4","name":"RelayV4","base_branch":"main","order":0}""")!!)!!))
            relay.hub.ledger.upsert(listOf(Row.of(Kind.Note, Wire.parse("""{"id":201,"project_id":1,"title":"Phone app plan","body":"Ship the outbox.","pinned":false,"created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T21:00:00Z"}""")!!)!!))
        }
        val shell = ShellModel(relay, Settings(context))
        compose.setContent {
            val controller = rememberNavController()
            val nav = remember { Nav(controller, shell) }
            RelayTheme(Palette.Matte) {
                CompositionLocalProvider(LocalNav provides nav) { AppNav(nav, "note?id=201") }
            }
        }
        compose.waitUntil(10_000) { compose.onAllNodesWithText("Create task from note").fetchSemanticsNodes().isNotEmpty() }
        val key = compose.onNodeWithText("Create task from note").performScrollTo()
        key.performClick()
        key.performClick()
        compose.waitForIdle()
        compose.waitUntil(10_000) { runBlocking { relay.hub.outbox.open() }.any { it.op == "task.create" } }
        Thread.sleep(300)
        compose.waitForIdle()
        assertEquals(1, runBlocking { relay.hub.outbox.open() }.count { it.op == "task.create" })
    }
}
