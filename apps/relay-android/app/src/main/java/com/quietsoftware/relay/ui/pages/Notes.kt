package com.quietsoftware.relay.ui.pages

import android.os.SystemClock
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.selection.toggleable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Note
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.model.titleOf
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.tasks.StatusRing
import com.quietsoftware.relay.ui.theme.Relay
import java.time.Instant
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.onEach
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** The project's notes: pinned notes, the rest, and search. */
@Composable
fun NotesScreen(projectId: Long?, nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val pid = projectId ?: s.project?.num
    SpaceFrame(nav) {
        if (pid == null) Empty("notes", "No project yet", "Notes belong to a project.") else NotesList(pid, nav)
    }
}

@Composable
private fun NotesList(pid: Long, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // Null until the phone's copy answers, so "No notes yet" never shows before the notes.
    val listed by remember(pid) { nav.relay.notes(pid) }.collectAsStateWithLifecycle(initialValue = null)
    val notes = listed.orEmpty()
    var query by rememberSaveable { mutableStateOf("") }

    // Notes with an edit or a create still on their way to the PC.
    val waiting = remember(s.outbox) {
        s.outbox.mapNotNull { Optimistic.target(it.op, it.payload, it.id) }.filter { it.first == Kind.Note }.map { it.second }.toSet()
    }
    val q = query.trim().lowercase()
    val shown = if (q.isEmpty()) notes else notes.filter { n -> (n.title ?: "").lowercase().contains(q) || n.body.lowercase().contains(q) }
    val pinned = shown.filter { it.pinned }
    val rest = shown.filter { !it.pinned }

    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        LazyColumn(
            Modifier.column().fillMaxSize(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp),
        ) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    T("Notes", Modifier.weight(1f), Relay.type.heading, c.ink)
                    Key("New note", { nav.newNote(pid) }, glyph = "plus", compact = true)
                }
            }
            item {
                Gap(12.dp)
                Field(query, { query = it }, Modifier.fillMaxWidth(), placeholder = "Search notes", leading = "search")
                Gap(4.dp)
            }

            if (listed != null && notes.isEmpty()) {
                item { Empty("notes", "No notes yet", "Notes are shared with the PC. Agents read only the tasks they are given.") }
            } else if (notes.isNotEmpty() && shown.isEmpty()) {
                item { Empty("search", "No note matches", query) }
            }
            if (pinned.isNotEmpty()) {
                item { SectionLabel("Pinned · ${pinned.size}") }
                itemsIndexed(pinned, key = { _, n -> n.id }) { index, n ->
                    NoteRow(n, pending = n.id in waiting || Optimistic.isTemp(n.id), divider = index > 0, onClick = { nav.note(n.id) })
                }
            }
            if (rest.isNotEmpty()) {
                item { SectionLabel("Notes · ${rest.size}") }
                itemsIndexed(rest, key = { _, n -> n.id }) { index, n ->
                    NoteRow(n, pending = n.id in waiting || Optimistic.isTemp(n.id), divider = index > 0, onClick = { nav.note(n.id) })
                }
            }
        }
    }
}

@Composable
private fun NoteRow(note: Note, pending: Boolean, divider: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    Column {
        if (divider) Hairline()
        Column(
            Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 4.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                if (pending) {
                    Dot(c.waiting, 6.dp)
                    Spacer(Modifier.width(8.dp))
                }
                T(note.heading, Modifier.weight(1f), Relay.type.body, c.ink, maxLines = 1, weight = FontWeight.Medium)
                if (note.updatedAt.isNotEmpty()) {
                    Spacer(Modifier.width(8.dp))
                    T(ago(epoch(note.updatedAt)), style = Relay.type.caption, color = c.ink3, maxLines = 1)
                }
            }
            val preview = preview(note)
            if (preview.isNotEmpty()) T(preview, style = Relay.type.caption, color = c.ink3, maxLines = 2)
        }
    }
}

/** The body on one line, without the line the title already shows. */
private fun preview(note: Note): String {
    val lines = plainLines(note.body)
    return (if (note.title.isNullOrBlank()) lines.drop(1) else lines).joinToString(" ").take(200)
}

/**
 * Markdown's lines as a preview reads them: the words, without the marks that only mean
 * something drawn (a fence, a table's rule and pipes, list bullets, heading hashes, emphasis).
 */
internal fun plainLines(body: String): List<String> = body.lines().mapNotNull { raw ->
    val line = raw.trim()
    when {
        line.startsWith("```") -> null
        TABLE_RULE.matches(line) -> null
        else -> line.trimStart('#', '>', ' ')
            .replace(LIST_MARK, "")
            .replace(LINK, "$1")
            .replace(EMPHASIS, "")
            .let { if (it.startsWith("|")) it.trim('|').split('|').joinToString(" · ") { cell -> cell.trim() } else it }
            .trim()
            .takeIf { it.isNotEmpty() }
    }
}

private val TABLE_RULE = Regex("""^\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?$""")
private val LIST_MARK = Regex("""^([-*+]|\d+[.)])\s+(\[[ xX]]\s+)?""")
private val LINK = Regex("""\[([^\]]*)]\([^)]*\)""")
private val EMPHASIS = Regex("""\*\*|__|`""")

/** Unsaved text, and what it started from (null for a new note). */
internal data class Draft(val title: String, val body: String, val base: Note?) {
    val changed: Boolean
        get() = base?.let { title.trim() != (it.title ?: "").trim() || body != it.body } ?: (title.isNotBlank() || body.isNotBlank())

    /** What the note is called as it stands: its title, else its first line (doc.rs `name`). */
    val name: String
        get() = title.trim().ifEmpty { titleOf(body, 80) ?: "Untitled" }
}

/**
 * A draft kept across the process being reclaimed or the activity made again (a font size or
 * locale change): the text, what it started from, and whether the note was open for editing at
 * all, as one list of strings.
 */
internal val DraftSaver: Saver<Draft?, Any> = Saver(
    save = { d -> if (d == null) arrayListOf(VIEWING) else arrayListOf(EDITING, d.title, d.body, d.base?.let { Wire.json.encodeToString(Note.serializer(), it) }.orEmpty()) },
    restore = { saved ->
        val v = (saved as List<*>).map { it as String }
        if (v.first() == VIEWING) null else Draft(v[1], v[2], v[3].takeIf { it.isNotEmpty() }?.let { Wire.json.decodeFromString(Note.serializer(), it) })
    },
)

private const val VIEWING = "viewing"
private const val EDITING = "editing"

/**
 * A note's key on the bus: its number, or, for one the PC has not numbered yet, the outbox entry
 * that creates it (the PC answers that reference with the new number).
 */
private fun noteKey(ref: String): JsonElement =
    ref.toLongOrNull()?.let { JsonPrimitive(it) } ?: Refs.ref(ref.removePrefix(Optimistic.TEMP))

/**
 * One note: read it, edit it, pin it, delete it (Undo on the toast). Edits go through the outbox
 * with the values the edit started from as `expected`, so a change made on the PC meanwhile is
 * a conflict to decide in the outbox, not a silent overwrite.
 */
@Composable
fun NoteScreen(id: String?, projectId: Long?, nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val pid = projectId ?: s.project?.num
    var ref by rememberSaveable(id) { mutableStateOf(id) }
    var created by remember { mutableStateOf<Note?>(null) }
    var draft by rememberSaveable(id, stateSaver = DraftSaver) { mutableStateOf(if (id == null) Draft("", "", null) else null) }
    var leave by remember { mutableStateOf<(() -> Unit)?>(null) }
    // Discard on a changed draft asks first, as the PC's editor does before it drops edits.
    var discarding by remember { mutableStateOf(false) }

    // A note made on this phone keeps its temp id until the PC numbers it; follow it there.
    val current by remember(ref) { ref?.let { nav.relay.resolved(it) } ?: flowOf<String?>(null) }
        .collectAsStateWithLifecycle(initialValue = ref)
    // Until the phone's copy answers, the note is being read, not missing.
    var loaded by remember(current) { mutableStateOf(current == null) }
    val stored by remember(current) { (current?.let { nav.relay.note(it) } ?: flowOf<Note?>(null)).onEach { loaded = true } }
        .collectAsStateWithLifecycle(initialValue = null)
    // A note this screen made shows from its own copy once the PC has numbered it and the phone's
    // temporary row has gone.
    val note: Note? = stored ?: created?.takeIf { it.id == current }
    val projectName = s.projects.firstOrNull { it.num == pid }?.name
    val editing = draft != null
    // The tasks made from this note (task.list by note_id), once the PC has numbered it.
    val noteNum = current?.toLongOrNull()
    val taskProject = note?.projectId?.takeIf { it > 0 } ?: pid
    val tasksLive by remember(noteNum, taskProject) {
        if (noteNum == null || taskProject == null) flowOf(Live())
        else nav.relay.live("task.list", buildJsonObject { put("project_id", taskProject); put("note_id", noteNum); put("summary", true) })
    }.collectAsStateWithLifecycle(initialValue = Live())
    val noteTasks = (tasksLive.result as? JsonObject)?.arr("tasks")?.mapNotNull { it.decodeAs<Task>() }.orEmpty()

    fun guard(then: () -> Unit) {
        if (draft?.changed == true) leave = then else then()
    }

    fun save() {
        val d = draft ?: return
        val title = d.title.trim()
        val base = d.base
        if (base == null) {
            val p = pid ?: return
            nav.shell.change(
                "notes.create",
                buildJsonObject {
                    put("project_id", p)
                    if (title.isNotEmpty()) put("title", title)
                    put("body", d.body)
                    put("pinned", false)
                },
                "New note",
            ) { change ->
                val entry = (change as? Hub.Change.Queued)?.entry ?: return@change
                val temp = Optimistic.tempId(entry.id)
                val now = Instant.now().toString()
                created = Note(id = temp, projectId = p, title = title.takeIf { it.isNotEmpty() }, body = d.body, createdAt = now, updatedAt = now)
                ref = temp
                draft = null
            }
        } else {
            val key = current?.let { noteKey(it) } ?: return
            val titleChanged = title != (base.title ?: "").trim()
            val bodyChanged = d.body != base.body
            // Only the fields this edit changes, so a PC edit to the other one merges, not conflicts.
            nav.shell.change(
                "notes.update",
                buildJsonObject {
                    put("note_id", key)
                    if (titleChanged) put("title", title)
                    if (bodyChanged) put("body", d.body)
                    put(
                        "expected",
                        buildJsonObject {
                            if (titleChanged) put("title", base.title?.let { JsonPrimitive(it) } ?: JsonNull)
                            if (bodyChanged) put("body", base.body)
                        },
                    )
                },
                "Edit note",
            )
            draft = null
        }
    }

    // A tick is an edit of the body like any other: through the outbox, against the body it was
    // made on, so a change on the PC meanwhile is a conflict to decide, not a lost tick.
    fun tick(n: Note, item: CheckItem) {
        val key = current?.let { noteKey(it) } ?: return
        val body = toggled(n.body, item.line) ?: return
        val what = plainCheck(item.text).take(40).ifEmpty { "an item" }
        nav.shell.change(
            "notes.update",
            buildJsonObject {
                put("note_id", key)
                put("body", body)
                put("expected", buildJsonObject { put("body", n.body) })
            },
            if (item.done) "Uncheck “$what”" else "Check “$what”",
        )
    }

    var madeAt by remember { mutableLongStateOf(0L) }

    // File > Create task from note (doc.rs `to_task`): the whole note becomes a Backlog task that
    // remembers its note, which lists it from then on. It is queued at once, so a second tap just
    // after is the same tap again, not a second task.
    fun toTask(n: Note) {
        val now = SystemClock.uptimeMillis()
        if (madeAt > 0 && now - madeAt < 1_500) return
        madeAt = now
        val p = n.projectId.takeIf { it > 0 } ?: pid ?: return
        val name = n.title?.trim()?.takeIf { it.isNotEmpty() } ?: plainLines(n.body).firstOrNull()?.take(60) ?: "Untitled"
        val text = n.body.trim()
        val source = "From the note “$name”."
        nav.shell.change(
            "task.create",
            buildJsonObject {
                put("project_id", p)
                put("title", name)
                put("body", if (text.isEmpty()) source else "$text\n\n$source")
                put("column", "backlog")
                current?.let { put("note_id", noteKey(it)) }
            },
            "New task from a note",
        ) { change ->
            val made = when (change) {
                is Hub.Change.Queued -> Optimistic.tempId(change.entry.id)
                is Hub.Change.Now -> (change.result as? JsonObject)?.l("id")?.toString()
            }
            if (made != null) nav.shell.toast("Created task “$name” in Backlog", "Open") { it.task(made) }
            else nav.shell.toast("Created task “$name” in Backlog")
        }
    }

    fun pin(n: Note) {
        val key = current?.let { noteKey(it) } ?: return
        nav.shell.change(
            "notes.pin",
            buildJsonObject { put("note_id", key); put("pinned", !n.pinned) },
            if (n.pinned) "Unpin" else "Pin",
        )
    }

    fun remove(n: Note) {
        val key = current?.let { noteKey(it) } ?: return
        nav.shell.change("notes.delete", buildJsonObject { put("note_id", key) }, "Delete note")
        nav.shell.toast("Deleted “${n.heading}”", undo = {
            nav.shell.change("notes.restore", buildJsonObject { put("note_id", key) }, "Restore note")
        })
        nav.back()
    }

    val back: () -> Unit = { guard { nav.back() } }
    BackHandler(enabled = draft?.changed == true) { leave = { nav.back() } }

    // The desktop's question (doc.rs `request_close`), with its three answers.
    leave?.let { then ->
        Ask(
            title = "Save changes to “${draft?.name ?: "Untitled"}” before closing?",
            body = "If you don't save, your changes will be lost.",
            onDismiss = { leave = null },
        ) {
            Key("Cancel", { leave = null }, kind = KeyKind.Quiet, compact = true)
            Key("Don't Save", { draft = null; leave = null; then() }, kind = KeyKind.Plain, compact = true)
            Key("Save", { save(); leave = null; then() }, kind = KeyKind.Primary, compact = true)
        }
    }

    // A new note's draft goes with the screen; an open note's, back to the note.
    fun discard() {
        if (note == null) nav.back() else draft = null
    }

    if (discarding) {
        Ask(title = "Discard your changes?", body = "The draft is not saved.", onDismiss = { discarding = false }) {
            Key("Cancel", { discarding = false }, kind = KeyKind.Quiet, compact = true)
            Key("Discard", { discarding = false; discard() }, kind = KeyKind.Danger, compact = true)
        }
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(
                title = note?.heading ?: if (editing) "New note" else "Note",
                onBack = back,
                subtitle = projectName,
            ) {
                val n = note
                if (n != null && !editing) {
                    IconKey("pin", { pin(n) }, selected = n.pinned, label = if (n.pinned) "Unpin" else "Pin")
                    IconKey("trash", { remove(n) })
                }
            }
            ScrollBody {
                val d = draft
                val n = note
                if (n != null) {
                    Segmented(
                        listOf("view" to "View", "edit" to "Edit"),
                        selected = if (d != null) "edit" else "view",
                        onSelect = { mode ->
                            if (mode == "edit") {
                                if (draft == null) draft = Draft(n.title.orEmpty(), n.body, n)
                            } else {
                                guard { draft = null }
                            }
                        },
                    )
                    Gap(14.dp)
                }
                when {
                    d != null -> EditForm(
                        d,
                        onChange = { draft = it },
                        onSave = { save() },
                        onDiscard = {
                            if (d.changed) {
                                discarding = true
                            } else {
                                discard()
                            }
                        },
                    )
                    n != null -> NoteView(n, noteTasks, nav, onTick = { tick(n, it) }, onTask = { toTask(n) })
                    !loaded -> Unit
                    else -> Empty("file-text", "Note not found", "It may have been deleted on the PC.")
                }
            }
        }
    }
}

@Composable
private fun NoteView(n: Note, tasks: List<Task>, nav: Nav, onTick: (CheckItem) -> Unit, onTask: () -> Unit) {
    val c = Relay.colors
    if (n.updatedAt.isNotEmpty()) {
        T("Edited ${ago(epoch(n.updatedAt))}", style = Relay.type.caption, color = c.ink3)
        Gap(12.dp)
    }
    // Under the title, as the desktop's strip: each task made from the note, a tap opens it.
    if (tasks.isNotEmpty()) {
        T(if (tasks.size == 1) "Task" else "Tasks", style = Relay.type.section, color = c.ink3)
        Gap(4.dp)
        for (t in tasks) {
            ListRow(onClick = { nav.task(t.id) }, shape = Radii.board, padding = PaddingValues(horizontal = 6.dp, vertical = 6.dp)) {
                StatusRing(t.column, 13.dp)
                T(t.ref, style = Relay.type.caption, color = c.ink3)
                T(t.title, Modifier.weight(1f), Relay.type.ui, if (t.column == "done") c.ink2 else c.ink, maxLines = 1)
                T(Task.columnLabel(t.column), style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
        }
        Gap(12.dp)
    }
    if (n.body.isBlank()) T("Empty note.", style = Relay.type.ui, color = c.ink3) else NoteBody(n.body, onTick)
    Gap(20.dp)
    Key("Create task from note", onTask, glyph = "board", compact = true)
}

/** The note drawn: its Markdown, with each `- [ ]` item a box that ticks the line in the note. */
@Composable
private fun NoteBody(body: String, onTick: (CheckItem) -> Unit) {
    val c = Relay.colors
    val runs = remember(body) { runs(body) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        for (run in runs) when (run) {
            is Run.Text -> Markdown(run.markdown, style = Relay.type.body, color = c.ink)
            is Run.Checks -> Column {
                for (item in run.items) CheckLine(item) { onTick(item) }
            }
        }
    }
}

@Composable
private fun CheckLine(item: CheckItem, onToggle: () -> Unit) {
    val c = Relay.colors
    val text = plainCheck(item.text)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = (item.depth * 18).dp)
            .heightIn(min = 36.dp)
            .clip(Radii.key)
            .toggleable(value = item.done, role = Role.Checkbox, onValueChange = { onToggle() })
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.padding(top = 3.dp)) { CheckBox(item.done) }
        T(
            text.ifEmpty { " " },
            Modifier.weight(1f),
            if (item.done) Relay.type.body.copy(textDecoration = TextDecoration.LineThrough) else Relay.type.body,
            if (item.done) c.ink3 else c.ink,
        )
    }
}

/** The desktop's note checkbox (notes.css `notes-check`): a 4dp-rounded square, filled live green with a tick once done. */
@Composable
private fun CheckBox(done: Boolean) {
    val c = Relay.colors
    val edge = c.ink.copy(alpha = .5f)
    Canvas(Modifier.size(16.dp)) {
        val r = CornerRadius(4.dp.toPx())
        if (done) {
            drawRoundRect(c.live, cornerRadius = r)
            val tick = Path().apply {
                moveTo(size.width * .27f, size.height * .52f)
                lineTo(size.width * .44f, size.height * .68f)
                lineTo(size.width * .74f, size.height * .34f)
            }
            drawPath(tick, c.wall, style = Stroke(1.6.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        } else {
            val w = 1.dp.toPx()
            drawRoundRect(edge, topLeft = Offset(w / 2, w / 2), size = Size(size.width - w, size.height - w), cornerRadius = r, style = Stroke(w))
        }
    }
}

/** A note's body in runs: Markdown drawn as it is, and the `- [ ]` items between it. */
private sealed interface Run {
    data class Text(val markdown: String) : Run
    data class Checks(val items: List<CheckItem>) : Run
}

/** One `- [ ]` item: [line] is its index among the body's lines, which a tick rewrites. */
private data class CheckItem(val line: Int, val depth: Int, val done: Boolean, val text: String)

/** A checklist item as the desktop's editor finds one (text.rs `marker`): `- [ ] `, `* [x] `, `+ [X] `. */
private val CHECK = Regex("""^([ \t]*)[-*+] \[([ xX])] (.*)$""")

private fun runs(body: String): List<Run> {
    val out = ArrayList<Run>()
    val text = ArrayList<String>()
    var fence: String? = null
    // Blank lines between two items keep them one list.
    fun flush() {
        if (text.any { it.isNotBlank() }) out += Run.Text(text.joinToString("\n"))
        text.clear()
    }
    body.replace("\r\n", "\n").split('\n').forEachIndexed { i, line ->
        val t = line.trimStart()
        val open = fence
        when {
            // A fenced block is code to the end: a `- [ ]` in it is an example, not an item.
            open != null -> {
                text += line
                if (t.startsWith(open)) fence = null
            }
            t.startsWith("```") || t.startsWith("~~~") -> {
                fence = t.take(3)
                text += line
            }
            else -> {
                val m = CHECK.matchEntire(line)
                if (m == null) {
                    text += line
                } else {
                    flush()
                    val item = CheckItem(i, (m.groupValues[1].replace("\t", "  ").length / 2).coerceAtMost(3), m.groupValues[2] != " ", m.groupValues[3])
                    val last = out.lastOrNull()
                    if (last is Run.Checks) out[out.lastIndex] = Run.Checks(last.items + item) else out += Run.Checks(listOf(item))
                }
            }
        }
    }
    flush()
    return out
}

/** [body] with the item on [line] ticked or unticked and nothing else touched; null when that line is no longer an item. */
private fun toggled(body: String, line: Int): String? {
    val lines = body.split('\n').toMutableList()
    val l = lines.getOrNull(line) ?: return null
    val m = CHECK.matchEntire(l.removeSuffix("\r")) ?: return null
    val box = m.groups[2]?.range?.first ?: return null
    lines[line] = l.substring(0, box) + (if (m.groupValues[2] == " ") "x" else " ") + l.substring(box + 1)
    return lines.joinToString("\n")
}

/** An item's words without the inline marks that only mean something drawn. */
private fun plainCheck(text: String): String = text.replace(LINK, "$1").replace(EMPHASIS, "").trim()

@Composable
private fun EditForm(d: Draft, onChange: (Draft) -> Unit, onSave: () -> Unit, onDiscard: () -> Unit) {
    Field(
        d.title,
        { onChange(d.copy(title = it)) },
        Modifier.fillMaxWidth(),
        placeholder = "Title (optional)",
        style = Relay.type.uiMedium,
    )
    Gap(10.dp)
    Field(
        d.body,
        { onChange(d.copy(body = it)) },
        Modifier.fillMaxWidth(),
        placeholder = "Write in Markdown",
        singleLine = false,
        minLines = 12,
        maxLines = Int.MAX_VALUE,
        style = Relay.type.code,
    )
    Gap(14.dp)
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Key("Save", onSave, kind = KeyKind.Primary, enabled = d.changed, compact = true)
        Key("Discard", onDiscard, kind = KeyKind.Quiet, compact = true)
    }
}
