package com.quietsoftware.relay.ui

import android.content.Context
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.navigation.compose.rememberNavController
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.ui.shell.HoldTray
import com.quietsoftware.relay.ui.shell.ShellModel
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.RelayTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Every main screen rendered over a seeded copy of the PC's data, with no PC: what the person
 * sees on the bus with the PC asleep. Captured to `src/test/screenshots/<name>.png`.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class ScreensTest {
    @get:Rule val compose = createComposeRule()

    private lateinit var relay: Relay
    private lateinit var shell: ShellModel

    @Before
    fun seed() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        relay = Relay(context, db, PcStore(context), scope)
        val settings = Settings(context)
        runBlocking {
            settings.update { it.copy(name = "Antho", lastProject = 1) }
            val ledger = relay.hub.ledger
            fun rows(kind: Kind, vararg json: String) = json.map { Row.of(kind, Wire.parse(it)!!)!! }
            ledger.upsert(rows(Kind.Workspace, """{"id":1,"path":"/home/antho/dev","name":"dev","order":0}"""))
            ledger.upsert(rows(Kind.Project,
                """{"id":1,"workspace_id":1,"path":"/home/antho/dev/RelayV4","name":"RelayV4","base_branch":"main","order":0,"pinned":true}""",
                """{"id":2,"workspace_id":1,"path":"/home/antho/dev/Tally","name":"Tally","base_branch":"main","order":1,"pinned":false}"""))
            ledger.upsert(rows(Kind.Session,
                """{"id":11,"name":"brisk-otter","project_id":1,"provider":"claude","role":"builder","branch":"relay/brisk-otter","worktree":"/w/brisk-otter","state":"running","intent":"Porting the board to the phone","created_at":"2026-10-09T20:00:00Z","updated_at":"2026-10-09T21:00:00Z"}""",
                """{"id":12,"name":"calm-heron","project_id":1,"provider":"codex","role":"reviewer","branch":"relay/brisk-otter","worktree":"/w/brisk-otter","state":"blocked","intent":"Reviewing the outbox","created_at":"2026-10-09T20:00:00Z","updated_at":"2026-10-09T21:00:00Z"}""",
                """{"id":13,"name":"quiet-lynx","project_id":1,"provider":"claude","role":"docs","branch":"relay/quiet-lynx","worktree":"/w/quiet-lynx","state":"parked","created_at":"2026-10-09T20:00:00Z","updated_at":"2026-10-09T21:00:00Z"}"""))
            ledger.upsert(rows(Kind.Task,
                """{"id":101,"project_id":1,"title":"Offline outbox for the phone","body":"Edits made with the PC away wait and go once.\n\n- ordered\n- idempotent","column":"active","state":"running","priority":"high","type":"feature","size":"M","labels":["phone","sync"],"sessions":["brisk-otter"],"rollup":{"total":3,"done":1},"children":[102],"updated_at":"2026-10-09T21:00:00Z","created_at":"2026-10-08T10:00:00Z"}""",
                """{"id":102,"project_id":1,"title":"Conflict card when the PC changed a field too","column":"ready","priority":"medium","type":"task","parent_id":101,"labels":["sync"],"updated_at":"2026-10-09T20:00:00Z","created_at":"2026-10-08T10:00:00Z"}""",
                """{"id":103,"project_id":1,"title":"Terminal renders Claude Code's spinner line","column":"in_review","state":"awaiting_review","priority":"urgent","type":"bug","labels":["terminal"],"updated_at":"2026-10-09T19:00:00Z","created_at":"2026-10-08T10:00:00Z"}""",
                """{"id":104,"project_id":1,"title":"Wake-on-LAN from the PC card","column":"done","priority":"low","type":"chore","updated_at":"2026-10-09T18:00:00Z","created_at":"2026-10-08T10:00:00Z"}""",
                """{"id":105,"project_id":1,"title":"Share sheet: text to a task","column":"backlog","priority":"medium","type":"feature","blocked_by":[101],"updated_at":"2026-10-09T17:00:00Z","created_at":"2026-10-08T10:00:00Z"}"""))
            ledger.upsert(rows(Kind.Note,
                """{"id":201,"project_id":1,"title":"Phone app plan","body":"# Phone app\n\nThe phone keeps a copy of the PC's data.\n\n```kotlin\nhub.change(op, payload, label)\n```\n\n| route | private |\n|---|---|\n| WiFi | yes |\n| Tailscale | yes |","pinned":true,"created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T21:00:00Z"}""",
                """{"id":202,"project_id":1,"title":null,"body":"Ask about the door service at login","pinned":false,"created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T12:00:00Z"}"""))
            ledger.upsert(rows(Kind.Hold,
                """{"id":7,"project_id":1,"session":"calm-heron","actor":"agent:calm-heron","op":"git.push","payload_hash":"x","policy":"destructive_write","state":"open","created_at":"2026-10-09T21:00:00Z"}"""))
            ledger.upsert(rows(Kind.Notification,
                """{"id":31,"project_id":1,"category":"agent_blocked","title":"calm-heron is blocked","body":"It asks whether to push to main.","read":false,"created_at":"2026-10-09T21:01:00Z"}""",
                """{"id":30,"project_id":1,"category":"agent_done","title":"brisk-otter reports done","body":"The board renders on the phone.","read":true,"created_at":"2026-10-09T20:01:00Z"}"""))
            ledger.upsert(rows(Kind.Thread,
                """{"id":301,"title":"Groceries this month","provider":"claude","created_at":"2026-10-09T10:00:00Z","updated_at":"2026-10-09T21:00:00Z","working":false,"live":false,"preview":"How much did I spend on groceries?"}"""))
            ledger.upsert(rows(Kind.Message,
                """{"id":401,"thread_id":301,"role":"user","body":{"text":"How much did I spend on groceries this month?"},"created_at":"2026-10-09T21:00:00Z"}""",
                """{"id":402,"thread_id":301,"role":"assistant","body":{"blocks":[{"type":"tool_use","id":"t1","name":"mcp__relay__money_summary","input":{}},{"type":"text","text":"You spent **$412.30** on groceries, about 9% under pace.\n\n| Week | Spent |\n|---|---:|\n| 1 | $98 |\n| 2 | $121 |"}]},"created_at":"2026-10-09T21:00:05Z"}"""))
        }
        runBlocking {
            // What the phone saved of brisk-otter's terminal: a Claude Code turn, with colour.
            val e = "\u001b"
            val text = listOf(
                "$e[38;2;215;119;87m✻$e[0m Welcome to $e[1mClaude Code$e[0m",
                "",
                "$e[2m> $e[0mPort the board to the phone, offline first",
                "",
                "$e[38;5;245m⏺$e[0m I'll start with the replica and the outbox.",
                "",
                "$e[32m⏺$e[0m $e[1mRead$e[0m(core/sync/Ledger.kt)",
                "  ⎿  Read 182 lines",
                "",
                "$e[32m⏺$e[0m $e[1mUpdate$e[0m(core/sync/Optimistic.kt)",
                "  ⎿  Updated with $e[32m14 additions$e[0m and $e[31m2 removals$e[0m",
                "      $e[48;5;22m+ \"task.relate\", \"task.unrelate\" -> row?.let { r ->$e[0m",
                "      $e[48;5;52m- \"task.relate\", \"task.unrelate\", \"task.link_commit\" -> row$e[0m",
                "",
                "$e[38;5;245m⏺$e[0m Tests pass: $e[32m162 passed$e[0m, 1 skipped.",
                "",
                "$e[38;5;215m✶ Thinking… $e[2m(esc to interrupt)$e[0m",
                "╭──────────────────────────────────────╮",
                "│ > $e[7m $e[0m                                  │",
                "╰──────────────────────────────────────╯",
            ).joinToString("\n")
            relay.hub.store.putQuery(
                com.quietsoftware.relay.core.sync.QueryKey.of("session.scrollback", kotlinx.serialization.json.buildJsonObject {
                    put("session", kotlinx.serialization.json.JsonPrimitive("brisk-otter")); put("lines", kotlinx.serialization.json.JsonPrimitive(400))
                }),
                kotlinx.serialization.json.buildJsonObject { put("text", kotlinx.serialization.json.JsonPrimitive(text)) }.toString(),
                System.currentTimeMillis(),
            )
            // What the phone saved of brisk-otter's conversation: a turn with reasoning, steps, an edit and a reply.
            val transcript = """{"found":true,"cursor":9,"reset":false,"messages":[
                {"id":"1","role":"user","body":{"text":"The board drops a card dragged onto the Done column. Fix it?"},"created_at":"2026-10-09T20:58:00Z"},
                {"id":"2","role":"assistant","body":{"blocks":[{"type":"thinking","text":"The drop handler reads the column from the card, not the target."},{"type":"tool_use","id":"t1","name":"Grep","summary":"onDrop"}]},"created_at":"2026-10-09T20:58:04Z"},
                {"id":"3","role":"tool","body":{"tool_use_id":"t1","is_error":false,"text":"ui/tasks/Board.kt:212"},"created_at":"2026-10-09T20:58:05Z"},
                {"id":"4","role":"assistant","body":{"blocks":[{"type":"tool_use","id":"t2","name":"Read","summary":"ui/tasks/Board.kt"},{"type":"tool_use","id":"t3","name":"Bash","summary":"./gradlew :app:testDebugUnitTest --tests '*Board*'"}]},"created_at":"2026-10-09T20:58:09Z"},
                {"id":"5","role":"tool","body":{"tool_use_id":"t2","is_error":false,"text":"331 lines"},"created_at":"2026-10-09T20:58:10Z"},
                {"id":"6","role":"tool","body":{"tool_use_id":"t3","is_error":true,"text":"BoardTest > dropOnDone FAILED"},"created_at":"2026-10-09T20:58:40Z"},
                {"id":"7","role":"assistant","body":{"blocks":[{"type":"text","text":"Found it: `onDrop` moved the card to the column it **came from**. The fix reads the target column instead."},{"type":"tool_use","id":"t4","name":"Edit","summary":"ui/tasks/Board.kt","detail":"@@ -212 +212 @@\n-    move(card, card.column)\n+    move(card, target.column)"}]},"created_at":"2026-10-09T20:59:00Z"},
                {"id":"8","role":"tool","body":{"tool_use_id":"t4","is_error":false,"text":"ok"},"created_at":"2026-10-09T20:59:01Z"},
                {"id":"8b","role":"assistant","body":{"blocks":[{"type":"tool_use","id":"t5","name":"TodoWrite","summary":"2 of 4 done","detail":"[x] Find the drop handler\n[x] Move the card to the target column\n[~] Run the board tests\n[ ] Update the changelog"}]},"created_at":"2026-10-09T20:59:05Z"},
                {"id":"8c","role":"tool","body":{"tool_use_id":"t5","is_error":false,"text":"ok"},"created_at":"2026-10-09T20:59:06Z"},
                {"id":"9","role":"assistant","body":{"blocks":[{"type":"text","text":"Fixed. Dropping on **Done** now lands there:\n\n```kotlin\nmove(card, target.column)\n```\n\nThe board tests pass."}]},"created_at":"2026-10-09T20:59:30Z"}
            ]}"""
            relay.hub.store.putQuery(
                com.quietsoftware.relay.core.sync.QueryKey.of("session.transcript", kotlinx.serialization.json.buildJsonObject {
                    put("session", kotlinx.serialization.json.JsonPrimitive("brisk-otter")); put("limit", kotlinx.serialization.json.JsonPrimitive(300))
                }),
                transcript,
                System.currentTimeMillis(),
            )
        }
        shell = ShellModel(relay, settings)
    }

    private fun shoot(name: String, route: String) {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            val controller = rememberNavController()
            val nav = remember { Nav(controller, shell) }
            RelayTheme(Palette.Matte) {
                CompositionLocalProvider(LocalNav provides nav) {
                    Box(Modifier.fillMaxSize()) {
                        AppNav(nav, route)
                        HoldTray(shell, onOpen = {})
                    }
                }
            }
        }
        compose.mainClock.advanceTimeBy(2_500)
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", OPTIONS)
    }

    @Test fun start() = shoot("start", "start")
    @Test fun agents() = shoot("agents", "agents")
    @Test fun board() = shoot("board", "board?project=1")
    @Test fun task() = shoot("task", "task/101")
    @Test fun inbox() = shoot("inbox", "inbox")
    @Test fun notes() = shoot("notes", "notes?project=1")
    @Test fun note() = shoot("note", "note?id=201")
    @Test fun threads() = shoot("threads", "threads")
    @Test fun thread() = shoot("thread", "thread/301")
    @Test fun search() = shoot("search", "search")
    @Test fun settings() = shoot("settings", "settings")
    @Test fun outbox() = shoot("outbox", "outbox")
    @Test fun terminal() = shoot("terminal", "terminal/brisk-otter")
    @Test fun chat() = shoot("chat", "chat/brisk-otter")
    @Test fun session() = shoot("session", "session/brisk-otter")
    @Test fun launch() = shoot("launch", "launch?project=1")
    @Test fun tally() = shoot("tally", "tally")
    @Test fun pc() = shoot("pc", "pc")
    @Test fun projectSettings() = shoot("project-settings", "project-settings/1")
    @Test fun share() = shoot("share", "share?text=Crash%20in%20the%20login%20flow%0Asteps%20to%20reproduce")

    private companion object {
        val OPTIONS = RoborazziOptions(compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f))
    }
}
