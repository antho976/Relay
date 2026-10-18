package com.quietsoftware.relay.ui.tasks

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Label
import com.quietsoftware.relay.core.model.Module
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.pages.Ask
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private class Loaded(val task: Task?)

private val SIZE_CHOICES = listOf("" to "None") + SIZES.map { it to "$it · ${points(it)} pt" }

/**
 * New task, or edit one ([id] given) (task_pages.rs `compose`). An edit sends only the fields
 * that changed, with the values they started from as `expected`, so a change made on the PC
 * meanwhile is caught rather than overwritten; the outbox decides what to do about it then.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TaskEditScreen(id: String?, projectId: Long?, parentId: Long?, column: String?, nav: Nav) {
    val c = Relay.colors
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    // Editing a task made on this phone: follow it to the PC's id once the PC numbers it.
    val real by remember(id) { id?.let { nav.relay.resolved(it) } ?: flowOf(null) }.collectAsStateWithLifecycle(id)
    val found by remember(real) { real?.let { r -> nav.relay.task(r).map { Loaded(it) } } ?: flowOf(Loaded(null)) }.collectAsStateWithLifecycle(null)
    val editing = id != null

    var title by rememberSaveable { mutableStateOf("") }
    var body by rememberSaveable { mutableStateOf("") }
    var type by rememberSaveable { mutableStateOf("task") }
    var priority by rememberSaveable { mutableStateOf("medium") }
    var size by rememberSaveable { mutableStateOf("") }
    var col by rememberSaveable { mutableStateOf(column?.takeIf { it in Task.COLUMNS && it != "done" } ?: "backlog") }
    var moduleId by rememberSaveable { mutableStateOf<Long?>(null) }
    var labels by rememberSaveable { mutableStateOf("") }
    var parent by rememberSaveable { mutableStateOf(parentId) }
    var typed by rememberSaveable { mutableStateOf("") }
    var preview by rememberSaveable { mutableStateOf(false) }
    // The task as it was when the draft began: the edit's `expected`, kept across a rotation.
    var original by rememberSaveable { mutableStateOf<String?>(null) }

    val task = found?.task
    LaunchedEffect(task != null) {
        if (task != null && original == null) {
            title = task.title
            body = task.body
            type = task.type
            priority = task.priority
            size = task.size.orEmpty()
            moduleId = task.moduleId
            original = Wire.lenient.encodeToString(Task.serializer(), task)
        }
    }
    val orig = remember(original) { original?.let { runCatching { Wire.lenient.decodeFromString(Task.serializer(), it) }.getOrNull() } }

    val pid = task?.projectId ?: projectId ?: shell.project?.num
    val modules by remember(pid) { pid?.let { nav.relay.modules(it) } ?: flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList<Module>())
    val known by remember(pid) { pid?.let { nav.relay.labels(it) } ?: flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList<Label>())
    val all by remember(pid) { pid?.let { nav.relay.tasks(it) } ?: flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList<Task>())
    val chosen = labels.split('\n').filter { it.isNotBlank() }
    val parentTask = parent?.let { p -> all.firstOrNull { it.num == p } }
    // A sub-task starts in its parent's module, as on the PC.
    var seeded by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(parentTask != null) {
        if (!editing && !seeded && parentTask != null) {
            if (moduleId == null) moduleId = parentTask.moduleId
            seeded = true
        }
    }

    // What Back would throw away: a field changed from the task, or anything put into a new one.
    val dirty = if (editing) {
        orig != null && (title.trim() != orig.title || body != orig.body || type != orig.type || priority != orig.priority || size != orig.size.orEmpty() || moduleId != orig.moduleId)
    } else {
        title.isNotBlank() || body.isNotBlank() || chosen.isNotEmpty()
    }
    val canSave = title.isNotBlank() && pid != null && (!editing || orig != null)
    var leaving by remember { mutableStateOf(false) }

    fun leave() {
        if (dirty) leaving = true else nav.back()
    }
    BackHandler(enabled = dirty) { leaving = true }

    fun save() {
        val name = title.trim()
        if (name.isEmpty() || pid == null) return
        if (editing) {
            val was = orig ?: task ?: return
            val target = task ?: was
            val patch = LinkedHashMap<String, JsonElement>()
            val expected = LinkedHashMap<String, JsonElement>()
            fun diff(field: String, now: JsonElement, before: JsonElement) {
                if (now != before) {
                    patch[field] = now
                    expected[field] = before
                }
            }
            fun opt(v: String?): JsonElement = v?.let { JsonPrimitive(it) } ?: JsonNull
            diff("title", JsonPrimitive(name), JsonPrimitive(was.title))
            diff("body", JsonPrimitive(body), JsonPrimitive(was.body))
            diff("type", JsonPrimitive(type), JsonPrimitive(was.type))
            diff("priority", JsonPrimitive(priority), JsonPrimitive(was.priority))
            diff("size", opt(size.ifEmpty { null }), opt(was.size))
            diff("module_id", moduleId?.let { JsonPrimitive(it) } ?: JsonNull, was.moduleId?.let { JsonPrimitive(it) } ?: JsonNull)
            if (patch.isNotEmpty()) {
                nav.shell.change("task.update", buildJsonObject {
                    put("task_id", target.idJson())
                    for ((k, v) in patch) put(k, v)
                    put("expected", JsonObject(expected))
                }, "Edit ${target.named()}")
            }
            nav.back()
        } else {
            nav.shell.change("task.create", buildJsonObject {
                put("project_id", pid)
                put("title", name)
                put("body", body)
                put("type", type)
                put("priority", priority)
                size.takeIf { it.isNotEmpty() }?.let { put("size", it) }
                put("column", col)
                moduleId?.let { put("module_id", it) }
                parent?.let { put("parent_id", it) }
                if (chosen.isNotEmpty()) put("labels", JsonArray(chosen.map { JsonPrimitive(it) }))
            }, "New task") { change ->
                val made = when (change) {
                    is Hub.Change.Queued -> Optimistic.tempId(change.entry.id)
                    is Hub.Change.Now -> (change.result as? JsonObject)?.l("id")?.toString()
                }
                nav.back()
                made?.let { nav.task(it) }
            }
        }
    }

    fun addLabel(name: String) {
        val l = name.trim()
        if (l.isNotEmpty() && l !in chosen) labels = (chosen + l).joinToString("\n")
        typed = ""
    }

    if (leaving) {
        Ask(
            title = "Keep your changes?",
            body = if (editing) "This task has changes that are not saved." else "This task is not created yet.",
            onDismiss = { leaving = false },
        ) {
            Key("Discard", { leaving = false; nav.back() }, kind = KeyKind.Plain, compact = true)
            Key(if (editing) "Save" else "Create", { leaving = false; save() }, kind = KeyKind.Primary, compact = true, enabled = canSave)
        }
    }

    Page {
        Column(Modifier.fillMaxSize().imePadding()) {
            PageBar(if (editing) "Edit ${task?.named() ?: "task"}" else if (parent != null) "New sub-task" else "New task", ::leave, subtitle = shell.projects.firstOrNull { it.num == pid }?.name) {
                Key(if (editing) "Save" else "Create", ::save, Modifier.padding(end = 8.dp), kind = KeyKind.Primary, compact = true, enabled = canSave)
            }
            when {
                editing && orig == null -> {
                    if (found != null && task == null) Empty("trash", "This task is gone", "It was deleted, or this phone has not received it yet.")
                    return@Column
                }
                pid == null -> {
                    Empty("board", "No project yet", "Add a project on the PC first.")
                    return@Column
                }
            }
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 16.dp, vertical = 12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.column().fillMaxWidth()) {
                    val focus = remember { FocusRequester() }
                    LaunchedEffect(Unit) { if (!editing) runCatching { focus.requestFocus() } }
                    Eyebrow("Title")
                    Gap(6.dp)
                    // One line, as the PC's title: it wraps here, and a line break becomes a space.
                    Field(title, { title = it.replace('\n', ' ') }, Modifier.fillMaxWidth().focusRequester(focus), placeholder = "What needs doing", singleLine = false, maxLines = 4, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Next))

                    Row(Modifier.fillMaxWidth().padding(top = 18.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Eyebrow("Description", Modifier.weight(1f))
                        Segmented(listOf(false to "Write", true to "Preview"), preview, { preview = it })
                    }
                    if (preview) {
                        Box(Modifier.fillMaxWidth().heightIn(min = 140.dp).clip(Radii.key).border(1.dp, c.strong, Radii.key).padding(12.dp)) {
                            if (body.isBlank()) T("Nothing to preview.", style = Relay.type.ui, color = c.ink3) else Markdown(body)
                        }
                    } else {
                        Field(body, { body = it }, Modifier.fillMaxWidth(), placeholder = "Markdown: what, why, and how to tell it is done", singleLine = false, minLines = 6, maxLines = 18, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
                    }

                    Group("Type") {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            for ((v, label) in TYPES) Pill(label, selected = type == v, onClick = { type = v })
                        }
                    }
                    Group("Priority") { Segmented(PRIORITIES, priority, { priority = it }, Modifier.fillMaxWidth(), fill = true) }
                    Group("Size · how much work") { Segmented(SIZE_CHOICES, size, { size = it }, Modifier.fillMaxWidth(), fill = true) }
                    if (!editing) {
                        Group("Column") {
                            Segmented(Task.COLUMNS.filter { it != "done" }.map { it to Task.columnLabel(it) }, col, { col = it }, Modifier.fillMaxWidth(), fill = true)
                        }
                    }
                    val offered = modules.filter { it.completedAt == null || it.id == moduleId?.toString() }
                    Group("Module") {
                        if (offered.isEmpty()) T("No open modules in this project.", style = Relay.type.caption, color = c.ink3)
                        else FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            Pill("None", selected = moduleId == null, onClick = { moduleId = null })
                            for (m in offered) {
                                val n = m.id.toLongOrNull() ?: continue
                                Pill(m.name, selected = moduleId == n, glyph = "modules", onClick = { moduleId = n })
                            }
                        }
                    }
                    if (!editing) {
                        Group("Labels") {
                            if (chosen.isNotEmpty()) {
                                FlowRow(Modifier.padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                    for (l in chosen) LabelChip(l, onClick = { labels = (chosen - l).joinToString("\n") }, trailing = "close")
                                }
                            }
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Field(
                                    typed, { typed = it }, Modifier.weight(1f),
                                    placeholder = "Add a label",
                                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, imeAction = ImeAction.Done),
                                    keyboardActions = KeyboardActions(onDone = { addLabel(typed) }),
                                )
                                Key("Add", { addLabel(typed) }, enabled = typed.isNotBlank())
                            }
                            val needle = typed.trim().lowercase()
                            val suggest = known.map { it.name }.filter { it !in chosen && it.lowercase().contains(needle) }.take(12)
                            if (suggest.isNotEmpty()) {
                                FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                    for (l in suggest) LabelChip(l, onClick = { addLabel(l) }, trailing = "plus")
                                }
                            }
                        }
                        parent?.let { p ->
                            Group("Parent") {
                                FieldPill("#$p ${parentTask?.title.orEmpty()}".trim(), onClick = { parent = null }) { StatusRing(parentTask?.column ?: "backlog", 11.dp) }
                                T("Tap to make it a top-level task instead.", Modifier.padding(top = 6.dp, start = 2.dp), Relay.type.caption, c.ink3)
                            }
                        }
                    }
                    if (editing && orig != null && task != null && task.updatedAt != orig.updatedAt) {
                        T("This task changed since you opened it. Saving sends only your changes; if the same field changed, the outbox asks which to keep.", Modifier.padding(top = 18.dp), Relay.type.caption, c.waiting)
                    }
                    Gap(24.dp)
                }
            }
        }
    }
}

@Composable
private fun Group(title: String, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(top = 18.dp)) {
        Eyebrow(title)
        Gap(8.dp)
        content()
    }
}
