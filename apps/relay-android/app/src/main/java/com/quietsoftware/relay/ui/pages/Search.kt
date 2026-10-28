package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Note
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay

/**
 * The command palette (shell.rs) for a phone: places to go, then everything the phone holds a
 * copy of — agents, tasks, notes, threads — found as you type, with or without the PC.
 */
@Composable
fun SearchScreen(nav: Nav) {
    val c = Relay.colors
    var query by rememberSaveable { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val tasks by remember { nav.relay.allTasks() }.collectAsStateWithLifecycle(emptyList())
    val notes by remember(s.projects) { kotlinx.coroutines.flow.combine(s.projects.mapNotNull { p -> p.num?.let { nav.relay.notes(it) } }.ifEmpty { listOf(kotlinx.coroutines.flow.flowOf(emptyList())) }) { it.toList().flatten() } }
        .collectAsStateWithLifecycle(emptyList<Note>())
    val q = query.trim().lowercase()
    val project = s.project?.num

    val pages = listOf(
        Triple("Agents", "agents") { nav.agents() },
        Triple("Board", "board") { nav.board(project) },
        Triple("Notes", "brief") { nav.notes(project) },
        Triple("Inbox", "inbox") { nav.inbox() },
        Triple("Outbox", "outbox") { nav.outbox() },
        Triple("New agent", "plus") { nav.launch(project) },
        Triple("New task", "plus") { nav.newTask(project) },
        Triple("New note", "plus") { nav.newNote(project) },
        Triple("New thread", "threads") { nav.space("threads") },
        Triple("Files", "folder") { project?.let { nav.files(it) } ?: Unit },
        Triple("Git", "branch") { project?.let { nav.git(it) } ?: Unit },
        Triple("Tally", "home") { nav.tally() },
        Triple("Arbiter", "arbiter") { nav.arbiter() },
        Triple("Avex", "avex") { nav.avex() },
        Triple("Aide", "aide") { nav.aide() },
        Triple("Skills", "skills") { nav.skills() },
        Triple("Plugins", "dashboard") { nav.plugins() },
        Triple("Your PC", "device") { nav.pc() },
        Triple("Settings", "gear") { nav.settings() },
        Triple("Home", "home") { nav.home() },
    ).filter { q.isEmpty() || it.first.lowercase().contains(q) }

    fun Task.matches() = q.isNotEmpty() && (title.lowercase().contains(q) || ref == q || ref == "#$q" || body.lowercase().contains(q))
    val sessions = if (q.isEmpty()) emptyList() else s.sessions.filter { it.name.lowercase().contains(q) || (it.intent ?: "").lowercase().contains(q) || it.branch.lowercase().contains(q) }
    val foundTasks = tasks.filter { it.matches() }.sortedByDescending { it.title.lowercase().contains(q) }.take(30)
    val foundNotes = if (q.isEmpty()) emptyList() else notes.filter { it.heading.lowercase().contains(q) || it.body.lowercase().contains(q) }.take(20)
    val threads = if (q.isEmpty()) emptyList() else s.threads.filter { it.title.lowercase().contains(q) || (it.preview ?: "").lowercase().contains(q) }.take(20)
    val projects = if (q.isEmpty()) emptyList() else s.projects.filter { it.name.lowercase().contains(q) }

    /** Go on the keyboard opens what is listed first. */
    fun openFirst() {
        if (q.isEmpty()) return
        when {
            pages.isNotEmpty() -> pages.first().third()
            projects.isNotEmpty() -> projects.first().num?.let(nav::project)
            sessions.isNotEmpty() -> nav.terminal(sessions.first().name)
            foundTasks.isNotEmpty() -> nav.task(foundTasks.first().id)
            foundNotes.isNotEmpty() -> nav.note(foundNotes.first().id)
            threads.isNotEmpty() -> nav.thread(threads.first().id)
        }
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Search", nav::back)
            Field(
                query, { query = it }, Modifier.fillMaxWidth().padding(horizontal = 12.dp).focusRequester(focus), placeholder = "Search or go to…", leading = "search",
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { openFirst() }),
            )
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
                if (pages.isNotEmpty()) item { SectionLabel("Go to") }
                items(pages, key = { "p" + it.first }) { (label, glyph, go) ->
                    ListRow(onClick = go) {
                        Glyph(glyph, 17.dp)
                        T(label, style = Relay.type.ui, color = c.ink)
                    }
                }
                if (projects.isNotEmpty()) item { SectionLabel("Projects") }
                items(projects, key = { "pr" + it.id }) { p ->
                    ListRow(onClick = { p.num?.let(nav::project) }) {
                        Glyph("folder", 17.dp)
                        T(p.name, style = Relay.type.ui, color = c.ink)
                    }
                }
                if (sessions.isNotEmpty()) item { SectionLabel("Agents") }
                items(sessions, key = { "s" + it.name }) { a ->
                    ListRow(onClick = { nav.terminal(a.name) }) {
                        LampDot(a.lamp)
                        Column(Modifier.weight(1f)) {
                            T(a.name, style = Relay.type.uiMedium, color = c.ink)
                            T("${a.provider} · ${a.role} · ${a.branch}", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                        }
                    }
                }
                if (foundTasks.isNotEmpty()) item { SectionLabel("Tasks") }
                items(foundTasks, key = { "t" + it.id }) { t ->
                    ListRow(onClick = { nav.task(t.id) }) {
                        T(t.ref, style = Relay.type.mono, color = c.ink3)
                        T(t.title, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 2)
                        T(Task.columnLabel(t.column), style = Relay.type.caption, color = c.ink3)
                    }
                }
                if (foundNotes.isNotEmpty()) item { SectionLabel("Notes") }
                items(foundNotes, key = { "n" + it.id }) { n ->
                    ListRow(onClick = { nav.note(n.id) }) {
                        Glyph("brief", 17.dp)
                        T(n.heading, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                    }
                }
                if (threads.isNotEmpty()) item { SectionLabel("Threads") }
                items(threads, key = { "th" + it.id }) { t ->
                    ListRow(onClick = { nav.thread(t.id) }) {
                        Glyph("threads", 17.dp)
                        T(t.title, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                    }
                }
                if (q.isNotEmpty() && pages.isEmpty() && projects.isEmpty() && sessions.isEmpty() && foundTasks.isEmpty() && foundNotes.isEmpty() && threads.isEmpty()) {
                    item { T("Nothing here matches.", Modifier.padding(16.dp), Relay.type.ui, c.ink3) }
                }
            }
        }
    }
}
