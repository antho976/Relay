package com.quietsoftware.relay.ui.pages

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Note
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import java.time.Instant
import kotlinx.coroutines.flow.flowOf
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
    val notes by remember(pid) { nav.relay.notes(pid) }.collectAsStateWithLifecycle(initialValue = emptyList())
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

            if (notes.isEmpty()) {
                item { Empty("notes", "No notes yet", "Notes are shared with the PC. Agents read only the tasks they are given.") }
            } else if (shown.isEmpty()) {
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
    val lines = note.body.lines().map { it.trim().trimStart('#', ' ') }.filter { it.isNotEmpty() }
    return (if (note.title.isNullOrBlank()) lines.drop(1) else lines).joinToString(" ").take(200)
}

/** Unsaved text, and what it started from (null for a new note). */
private data class Draft(val title: String, val body: String, val base: Note?) {
    val changed: Boolean
        get() = base?.let { title.trim() != (it.title ?: "").trim() || body != it.body } ?: (title.isNotBlank() || body.isNotBlank())
}

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
    var draft by remember { mutableStateOf(if (id == null) Draft("", "", null) else null) }
    var leave by remember { mutableStateOf<(() -> Unit)?>(null) }

    // A note made on this phone keeps its temp id until the PC numbers it; follow it there.
    val current by remember(ref) { ref?.let { nav.relay.resolved(it) } ?: flowOf<String?>(null) }
        .collectAsStateWithLifecycle(initialValue = ref)
    val stored by remember(current) { current?.let { nav.relay.note(it) } ?: flowOf<Note?>(null) }
        .collectAsStateWithLifecycle(initialValue = null)
    // A note this screen made shows from its own copy once the PC has numbered it and the phone's
    // temporary row has gone.
    val note: Note? = stored ?: created?.takeIf { it.id == current }
    val projectName = s.projects.firstOrNull { it.num == pid }?.name
    val editing = draft != null

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

    leave?.let { then ->
        Ask(
            title = "Keep your changes?",
            body = "This note has changes that are not saved.",
            onDismiss = { leave = null },
        ) {
            Key("Discard", { draft = null; leave = null; then() }, kind = KeyKind.Plain, compact = true)
            Key("Save", { save(); leave = null; then() }, kind = KeyKind.Primary, compact = true)
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
                    IconKey("pin", { pin(n) }, selected = n.pinned)
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
                            if (n == null) {
                                nav.back()
                            } else {
                                draft = null
                            }
                        },
                    )
                    n != null -> NoteView(n)
                    else -> Empty("file-text", "Note not found", "It may have been deleted on the PC.")
                }
            }
        }
    }
}

@Composable
private fun NoteView(n: Note) {
    val c = Relay.colors
    if (n.updatedAt.isNotEmpty()) {
        T("Edited ${ago(epoch(n.updatedAt))}", style = Relay.type.caption, color = c.ink3)
        Gap(12.dp)
    }
    if (n.body.isBlank()) T("Empty note.", style = Relay.type.ui, color = c.ink3) else Markdown(n.body, style = Relay.type.body, color = c.ink)
}

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
