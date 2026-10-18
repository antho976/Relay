package com.quietsoftware.relay.ui.tasks

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Label
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as Data
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** The replica's answer for one task: null inside means it is not (or no longer) there. */
private class Found(val task: Task?)

/**
 * A task's page (task_pages.rs), laid out like an issue: title and status, then description,
 * labels, sub-tasks, relations, changelog, commits, the agents on it, a message to them, and
 * what happened. Edits wait in the outbox when the PC is away; Dispatch and the activity need it.
 */
@Composable
fun TaskScreen(id: String, nav: Nav) {
    // A task made on this phone shows under its temp id until the PC numbers it; then follow it.
    val shown by remember(id) { nav.relay.resolved(id) }.collectAsStateWithLifecycle(id)
    val found by remember(shown) { nav.relay.task(shown).map { Found(it) } }.collectAsStateWithLifecycle(null)
    val pending by remember(shown) { nav.relay.pending(Kind.Task, shown) }.collectAsStateWithLifecycle(false)
    // A temp row with no create behind it any more was dropped from the outbox.
    var lost by remember(shown) { mutableStateOf(false) }
    LaunchedEffect(shown, found) {
        if (Optimistic.isTemp(shown) && found != null && found?.task == null) lost = nav.relay.hub.outbox.get(shown.removePrefix(Optimistic.TEMP)) == null
    }
    val task = found?.task
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    val project = task?.let { t -> shell.projects.firstOrNull { it.num == t.projectId } }
    val all by remember(task?.projectId) { task?.let { nav.relay.tasks(it.projectId) } ?: flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList<Task>())
    // An edit the PC refused waits in the outbox for the person: the page says so instead of "saved".
    val refused = task?.let { t -> refusedEdit(shell.outbox, t.id) }
    var sheet by remember { mutableStateOf<String?>(null) }

    Page {
        Column(Modifier.fillMaxSize().imePadding()) {
            PageBar(task?.ref?.let { if (it == "new") "New task" else it } ?: "Task", nav::back, subtitle = project?.name) {
                if (task != null) {
                    IconKey("edit", { nav.editTask(task.id) }, label = "Edit task")
                    IconKey("more", { sheet = "more" })
                }
            }
            when {
                found == null -> Unit
                task == null -> if (!Optimistic.isTemp(shown) || lost) Gone(shown, nav)
                else -> TaskBody(task, all, pending || Optimistic.isTemp(task.id), refused, nav, Modifier.weight(1f)) { sheet = it }
            }
        }
    }

    if (task != null) {
        val ops = remember(nav) { TaskOps(nav) }
        when (sheet) {
            "move" -> BoardSheet({ sheet = null }, title = "Move ${task.named()} to") { close ->
                MoveChoices(task, { close { ops.approve(task) } }) { to -> close { ops.move(task, to, indexIn(all, task)) } }
            }
            "dispatch" -> DispatchSheet(task, nav) { sheet = null }
            "label" -> LabelSheet(task, nav) { sheet = null }
            "relate" -> RelateSheet(task, all, nav) { sheet = null }
            "more" -> BoardSheet({ sheet = null }, title = task.headline()) { close ->
                task.num?.let { n -> MenuRow("New agent on it", "play", { close { nav.launch(task.projectId, n) } }, detail = "the launch sheet") }
                MenuRow("Mailbox", "mail", { close { nav.mailbox(task.projectId, task.sessions.lastOrNull()) } })
                MenuRow("Board", "board", { close { nav.boardOf(task.projectId) } })
                MenuRow("Delete", "trash", { close { ops.delete(task); nav.back() } }, danger = true)
            }
        }
    }
}

/** No such task here: deleted, or a phone-made one dropped before the PC took it. */
@Composable
private fun Gone(id: String, nav: Nav) {
    val num = id.toLongOrNull()
    if (num == null) {
        Empty("outbox", "Not on the PC", "This task was made on the phone and dropped from the outbox before the PC took it.")
        return
    }
    Empty("trash", "This task is gone", "It was deleted, or this phone has not received it yet.") {
        Key("Restore", { nav.shell.change("task.restore", buildJsonObject { put("task_id", num) }, "Restore #$num") }, glyph = "undo")
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TaskBody(task: Task, all: List<Task>, pending: Boolean, refused: OutboxEntry?, nav: Nav, modifier: Modifier, open: (String) -> Unit) {
    val c = Relay.colors
    val ops = remember(nav) { TaskOps(nav) }
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    val byNum = remember(all) { all.mapNotNull { t -> t.num?.let { it to t } }.toMap() }
    val num = task.num
    val children = remember(all, num) { if (num == null) emptyList() else all.filter { it.parentId == num } }
    val canNest = num != null && task.depth + 1 < DEPTH_MAX
    val sessions = remember(shell.sessions) { shell.sessions.associateBy { it.name } }
    val activity by remember(num) {
        num?.let { n -> nav.relay.live("task.activity", buildJsonObject { put("task_id", n); put("limit", 30) }) } ?: flowOf(Data.Live())
    }.collectAsStateWithLifecycle(Data.Live())
    val entries = remember(activity.result) { entries(activity.result) }

    Column(modifier.fillMaxWidth()) {
        LazyColumn(
            Modifier.fillMaxWidth().weight(1f),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item(key = "head") {
                Column(Modifier.column().fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    T(task.title.ifBlank { "Untitled" }, style = Relay.type.heading, color = c.ink, weight = FontWeight.Medium)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                        ColumnPill(task.column) { open("move") }
                        StateBadge(task.state)
                        PriorityPill(task.priority)
                        task.size?.let { FieldPill("$it · ${points(it)} pt", mono = true) }
                        FieldPill(titled(task.type)) { TypeMark(task.type) }
                        task.moduleName?.let { m -> FieldPill(m) { Glyph("modules", 11.dp, c.ink2) } }
                    }
                    T(
                        listOfNotNull(epoch(task.createdAt).takeIf { it > 0 }?.let { "opened ${ago(it)}" }, epoch(task.updatedAt).takeIf { it > 0 }?.let { "updated ${ago(it)}" }).joinToString(" · "),
                        style = Relay.type.caption, color = c.ink3,
                    )
                    task.parentId?.let { p ->
                        FieldPill("Parent: #$p ${byNum[p]?.title.orEmpty()}".trim(), onClick = { nav.task(p.toString()) }) { Glyph("chevron-left", 11.dp, c.ink2) }
                    }
                    if (task.rollup.total > 0) Rollup(task.rollup.done, task.rollup.total)
                    if (refused != null) {
                        Column(
                            Modifier.fillMaxWidth().clip(Radii.board).background(c.held.copy(alpha = .08f)).border(1.dp, c.held.copy(alpha = .35f), Radii.board).padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Dot(c.held, 7.dp)
                                Column(Modifier.weight(1f)) {
                                    T(parkedWhy(refused), style = Relay.type.caption, color = c.heldText)
                                    T(refused.label, style = Relay.type.caption, color = c.ink3, maxLines = 2)
                                }
                            }
                            Key("Open the outbox", nav::outbox, glyph = "outbox", compact = true)
                        }
                    } else if (pending) {
                        Row(
                            Modifier.fillMaxWidth().clip(Radii.board).background(c.waiting.copy(alpha = .08f)).border(1.dp, c.waiting.copy(alpha = .35f), Radii.board).padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Dot(c.waiting, 7.dp)
                            T(
                                if (num == null) "Saved on this phone, not yet on the PC. It gets its number when the PC takes it." else "Saved on this phone, not yet on the PC.",
                                Modifier.weight(1f), Relay.type.caption, c.ink2,
                            )
                        }
                    }
                }
            }

            section("description") {
                Heading("Description") { TextKey("Edit", { nav.editTask(task.id) }, glyph = "edit") }
                if (task.body.isBlank()) T("No description.", style = Relay.type.ui, color = c.ink3)
                else Markdown(task.body)
            }

            section("labels") {
                Heading("Labels") { TextKey("Add", { open("label") }, glyph = "plus") }
                if (task.labels.isEmpty()) T("No labels.", style = Relay.type.ui, color = c.ink3)
                else FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (l in task.labels) LabelChip(l, onClick = { removeLabel(nav, task, l) }, trailing = "close")
                }
            }

            section("children-head") {
                Heading("Sub-tasks", count = children.size.takeIf { it > 0 }) {
                    if (canNest) TextKey("Add sub-task", { nav.newTask(task.projectId, parentId = num) }, glyph = "plus")
                }
                if (children.isEmpty()) {
                    T(
                        when {
                            num == null -> "Sub-tasks can be added once the PC has this task."
                            canNest -> "No sub-tasks."
                            else -> "No sub-tasks; this one is nested as deep as tasks go."
                        },
                        style = Relay.type.ui, color = c.ink3,
                    )
                }
            }
            items(children, key = { "child:" + it.id }) { child ->
                ListRow(Modifier.column(), onClick = { nav.task(child.id) }, shape = Radii.board, padding = PaddingValues(horizontal = 6.dp, vertical = 6.dp)) {
                    StatusRing(child.column, 13.dp)
                    T(child.num?.let { "#$it" } ?: "new", style = tiny, color = c.ink3)
                    T(child.title, Modifier.weight(1f), Relay.type.ui, if (child.column == "done") c.ink2 else c.ink, maxLines = 2)
                    T(Task.columnLabel(child.column), style = Relay.type.caption, color = c.ink3, maxLines = 1)
                }
            }

            section("relations") {
                Heading("Relations") { TextKey("Add", { open("relate") }, glyph = "plus") }
                val none = task.blockedBy.isEmpty() && task.blocks.isEmpty() && task.duplicateOf == null
                if (none) T("Not blocked by another task, nor a duplicate of one.", style = Relay.type.ui, color = c.ink3)
                for (b in task.blockedBy) Relation("Blocked by #$b", byNum[b], c.held, { nav.task(b.toString()) }) { unrelate(nav, task, "blocked_by", b) }
                for (b in task.blocks) Relation("Blocks #$b", byNum[b], c.ink3, { nav.task(b.toString()) }, null)
                task.duplicateOf?.let { d -> Relation("Duplicate of #$d", byNum[d], c.ink3, { nav.task(d.toString()) }) { unrelate(nav, task, "duplicate_of", d) } }
            }

            section("changelog") { Changelog(task, pending, nav) }

            if (task.commits.isNotEmpty()) {
                section("commits-head") { Heading("Commits", count = task.commits.size) }
                items(task.commits, key = { "sha:" + it.sha }) { cm ->
                    Row(Modifier.column().fillMaxWidth().padding(vertical = 8.dp, horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Glyph("commit", 14.dp, c.ink3)
                        T(cm.sha.take(10), style = Relay.type.mono, color = Palette.SHA)
                        T(cm.branch.orEmpty(), Modifier.weight(1f), Relay.type.mono, c.ink3, maxLines = 1)
                        epoch(cm.linkedAt).takeIf { it > 0 }?.let { T(ago(it), style = Relay.type.caption, color = c.ink3) }
                    }
                }
            }

            if (task.sessions.isNotEmpty()) {
                section("agents-head") { Heading("Agents", count = task.sessions.size) }
                items(task.sessions.distinct(), key = { "agent:$it" }) { name ->
                    val s = sessions[name]
                    ListRow(Modifier.column(), onClick = { if (s?.attachable == true) nav.terminal(name) else nav.session(name) }, shape = Radii.board, padding = PaddingValues(horizontal = 6.dp, vertical = 6.dp)) {
                        LampDot(s?.lamp ?: Lamp.Off, 8.dp)
                        Column(Modifier.weight(1f)) {
                            T(name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                            T(s?.let { listOf(titled(it.provider), titled(it.role), it.stateLabel.lowercase()).joinToString(" · ") } ?: "closed", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                        }
                        Glyph(if (s?.attachable == true) "terminal" else "chevron-right", 15.dp, c.ink3)
                    }
                }
                if (num != null) section("ask") { Ask(task, num, nav) }
            }

            if (num != null) {
                section("activity-head") {
                    Heading("Activity") { if (!activity.fresh && activity.result != null) StaleNote(activity.at) }
                    when {
                        activity.result == null && activity.error != null -> T(
                            if (activity.error?.code == "link.down") "The activity is read from the PC, which is out of reach." else activity.error?.message.orEmpty(),
                            style = Relay.type.ui, color = c.ink3,
                        )
                        activity.loading -> T("Reading…", style = Relay.type.ui, color = c.ink3)
                        entries.isEmpty() -> T("Nothing recorded yet.", style = Relay.type.ui, color = c.ink3)
                    }
                }
                items(entries, key = { it.key }) { e -> ActivityRow(e) }
            }
        }
        // While the keyboard is up the page is for typing; the keys come back when it goes down.
        if (!WindowInsets.isImeVisible) Actions(task, ops, open) { ops.delete(task); nav.back() }
    }
}

/** A section of the page, kept to the readable column. */
private fun LazyListScope.section(key: String, content: @Composable () -> Unit) {
    item(key = key) { Column(Modifier.column().fillMaxWidth()) { content() } }
}

@Composable
private fun Relation(text: String, other: Task?, tone: Color, onOpen: () -> Unit, onRemove: (() -> Unit)?) {
    val c = Relay.colors
    ListRow(onClick = onOpen, shape = Radii.board, padding = PaddingValues(start = 6.dp, end = 0.dp, top = 2.dp, bottom = 2.dp)) {
        Dot(tone, 6.dp)
        Column(Modifier.weight(1f)) {
            T(text, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            other?.let { T(it.title, style = Relay.type.caption, color = c.ink3, maxLines = 1) }
        }
        other?.let { StatusRing(it.column, 12.dp) }
        if (onRemove != null) IconKey("close", onRemove, size = 15.dp, label = "Remove relation")
    }
}

@Composable
private fun Changelog(task: Task, pending: Boolean, nav: Nav) {
    val c = Relay.colors
    var draft by rememberSaveable(task.id) { mutableStateOf<String?>(null) }
    // The changelog and stamp the draft started from: one written meanwhile must not be overwritten unseen.
    var base by rememberSaveable(task.id) { mutableStateOf<String?>(null) }
    var baseAt by rememberSaveable(task.id) { mutableStateOf<String?>(null) }
    val text = draft ?: task.changelog
    Heading("Changelog")
    Field(text, { if (draft == null) { base = task.changelog; baseAt = task.updatedAt }; draft = it }, Modifier.fillMaxWidth(), placeholder = "The sentence that ships in the patch notes", singleLine = false, minLines = 2, maxLines = 6)
    if (draft != null && draft != task.changelog) {
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            Key("Revert", { draft = null; base = null }, kind = KeyKind.Quiet, compact = true)
            Key("Save", {
                nav.shell.change("task.changelog.write", buildJsonObject {
                    put("task_id", task.idJson())
                    put("text", text)
                    // The PC's stamp, only when the phone has no edit of its own laid over it; the
                    // draft's own when the changelog moved under it, so the PC refuses, not overwrites.
                    val at = if (base != null && base != task.changelog) baseAt else task.updatedAt
                    if (!pending && !at.isNullOrEmpty()) put("expected_updated_at", at)
                }, "Changelog of ${task.named()}")
                draft = null
                base = null
            }, kind = KeyKind.Primary, compact = true)
        }
    } else if (task.changelog.isEmpty()) {
        T("Empty until someone writes it; agents add theirs when they finish.", Modifier.padding(top = 6.dp, start = 4.dp), Relay.type.caption, c.ink3)
    }
}

/** "Ask the agent": a message to an agent on the task (`mailbox.send` with `re_task`). */
@Composable
private fun Ask(task: Task, num: Long, nav: Nav) {
    val names = task.sessions.distinct()
    var to by rememberSaveable(task.id) { mutableStateOf<String?>(null) }
    var text by rememberSaveable(task.id) { mutableStateOf("") }
    val recipient = to?.takeIf { it in names } ?: names.last()
    val send = {
        val body = text.trim()
        if (body.isNotEmpty()) {
            nav.shell.change("mailbox.send", buildJsonObject {
                put("project_id", task.projectId)
                put("to", recipient)
                put("text", body)
                put("re_task", num)
            }, "Message to $recipient") { if (nav.shell.state.value.online) nav.shell.toast("Sent to $recipient") }
            text = ""
        }
    }
    Heading("Ask the agent")
    if (names.size > 1) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for (n in names) Pill(n, selected = n == recipient, onClick = { to = n })
        }
    }
    Field(
        text, { text = it }, Modifier.fillMaxWidth(),
        placeholder = "Message $recipient about this task",
        singleLine = false, minLines = 2, maxLines = 6,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
    )
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.End) {
        Key("Send", send, kind = KeyKind.Primary, glyph = "send", compact = true, enabled = text.isNotBlank())
    }
}

/** The page's own keys, always in reach: Move, Dispatch, Approve, Delete. */
@Composable
private fun Actions(task: Task, ops: TaskOps, open: (String) -> Unit, onDelete: () -> Unit) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding()) {
        Hairline(color = c.edge)
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp).column().align(Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Key("Move", { open("move") }, Modifier.weight(1f), glyph = "columns", compact = true)
            if (task.column != "done") Key("Dispatch", { open("dispatch") }, Modifier.weight(1f), glyph = "play", compact = true, enabled = task.num != null)
            if (task.column == "in_review") Key("Approve", { ops.approve(task) }, Modifier.weight(1f), kind = KeyKind.Primary, glyph = "check", compact = true)
            IconKey("trash", onDelete, tint = c.heldText, label = "Delete task")
        }
    }
}

private fun removeLabel(nav: Nav, task: Task, label: String) {
    nav.shell.change("task.label.remove", buildJsonObject { put("task_id", task.idJson()); put("label", label) }, "Remove label $label from ${task.named()}") {
        nav.shell.toast("Removed $label", undo = {
            nav.relay.change("task.label.add", buildJsonObject { put("task_id", task.idJson()); put("label", label) }, "Label ${task.named()} $label")
        })
    }
}

private fun unrelate(nav: Nav, task: Task, relation: String, other: Long) {
    nav.shell.change("task.unrelate", buildJsonObject { put("task_id", task.idJson()); put("relation", relation); put("other_id", other) }, "Unrelate ${task.named()} from #$other") {
        nav.shell.toast(if (relation == "blocked_by") "No longer blocked by #$other" else "No longer a duplicate of #$other", undo = {
            nav.relay.change("task.relate", buildJsonObject { put("task_id", task.idJson()); put("relation", relation); put("other_id", other) },
                if (relation == "blocked_by") "Mark ${task.named()} blocked by #$other" else "Mark ${task.named()} a duplicate of #$other")
        })
    }
}

/** Add a label: one the project already uses, or a new one typed. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LabelSheet(task: Task, nav: Nav, onDismiss: () -> Unit) {
    val known by remember(task.projectId) { nav.relay.labels(task.projectId) }.collectAsStateWithLifecycle(emptyList<Label>())
    var typed by rememberSaveable { mutableStateOf("") }
    val needle = typed.trim().lowercase()
    val offered = known.map { it.name }.filter { it !in task.labels && it.lowercase().contains(needle) }.take(24)
    BoardSheet(onDismiss, title = "Label ${task.named()}") { close ->
        val focus = rememberSheetFocus()
        fun add(name: String) {
            val label = name.trim()
            if (label.isEmpty()) return
            close { nav.shell.change("task.label.add", buildJsonObject { put("task_id", task.idJson()); put("label", label) }, "Label ${task.named()} $label") }
        }
        Row(Modifier.padding(horizontal = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Field(
                typed, { typed = it }, Modifier.weight(1f).focusRequester(focus),
                placeholder = "A label",
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { add(typed) }),
            )
            Key("Add", { add(typed) }, enabled = typed.isNotBlank())
        }
        if (offered.isNotEmpty()) {
            Gap(12.dp)
            FlowRow(Modifier.padding(horizontal = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for (l in offered) LabelChip(l, onClick = { add(l) }, trailing = "plus")
            }
        }
    }
}

/** Mark the task blocked by, or a duplicate of, another task of the project. */
@Composable
private fun RelateSheet(task: Task, all: List<Task>, nav: Nav, onDismiss: () -> Unit) {
    var relation by rememberSaveable { mutableStateOf("blocked_by") }
    var typed by rememberSaveable { mutableStateOf("") }
    val needle = typed.trim().lowercase().removePrefix("#")
    val others = remember(all, needle, relation) {
        all.filter { t ->
            val n = t.num ?: return@filter false
            n != task.num && !(relation == "blocked_by" && n in task.blockedBy) && !(relation == "duplicate_of" && n == task.duplicateOf) &&
                (needle.isEmpty() || n.toString() == needle || t.title.lowercase().contains(needle))
        }.take(60)
    }
    BoardSheet(onDismiss, title = "Relate ${task.named()}") { close ->
        Column(Modifier.padding(horizontal = 6.dp)) {
            Segmented(listOf("blocked_by" to "Blocked by", "duplicate_of" to "Duplicate of"), relation, { relation = it }, Modifier.fillMaxWidth(), fill = true)
            Gap(10.dp)
            Field(typed, { typed = it }, Modifier.fillMaxWidth(), placeholder = "Search title or #id", leading = "search")
            Gap(6.dp)
        }
        Box(Modifier.weight(1f, fill = false)) {
            LazyColumn {
                items(others, key = { it.id }) { t ->
                    MenuRow("#${t.num} ${t.title}", "board", {
                        val other = t.num ?: return@MenuRow
                        close {
                            nav.shell.change("task.relate", buildJsonObject { put("task_id", task.idJson()); put("relation", relation); put("other_id", other) },
                                if (relation == "blocked_by") "Mark ${task.named()} blocked by #$other" else "Mark ${task.named()} a duplicate of #$other")
                        }
                    }, lead = { StatusRing(t.column, 13.dp) })
                }
            }
        }
        if (others.isEmpty()) T("No task matches.", Modifier.padding(12.dp), Relay.type.caption, Relay.colors.ink3)
    }
}

// ---- Activity ----

/** One line of the timeline: an audited edit, a message about the task, or a comment. */
private data class Entry(val key: String, val at: Long, val who: String, val what: String, val body: String?, val tone: Tone)

private enum class Tone { Plain, Message, Refused }

/** An audit actor (`user`, `system`, `agent:<session>`), or a mailbox sender (`user` or a session). */
private fun actor(a: String?): String = when {
    a == null -> "Someone"
    a == "user" -> "You"
    a == "system" -> "Relay"
    else -> a.removePrefix("agent:")
}

private val FIELDS = mapOf("title" to "title", "body" to "description", "priority" to "priority", "size" to "size", "module_id" to "module", "state" to "state", "changelog" to "changelog", "type" to "type")

/** What an audit row did, in a few words (the old app's `describe`, filled out). */
private fun describe(row: JsonObject): String {
    val op = row.s("op").orEmpty().removePrefix("task.")
    val p = row.o("payload")
    return when (op) {
        "create" -> "created the task"
        "move" -> {
            val to = p?.s("column")
            val from = row.o("undo_op")?.o("payload")?.s("column")
            when {
                to == null -> "moved it"
                from != null && from != to -> "moved it ${Task.columnLabel(from)} → ${Task.columnLabel(to)}"
                else -> "moved it to ${Task.columnLabel(to)}"
            }
        }
        "update" -> p?.keys?.mapNotNull { FIELDS[it] }?.takeIf { it.isNotEmpty() }?.let { "edited the ${it.joinToString(", ")}" } ?: "edited it"
        "approve" -> "approved it"
        "delete" -> "deleted it"
        "restore" -> "restored it"
        "dispatch" -> (p?.s("session") ?: row.o("result_summary")?.o("session")?.s("name"))?.let { "dispatched it to $it" } ?: "dispatched it"
        "label.add" -> "added the label ${p?.s("label").orEmpty()}"
        "label.remove" -> "removed the label ${p?.s("label").orEmpty()}"
        "changelog.write" -> "wrote the changelog"
        "relate" -> p?.let { if (it.s("relation") == "duplicate_of") "marked it a duplicate of #${it.l("other_id")}" else "marked it blocked by #${it.l("other_id")}" } ?: "related it"
        "unrelate" -> p?.l("other_id")?.let { "removed its relation to #$it" } ?: "removed a relation"
        "link_commit" -> p?.s("sha")?.let { "linked commit ${it.take(8)}" } ?: "linked a commit"
        "parent.set" -> p?.l("parent_id")?.let { "set its parent to #$it" } ?: "made it a top-level task"
        "attach" -> "attached ${p?.s("name") ?: "a file"}"
        "detach" -> "removed an attachment"
        else -> op.replace('.', ' ').replace('_', ' ')
    }
}

private fun entries(result: JsonElement?): List<Entry> {
    val o = result as? JsonObject ?: return emptyList()
    val out = ArrayList<Entry>()
    for (h in o.arr("history")) {
        val r = h as? JsonObject ?: continue
        val kind = r.s("kind")
        val suffix = buildString {
            when (kind) {
                "held" -> append(" · held")
                "refused" -> append(" · refused")
                "error" -> append(" · failed")
            }
            if (r.l("undone_by") != null) append(" · undone")
        }
        out += Entry("a" + r.l("id"), epoch(r.s("ts")), actor(r.s("actor")), describe(r) + suffix, null, if (kind == "ok" || kind == null) Tone.Plain else Tone.Refused)
    }
    for (m in o.arr("messages")) {
        val r = m as? JsonObject ?: continue
        val priority = if (r.b("priority") == true) " · priority" else ""
        out += Entry("m" + r.l("id"), epoch(r.s("sent_at")), actor(r.s("from")), "wrote to ${r.s("to").orEmpty()}$priority", r.s("text"), Tone.Message)
    }
    for (cm in o.arr("comments")) {
        val r = cm as? JsonObject ?: continue
        out += Entry("c" + r.l("id"), epoch(r.s("created_at")), actor(r.s("author")), "commented", r.s("body"), Tone.Message)
    }
    return out.sortedByDescending { it.at }
}

@Composable
private fun ActivityRow(e: Entry) {
    val c = Relay.colors
    Row(Modifier.column().fillMaxWidth().padding(vertical = 7.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.padding(top = 6.dp)) {
            Dot(when (e.tone) {
                Tone.Message -> c.ink2
                Tone.Refused -> c.held
                Tone.Plain -> c.ink3
            }, 6.dp)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(e.who, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                T(" " + e.what, Modifier.weight(1f), Relay.type.ui, if (e.tone == Tone.Refused) c.heldText else c.ink2, maxLines = 2)
                if (e.at > 0) T(ago(e.at), style = Relay.type.caption, color = c.ink3)
            }
            e.body?.takeIf { it.isNotBlank() }?.let {
                Box(Modifier.fillMaxWidth().clip(Radii.board).background(c.ink.copy(alpha = .045f)).border(1.dp, c.edge, Radii.board).padding(horizontal = 10.dp, vertical = 8.dp)) {
                    Markdown(it, style = Relay.type.ui, color = c.ink)
                }
            }
        }
    }
}
