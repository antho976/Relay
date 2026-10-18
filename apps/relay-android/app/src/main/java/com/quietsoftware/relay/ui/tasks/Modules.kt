package com.quietsoftware.relay.ui.tasks

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Module
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private fun Module.idJson(): JsonElement = id.toLongOrNull()?.let { JsonPrimitive(it) } ?: Refs.ref(id.removePrefix(Optimistic.TEMP))

/** A module's progress, counted from the phone's copy of the board so it moves as cards do. */
private data class Progress(val total: Int, val done: Int, val inFlight: Boolean) {
    val fraction: Float get() = if (total == 0) 0f else done.toFloat() / total
}

/**
 * The project's modules (pages.rs, board.css "Modules"): open ones first, completed ones folded
 * away. A card opens to list its tasks; hold it, or its ⋯ key, to complete, rename or delete it.
 * Every edit goes through the outbox.
 */
@Composable
fun ModulesScreen(projectId: Long, nav: Nav) {
    val c = Relay.colors
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    val modules by remember(projectId) { nav.relay.modules(projectId) }.collectAsStateWithLifecycle(null as List<Module>?)
    val tasks by remember(projectId) { nav.relay.tasks(projectId) }.collectAsStateWithLifecycle(emptyList<Task>())
    val project = shell.projects.firstOrNull { it.num == projectId }
    val byModule = remember(tasks) { tasks.filter { it.moduleId != null }.groupBy { it.moduleId!! } }
    val pending = remember(shell.outbox) {
        shell.outbox.mapNotNullTo(HashSet()) { e -> Optimistic.target(e.op, e.payload, e.id)?.takeIf { it.first == Kind.Module }?.second }
    }
    var expanded by rememberSaveable { mutableStateOf<String?>(null) }
    var showDone by rememberSaveable { mutableStateOf(false) }
    var sheet by remember { mutableStateOf<String?>(null) }
    var menuFor by remember { mutableStateOf<String?>(null) }

    fun progress(m: Module): Progress {
        val list = m.id.toLongOrNull()?.let { byModule[it] }.orEmpty()
        return Progress(list.size, list.count { it.column == "done" }, list.any { it.column == "active" || it.column == "in_review" })
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Modules", nav::back, subtitle = project?.name) {
                Key("New module", { sheet = "new" }, Modifier.padding(end = 8.dp), kind = KeyKind.Primary, glyph = "plus", compact = true)
            }
            val list = modules
            when {
                list == null -> Unit
                list.isEmpty() -> Empty("modules", "No modules yet", "A module groups the tasks of one release outcome.") {
                    Key("New module", { sheet = "new" }, kind = KeyKind.Primary, glyph = "plus")
                }
                else -> {
                    val open = list.filter { it.completedAt == null }
                    val done = list.filter { it.completedAt != null }
                    val inModules = open.sumOf { progress(it).total }
                    val finished = open.sumOf { progress(it).done }
                    LazyColumn(
                        Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        item(key = "stats") {
                            T(
                                "${open.size} open · ${open.count { progress(it).inFlight }} in flight · ${done.size} completed" +
                                    if (inModules > 0) " · ${finished * 100 / inModules}% done" else "",
                                Modifier.column().fillMaxWidth().padding(bottom = 4.dp), Relay.type.caption, c.ink3,
                            )
                        }
                        items(open, key = { it.id }) { m ->
                            ModuleCard(
                                m, progress(m), m.id in pending || Optimistic.isTemp(m.id), expanded == m.id,
                                m.id.toLongOrNull()?.let { byModule[it] }.orEmpty(),
                                onToggle = { expanded = if (expanded == m.id) null else m.id },
                                onMenu = { menuFor = m.id },
                                onTask = { nav.task(it) },
                                modifier = Modifier.animateItem().column(),
                            )
                        }
                        if (open.isEmpty()) {
                            item(key = "no-open") { T("Every module is completed.", Modifier.column().fillMaxWidth().padding(vertical = 12.dp), Relay.type.ui, c.ink3) }
                        }
                        if (done.isNotEmpty()) {
                            item(key = "done-head") {
                                Heading("Completed", Modifier.column(), count = done.size) {
                                    TextKey(if (showDone) "Hide" else "Show", { showDone = !showDone }, glyph = if (showDone) "chevron-up" else "chevron-down")
                                }
                            }
                            if (showDone) {
                                items(done, key = { it.id }) { m ->
                                    ModuleCard(
                                        m, progress(m), m.id in pending, expanded == m.id,
                                        m.id.toLongOrNull()?.let { byModule[it] }.orEmpty(),
                                        onToggle = { expanded = if (expanded == m.id) null else m.id },
                                        onMenu = { menuFor = m.id },
                                        onTask = { nav.task(it) },
                                        modifier = Modifier.animateItem().column(),
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    val menuModule = menuFor?.let { id -> modules?.firstOrNull { it.id == id } }
    if (menuModule != null) {
        BoardSheet({ menuFor = null }, title = menuModule.name) { close ->
            val m = menuModule
            if (m.completedAt == null) {
                MenuRow("Complete", "check", { close { undoable(nav, "module.complete", buildJsonObject { put("module_id", m.idJson()) }, "Complete ${m.name}", "Completed ${m.name}") { nav.relay.change("module.reopen", buildJsonObject { put("module_id", m.idJson()) }, "Reopen ${m.name}") } } })
            } else {
                MenuRow("Reopen", "undo", { close { undoable(nav, "module.reopen", buildJsonObject { put("module_id", m.idJson()) }, "Reopen ${m.name}", "Reopened ${m.name}") { nav.relay.change("module.complete", buildJsonObject { put("module_id", m.idJson()) }, "Complete ${m.name}") } } })
            }
            MenuRow("Rename or reprioritise", "edit", { close { sheet = "edit:" + m.id } })
            MenuRow("Delete", "trash", {
                close { undoable(nav, "module.delete", buildJsonObject { put("module_id", m.idJson()) }, "Delete ${m.name}", "Deleted ${m.name}") { nav.relay.change("module.restore", buildJsonObject { put("module_id", m.idJson()) }, "Restore ${m.name}") } }
            }, detail = "its tasks stay on the board", danger = true)
        }
    }

    val editing = sheet?.removePrefix("edit:")?.takeIf { sheet?.startsWith("edit:") == true }?.let { id -> modules?.firstOrNull { it.id == id } }
    when {
        sheet == "new" -> ModuleSheet(null, { sheet = null }) { name, priority ->
            nav.shell.change("module.create", buildJsonObject { put("project_id", projectId); put("name", name); put("priority", priority) }, "New module $name")
        }
        editing != null -> ModuleSheet(editing, { sheet = null }) { name, priority ->
            val changed = buildJsonObject {
                if (name != editing.name) put("name", name)
                if (priority != editing.priority) put("priority", priority)
            }
            if (changed.isNotEmpty()) {
                nav.shell.change("module.update", buildJsonObject {
                    put("module_id", editing.idJson())
                    for ((k, v) in changed) put(k, v)
                    put("expected", buildJsonObject {
                        if ("name" in changed) put("name", editing.name)
                        if ("priority" in changed) put("priority", editing.priority)
                    })
                }, "Edit ${editing.name}")
            }
        }
    }
}

@Composable
private fun ModuleCard(
    m: Module,
    p: Progress,
    pending: Boolean,
    open: Boolean,
    tasks: List<Task>,
    onToggle: () -> Unit,
    onMenu: () -> Unit,
    onTask: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = Relay.colors
    val done = m.completedAt != null
    Column(
        modifier.fillMaxWidth().clip(Radii.board).background(c.ink.copy(alpha = .045f)).border(1.dp, if (open) c.strong else c.edge, Radii.board).animateContentSize(),
    ) {
        Column(
            Modifier.fillMaxWidth().combinedClickable(onLongClick = onMenu, onClick = onToggle).padding(start = 14.dp, top = 6.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                m.icon?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.ui) }
                T(m.name, Modifier.weight(1f), Relay.type.ui.copy(fontSize = 15.sp), if (done) c.ink2 else c.ink, maxLines = 2, weight = FontWeight.SemiBold)
                if (done) {
                    Box(Modifier.clip(Radii.board).background(ROLLUP).padding(horizontal = 8.dp, vertical = 1.dp)) {
                        T("done", style = tiny, color = Color.White, weight = FontWeight.SemiBold)
                    }
                }
                IconKey("more", onMenu, size = 16.dp, label = "Module actions")
            }
            Row(Modifier.padding(end = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PriorityPill(m.priority)
                T(
                    if (p.total == 0) "No tasks" else "${p.done} of ${p.total} done",
                    Modifier.weight(1f), tiny, c.ink2, maxLines = 1,
                )
                if (pending) WaitingMark()
                T("${(p.fraction * 100).toInt()}%", style = Relay.type.mono.copy(fontSize = 11.sp), color = c.ink2)
            }
            Bar(p.fraction, Modifier.padding(end = 14.dp))
        }
        if (open) {
            Column(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 8.dp)) {
                if (tasks.isEmpty()) {
                    T("No tasks in this module yet. Give a task this module from its edit page.", Modifier.padding(6.dp), Relay.type.caption, c.ink3)
                }
                for (t in tasks.sortedWith(compareBy({ Task.COLUMNS.indexOf(it.column) }, { it.position }))) {
                    ListRow(onClick = { onTask(t.id) }, shape = Radii.board, padding = PaddingValues(horizontal = 6.dp, vertical = 4.dp)) {
                        StatusRing(t.column, 13.dp)
                        T(t.num?.let { "#$it" } ?: "new", style = tiny, color = c.ink3)
                        T(t.title, Modifier.weight(1f), Relay.type.ui, if (t.column == "done") c.ink2 else c.ink, maxLines = 2)
                        Glyph("chevron-right", 13.dp, c.ink3)
                    }
                }
            }
        }
    }
}

/** New module, or rename and reprioritise one. */
@Composable
private fun ModuleSheet(module: Module?, onDismiss: () -> Unit, onSave: (name: String, priority: String) -> Unit) {
    var name by rememberSaveable { mutableStateOf(module?.name.orEmpty()) }
    var priority by rememberSaveable { mutableStateOf(module?.priority ?: "medium") }
    BoardSheet(onDismiss, title = if (module == null) "New module" else "Edit ${module.name}") { close ->
        val save = { if (name.isNotBlank()) close { onSave(name.trim(), priority) } }
        val focus = rememberSheetFocus()
        Column(Modifier.padding(horizontal = 6.dp)) {
            Eyebrow("Name")
            Gap(6.dp)
            Field(
                name, { name = it }, Modifier.fillMaxWidth().focusRequester(focus),
                placeholder = "A release outcome, as \"Offline board\"",
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { save() }),
            )
            Gap(14.dp)
            Eyebrow("Priority")
            Gap(6.dp)
            Segmented(PRIORITIES, priority, { priority = it }, Modifier.fillMaxWidth(), fill = true)
            Gap(16.dp)
            Key(if (module == null) "Create" else "Save", save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, enabled = name.isNotBlank())
        }
    }
}
