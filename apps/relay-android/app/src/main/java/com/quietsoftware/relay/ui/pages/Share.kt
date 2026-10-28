package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.titleOf
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxRunner
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Text shared to Relay from another app (a link, an error message, a thought): made into a task,
 * a note, a thread, or mail to an agent. Everything but typing into a terminal waits in the
 * outbox when the PC is away. What it made opens in its place, so back does not return here to
 * make it twice.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ShareScreen(text: String, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    var body by remember { mutableStateOf(text) }
    // The current project until one is picked: a share can open the app before its state has loaded.
    var picked by remember { mutableStateOf<Long?>(null) }
    val projectId = picked ?: s.project?.num
    val title = titleOf(body, 120) ?: "Shared"
    val rest = body.substringAfter('\n', "").trim()

    /** The id the phone shows for what a change made: the PC's, or the temp one while it waits. */
    fun made(ch: Hub.Change): String? = when (ch) {
        is Hub.Change.Queued -> Optimistic.tempId(ch.entry.id)
        is Hub.Change.Now -> OutboxRunner.createdId(ch.result)
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Share to Relay", nav::back)
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.column().fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Field(body, { body = it }, Modifier.fillMaxWidth().heightIn(min = 120.dp), singleLine = false, maxLines = 10, style = Relay.type.body)
                    Eyebrow("Project")
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        s.projects.forEach { p -> Pill(p.name, selected = p.num == projectId, onClick = { picked = p.num }) }
                    }
                    Slab(Modifier.fillMaxWidth(), padding = androidx.compose.foundation.layout.PaddingValues(vertical = 4.dp)) {
                        ListRow(onClick = {
                            val p = projectId ?: return@ListRow
                            nav.shell.change("task.create", buildJsonObject { put("project_id", p); put("title", title); if (rest.isNotEmpty()) put("body", rest) }, "New task") { ch -> made(ch)?.let { id -> nav.instead { task(id) } } }
                        }) {
                            Glyph("board", 17.dp)
                            T("New task", Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                        }
                        ListRow(onClick = {
                            val p = projectId ?: return@ListRow
                            nav.shell.change("notes.create", buildJsonObject { put("project_id", p); put("body", body) }, "New note") { ch ->
                                nav.instead { made(ch)?.let { id -> note(id) } ?: notes(p) }
                            }
                        }) {
                            Glyph("brief", 17.dp)
                            T("New note", Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                        }
                        ListRow(onClick = {
                            nav.shell.change("thread.create", buildJsonObject { put("text", body) }, "New thread") { ch ->
                                nav.shell.pickSpace("threads")
                                made(ch)?.let { id -> nav.instead { thread(id) } }
                            }
                        }) {
                            Glyph("threads", 17.dp)
                            T("Ask in a new thread", Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                        }
                    }
                    val live = s.sessions.filter { it.projectId == projectId && it.state in setOf("running", "idle", "blocked") }
                    if (live.isNotEmpty()) {
                        Eyebrow("Mail an agent")
                        Slab(Modifier.fillMaxWidth(), padding = androidx.compose.foundation.layout.PaddingValues(vertical = 4.dp)) {
                            live.forEach { a ->
                                ListRow(onClick = {
                                    nav.shell.change("mailbox.send", buildJsonObject { put("project_id", a.projectId); put("to", a.name); put("text", body); put("priority", true) }, "Mail ${a.name}") { nav.instead { terminal(a.name) } }
                                }) {
                                    LampDot(a.lamp)
                                    T(a.name, Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                                    T("${a.provider} · ${a.role}", style = Relay.type.caption, color = c.ink3)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
