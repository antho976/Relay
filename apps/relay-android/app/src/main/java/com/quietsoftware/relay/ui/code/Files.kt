package com.quietsoftware.relay.ui.code

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.HGap
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private const val SEARCH_LIMIT = 200
private const val READ_LIMIT = 1_048_576

/** A row of the tree: an entry, or a note where an open folder has not answered yet. */
private sealed interface TreeRow {
    val depth: Int

    class Item(val entry: TreeEntry, override val depth: Int, val open: Boolean) : TreeRow
    class Note(val key: String, val text: String, override val depth: Int) : TreeRow
}

/** The open folders as a plain list, for the screen's saved state. */
private val OPEN_SAVER = listSaver<SnapshotStateList<String>, String>(save = { it.toList() }, restore = { it.toMutableStateList() })

/**
 * The tree of one worktree. The root and each open folder are read on their own, one level
 * deep: a folder costs one `file.tree` call, and nothing below an open folder is read. [open]
 * is the screen's, so the folders stay open after a file and back.
 */
private class TreeModel(
    private val nav: Nav,
    private val projectId: Long,
    private val worktree: String?,
    private val scope: CoroutineScope,
    val open: SnapshotStateList<String>,
) {
    val reads = mutableStateMapOf<String, Live>()
    private val jobs = HashMap<String, Job>()

    /** The root, and every folder still open from before. */
    fun start() {
        read("")
        for (path in open) read(path)
    }

    fun stop() {
        jobs.values.forEach { it.cancel() }
        jobs.clear()
    }

    fun expand(path: String) {
        if (path in open) return
        open.add(path)
        read(path)
    }

    fun collapse(path: String) {
        val gone = open.filter { it == path || it.startsWith("$path/") }
        open.removeAll(gone)
        for (p in gone) {
            jobs.remove(p)?.cancel()
            reads.remove(p)
        }
    }

    private fun read(path: String) {
        if (jobs.containsKey(path)) return
        val payload = codePayload(projectId, worktree) {
            put("path", path)
            put("depth", 1)
            put("git_badges", true)
        }
        jobs[path] = scope.launch { nav.relay.live("file.tree", payload).collect { reads[path] = it } }
    }
}

/** The rows the open folders show, in the tree's own order (folders first, then by name). */
private fun flatten(tree: TreeModel): List<TreeRow> {
    val out = ArrayList<TreeRow>()
    fun walk(path: String, depth: Int) {
        val read = tree.reads[path]
        val entries = read?.result?.decodeAs<TreeOut>()?.entries
        if (entries == null) {
            out.add(TreeRow.Note(path, read?.error?.let(::refusal) ?: "Reading…", depth))
            return
        }
        for (e in entries) {
            val isOpen = e.kind == "dir" && e.path in tree.open
            out.add(TreeRow.Item(e, depth, isOpen))
            if (isOpen) walk(e.path, depth + 1)
        }
    }
    walk("", 0)
    return out
}

private class SearchReq(val text: String, val glob: String, val regex: Boolean)

private sealed interface FilesSheet {
    data object Worktrees : FilesSheet
    class NewIn(val into: String) : FilesSheet
    class Menu(val entry: TreeEntry) : FilesSheet
    class Create(val into: String, val kind: String) : FilesSheet
    class Rename(val entry: TreeEntry) : FilesSheet
    class Move(val entry: TreeEntry) : FilesSheet
    data object Trashed : FilesSheet
    class Trash(val entry: TreeEntry) : FilesSheet
    class Revert(val entry: TreeEntry) : FilesSheet
}

/**
 * A worktree's files: a tree that reads one folder at a time with git badges, content search,
 * and the operations the desktop has (new file or folder, rename, move into a folder, trash with
 * Undo, Recently trashed with Restore, revert to HEAD). Reads work with the PC away; changes need
 * it. Desktop: editor.rs, project_files.rs.
 */
@Composable
fun FilesScreen(projectId: Long, worktree: String?, nav: Nav) {
    val scope = rememberCoroutineScope()
    val online = rememberOnline(nav)
    val project by nav.relay.project(projectId).collectAsStateWithLifecycle(initialValue = null)
    var wt by rememberSaveable { mutableStateOf(worktree) }
    val listed = rememberLive(nav, "worktree.list", codePayload(projectId, null)).result?.decodeAs<WorktreeList>()?.worktrees.orEmpty()
    val primary = primaryOf(listed, project?.path)
    val current = listed.firstOrNull { it.path == wt } ?: primary
    val opened = rememberSaveable(wt, saver = OPEN_SAVER) { mutableStateListOf<String>() }
    val tree = remember(wt) { TreeModel(nav, projectId, wt, scope, opened) }
    DisposableEffect(tree) {
        tree.start()
        onDispose { tree.stop() }
    }
    val rows by remember(tree) { derivedStateOf { flatten(tree) } }
    val root = tree.reads[""]
    var sheet by remember { mutableStateOf<FilesSheet?>(null) }
    var searching by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    var glob by rememberSaveable { mutableStateOf("") }
    var regex by rememberSaveable { mutableStateOf(false) }
    // The search that ran, kept in its parts so its hits are back after a file.
    var askedText by rememberSaveable { mutableStateOf<String?>(null) }
    var askedGlob by rememberSaveable { mutableStateOf("") }
    var askedRegex by rememberSaveable { mutableStateOf(false) }
    val submitted = askedText?.let { SearchReq(it, askedGlob, askedRegex) }
    val hits = rememberLive(nav, "file.search", submitted?.let { req ->
        codePayload(projectId, wt) {
            put("query", req.text)
            if (req.glob.isNotEmpty()) put("glob", req.glob)
            if (req.regex) put("regex", true)
            put("limit", SEARCH_LIMIT)
        }
    })

    fun openFile(path: String, line: Int? = null) = nav.file(projectId, path, wt, line)

    fun toggle(entry: TreeEntry) {
        if (entry.path in tree.open) tree.collapse(entry.path) else tree.expand(entry.path)
    }

    fun trash(entry: TreeEntry) {
        scope.write(nav, "file.delete", codePayload(projectId, wt) { put("path", entry.path) }) { r ->
            val trashId = r?.decodeAs<TrashOut>()?.trashId ?: return@write
            nav.shell.toast("Moved ${entry.name} to the trash") { restoreFromTrash(nav, projectId, trashId, entry.name) }
        }
    }

    fun revert(entry: TreeEntry) {
        scope.write(nav, "file.restore_head", codePayload(projectId, wt) { put("path", entry.path) }) { r ->
            if (r != null) nav.shell.toast("Reverted ${entry.name} to HEAD")
        }
    }

    fun rename(entry: TreeEntry, newName: String) {
        scope.write(nav, "file.rename", codePayload(projectId, wt) {
            put("path", entry.path)
            put("new_name", newName)
        }) { r ->
            if (r != null) nav.shell.toast("Renamed to $newName")
        }
    }

    fun move(entry: TreeEntry, into: String) {
        scope.write(nav, "file.move", codePayload(projectId, wt) {
            put("path", entry.path)
            put("into", into)
        }) { r ->
            if (r == null) return@write
            if (into.isNotEmpty()) tree.expand(into)
            nav.shell.toast("Moved ${entry.name} to ${into.ifEmpty { "the worktree root" }}")
        }
    }

    fun create(into: String, kind: String, name: String) {
        val path = if (into.isEmpty()) name else "$into/$name"
        scope.write(nav, "file.create", codePayload(projectId, wt) {
            put("path", path)
            put("kind", kind)
        }) { r ->
            if (r == null) return@write
            if (into.isNotEmpty()) tree.expand(into)
            if (kind == "dir") tree.expand(path) else openFile(path)
        }
    }

    fun submit() {
        val text = query.trim()
        if (text.isEmpty()) return
        askedText = text
        askedGlob = glob.trim()
        askedRegex = regex
    }

    Page {
        Column(Modifier.fillMaxSize().navigationBarsPadding()) {
            PageBar(
                title = "Files",
                onBack = nav::back,
                subtitle = listOfNotNull(project?.name?.ifBlank { null }, current?.branch?.ifBlank { null }).joinToString(" · ").ifBlank { null },
                actions = {
                    IconKey("search", { searching = !searching }, selected = searching, label = if (searching) "Hide the search" else "Search")
                    IconKey("trash", { sheet = FilesSheet.Trashed }, label = "Recently trashed")
                    IconKey("file-plus", { sheet = FilesSheet.NewIn("") }, enabled = online, label = "New file or folder")
                },
            )
            CodeBody {
                WorktreeKey(
                    current = current,
                    primary = current != null && current.path == primary?.path,
                    onClick = { sheet = FilesSheet.Worktrees },
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                    online = online,
                )
                if (searching) {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Field(
                            value = query,
                            onValueChange = { query = it },
                            modifier = Modifier.weight(1f),
                            placeholder = "Search in worktree",
                            leading = "search",
                            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
                            keyboardActions = KeyboardActions(onSearch = { submit() }),
                        )
                    }
                    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Field(
                            value = glob,
                            onValueChange = { glob = it },
                            modifier = Modifier.weight(1f),
                            placeholder = "Only files like *.kt",
                            leading = "file",
                            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
                            keyboardActions = KeyboardActions(onSearch = { submit() }),
                        )
                        Pill("Regex", selected = regex, onClick = { regex = !regex })
                    }
                    val result = hits.result?.decodeAs<SearchOut>()?.hits
                    when {
                        submitted == null -> Empty("search", "Search in worktree", "Type a word or a pattern, then press search.")
                        result == null -> LiveWait(hits)
                        result.isEmpty() -> Empty("search", "No matches", "Nothing in this worktree matches.")
                        else -> LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
                            items(result) { hit ->
                                HitRow(hit, onClick = { openFile(hit.path, hit.line.takeIf { it > 0 }) })
                            }
                        }
                    }
                } else {
                    LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
                        if (root?.result != null && !root.fresh) item { StaleNote(root.at) }
                        val unread = root?.result == null && root?.error?.code == "link.down"
                        if (unread) item { NotReadYet() }
                        if (rows.isEmpty() && root?.result != null) {
                            item {
                                Empty("folder", "Nothing here", "This worktree has no files yet.", action = {
                                    Key("New file", { sheet = FilesSheet.NewIn("") }, kind = KeyKind.Plain, enabled = online)
                                })
                            }
                        }
                        items(if (unread) emptyList() else rows, key = { r ->
                            when (r) {
                                is TreeRow.Item -> "e:" + r.entry.path
                                is TreeRow.Note -> "n:" + r.key
                            }
                        }) { row ->
                            when (row) {
                                is TreeRow.Item -> TreeLine(
                                    item = row,
                                    onClick = { if (row.entry.kind == "dir") toggle(row.entry) else openFile(row.entry.path) },
                                    onLongClick = { sheet = FilesSheet.Menu(row.entry) },
                                )
                                is TreeRow.Note -> NoteLine(row.text, row.depth)
                            }
                        }
                        if (!unread && rows.isNotEmpty()) {
                            item(key = "hint") { T("Hold a folder or file for more", Modifier.padding(horizontal = 16.dp, vertical = 10.dp), Relay.type.caption, Relay.colors.ink3) }
                        }
                    }
                }
            }
        }
    }

    when (val s = sheet) {
        null -> Unit
        FilesSheet.Worktrees -> WorktreeSheet(
            worktrees = listed,
            primary = primary,
            selected = wt,
            onPick = { wt = it.path },
            onDismiss = { sheet = null },
        )
        is FilesSheet.NewIn -> ActionSheet(
            title = if (s.into.isEmpty()) "New in the worktree root" else "New in ${s.into}",
            subtitle = null,
            actions = listOf(
                SheetAction("New file", "file-plus", enabled = online) { sheet = FilesSheet.Create(s.into, "file") },
                SheetAction("New folder", "folder-plus", enabled = online) { sheet = FilesSheet.Create(s.into, "dir") },
            ),
            onDismiss = { sheet = null },
        )
        is FilesSheet.Menu -> {
            val e = s.entry
            val isDir = e.kind == "dir"
            val actions = buildList<SheetAction> {
                if (isDir) {
                    add(SheetAction("New file here", "file-plus", enabled = online) { sheet = FilesSheet.Create(e.path, "file") })
                    add(SheetAction("New folder here", "folder-plus", enabled = online) { sheet = FilesSheet.Create(e.path, "dir") })
                } else {
                    add(SheetAction("Open", "file") { openFile(e.path) })
                }
                add(SheetAction("Rename", "edit", enabled = online) { sheet = FilesSheet.Rename(e) })
                add(SheetAction("Move", "folder", enabled = online) { sheet = FilesSheet.Move(e) })
                if (!isDir && (e.badge == "M" || e.badge == "D")) {
                    add(SheetAction("Revert to HEAD", "undo", danger = true, enabled = online) { sheet = FilesSheet.Revert(e) })
                }
                add(SheetAction("Move to trash", "trash", danger = true, enabled = online) { sheet = FilesSheet.Trash(e) })
            }
            ActionSheet(title = e.name, subtitle = e.path, actions = actions, onDismiss = { sheet = null })
        }
        is FilesSheet.Create -> NameSheet(
            title = if (s.kind == "dir") "New folder" else "New file",
            initial = "",
            confirm = "Create",
            onSubmit = { name ->
                sheet = null
                create(s.into, s.kind, name)
            },
            onDismiss = { sheet = null },
        )
        is FilesSheet.Rename -> NameSheet(
            title = "Rename",
            initial = s.entry.name,
            confirm = "Rename",
            onSubmit = { name ->
                sheet = null
                rename(s.entry, name)
            },
            onDismiss = { sheet = null },
        )
        is FilesSheet.Move -> MoveSheet(nav, projectId, wt, s.entry, onMove = { into -> move(s.entry, into) }, onDismiss = { sheet = null })
        FilesSheet.Trashed -> TrashSheet(
            nav,
            projectId,
            project?.path,
            online,
            onRestore = { e -> scope.launch { restoreFromTrash(nav, projectId, e.id, e.originalPath.substringAfterLast('/')) } },
            onDismiss = { sheet = null },
        )
        is FilesSheet.Trash -> ConfirmSheet(
            title = "Move ${s.entry.name} to the trash?",
            body = if (s.entry.kind == "dir") {
                "The folder and everything in it go to the project trash (.relay/trash). Undo brings them back."
            } else {
                "It goes to the project trash (.relay/trash). Undo brings it back."
            },
            confirm = "Move to trash",
            danger = true,
            onConfirm = { trash(s.entry) },
            onDismiss = { sheet = null },
        )
        is FilesSheet.Revert -> ConfirmSheet(
            title = "Revert ${s.entry.name} to HEAD?",
            body = "Throws away every uncommitted change to this file. This cannot be undone.",
            confirm = "Revert",
            danger = true,
            onConfirm = { revert(s.entry) },
            onDismiss = { sheet = null },
        )
    }
}

@Composable
private fun TreeLine(item: TreeRow.Item, onClick: () -> Unit, onLongClick: () -> Unit) {
    val c = Relay.colors
    val e = item.entry
    val isDir = e.kind == "dir"
    val (glyph, tint) = if (isDir) (if (item.open) "folder-open" else "folder") to FOLDER_TINT else fileGlyph(e.name)
    val letter = e.badge.orEmpty()
    val guide = c.lineEmphasis
    Row(
        Modifier
            .fillMaxWidth()
            .height(40.dp)
            .drawBehind {
                val step = 16.dp.toPx()
                val first = 14.dp.toPx()
                for (i in 0 until item.depth) {
                    val x = first + step * i
                    drawLine(guide, Offset(x, 0f), Offset(x, size.height), strokeWidth = 1.dp.toPx())
                }
            }
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(start = 8.dp + 16.dp * item.depth, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (isDir) Glyph(if (item.open) "chevron-down" else "chevron-right", 12.dp, c.ink3) else Spacer(Modifier.width(12.dp))
        Glyph(glyph, 16.dp, tint)
        val deleted = letter == "D"
        T(
            e.name,
            Modifier.weight(1f),
            if (deleted) Relay.type.ui.copy(textDecoration = TextDecoration.LineThrough) else Relay.type.ui,
            if (deleted) Palette.GIT_DELETED else c.ink,
            maxLines = 1,
        )
        if (letter.isNotEmpty()) T(letter, style = Relay.type.mono, color = statusTint(letter, c), weight = FontWeight.Medium)
    }
}

@Composable
private fun NoteLine(text: String, depth: Int) {
    T(text, Modifier.padding(start = 30.dp + 16.dp * depth, top = 2.dp, bottom = 6.dp, end = 12.dp), Relay.type.caption, Relay.colors.ink3)
}

@Composable
private fun HitRow(hit: Hit, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 12.dp, vertical = 7.dp)) {
        Column(Modifier.weight(1f)) {
            T("${hit.path}:${hit.line}", style = Relay.type.mono, color = c.ink2, maxLines = 1)
            T(hit.text.trim(), style = Relay.type.mono, color = c.ink3, maxLines = 1)
        }
    }
}

private sealed interface FileSheet {
    data object Menu : FileSheet
    data object Discard : FileSheet
    data object Conflict : FileSheet
    data object Rename : FileSheet
    data object Move : FileSheet
    data object Revert : FileSheet
    data object Trash : FileSheet
}

/**
 * An edit under way, kept with the file's page rather than its screen, so a notification or
 * another page and back finds it as it was. Not saved state: a draft can be a whole MiB.
 */
private class FileDraft : ViewModel() {
    val editing = mutableStateOf(false)
    val base = mutableStateOf("")
    val draft = mutableStateOf("")
}

/**
 * One file: read with `file.read` (1 MiB at most), shown with line numbers and light colour, or
 * edited in full and saved with the hash of the text read, so an edit made on the PC since is
 * refused rather than overwritten. Images are drawn; anything binary says so. A search hit
 * opens at its [line], marked. Find (find_bar.rs) washes every match and steps through them.
 */
@Composable
fun FileScreen(projectId: Long, path: String, worktree: String?, line: Int?, nav: Nav) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val online = rememberOnline(nav)
    var current by rememberSaveable(path) { mutableStateOf(path) }
    var trashed by rememberSaveable { mutableStateOf<Long?>(null) }
    var reloads by remember { mutableIntStateOf(0) }
    val held = viewModel { FileDraft() }
    var editing by held.editing
    var base by held.base
    var draft by held.draft
    var saving by remember { mutableStateOf(false) }
    var sheet by remember { mutableStateOf<FileSheet?>(null) }
    var finding by rememberSaveable { mutableStateOf(false) }
    var findText by rememberSaveable { mutableStateOf("") }
    var matchCase by rememberSaveable { mutableStateOf(false) }
    var findRegex by rememberSaveable { mutableStateOf(false) }
    var step by rememberSaveable { mutableIntStateOf(0) }
    val payload = if (trashed != null || current.isEmpty()) {
        null
    } else {
        codePayload(projectId, worktree) {
            put("path", current)
            put("max_bytes", READ_LIMIT)
        }
    }
    val live = rememberLive(nav, "file.read", payload, key = reloads)
    val data = live.result?.decodeAs<FileRead>()
    val text = data?.text
    val name = current.substringAfterLast('/')
    val folder = current.substringBeforeLast('/', "")
    val editable = text != null && data?.truncated == false
    val dirty = editing && draft != base
    val findable = finding && !editing && text != null
    val shownLines = remember(text, findable) { if (findable && text != null) displayLines(text) else emptyList() }
    val matches = remember(shownLines, findText, matchCase, findRegex) { findIn(shownLines, findText, matchCase, findRegex) }
    val at = if (matches.isNullOrEmpty()) 0 else step.mod(matches.size)

    fun requestClose() {
        if (dirty) sheet = FileSheet.Discard else editing = false
    }
    BackHandler(enabled = editing) { requestClose() }

    fun startEdit() {
        val body = text ?: return
        base = body
        draft = body
        editing = true
    }

    fun save(overwrite: Boolean) {
        if (saving) return
        saving = true
        val body = draft
        val expected = if (overwrite) null else sha256Hex(base)
        scope.launch {
            try {
                val out = nav.relay.call("file.write", codePayload(projectId, worktree) {
                    put("path", current)
                    put("text", body)
                    if (expected != null) put("expected_sha256", expected)
                }).decodeAs<WriteOut>()
                base = body
                editing = false
                reloads++
                nav.shell.toast("Saved · +${out?.added ?: 0} -${out?.removed ?: 0}")
            } catch (e: BusException) {
                if (e.error.code == "file.edit_conflict") sheet = FileSheet.Conflict else nav.shell.toast(refusal(e.error))
            } finally {
                saving = false
            }
        }
    }

    fun trash() {
        scope.write(nav, "file.delete", codePayload(projectId, worktree) { put("path", current) }) { r ->
            r?.decodeAs<TrashOut>()?.trashId?.let { trashed = it }
        }
    }

    fun revert() {
        scope.write(nav, "file.restore_head", codePayload(projectId, worktree) { put("path", current) }) { r ->
            if (r != null) {
                reloads++
                nav.shell.toast("Reverted to HEAD")
            }
        }
    }

    fun rename(newName: String) {
        scope.write(nav, "file.rename", codePayload(projectId, worktree) {
            put("path", current)
            put("new_name", newName)
        }) { r ->
            val to = r?.decodeAs<TreeEntry>()?.path
            if (!to.isNullOrEmpty()) {
                current = to
                reloads++
                nav.shell.toast("Renamed to $newName")
            }
        }
    }

    fun move(into: String) {
        scope.write(nav, "file.move", codePayload(projectId, worktree) {
            put("path", current)
            put("into", into)
        }) { r ->
            val to = r?.decodeAs<TreeEntry>()?.path
            if (!to.isNullOrEmpty()) {
                current = to
                reloads++
                nav.shell.toast("Moved $name to ${into.ifEmpty { "the worktree root" }}")
            }
        }
    }

    fun undoTrash(id: Long) {
        scope.launch {
            try {
                val back = nav.relay.call("file.restore", buildJsonObject {
                    put("project_id", projectId)
                    put("trash_id", id)
                }).decodeAs<TreeEntry>()
                trashed = null
                back?.path?.takeIf { it.isNotEmpty() }?.let { current = it }
                reloads++
                nav.shell.toast("Restored ${back?.name?.ifEmpty { null } ?: name}")
            } catch (e: BusException) {
                nav.shell.toast(refusal(e.error))
            }
        }
    }

    Page {
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.ime.union(WindowInsets.navigationBars))) {
            PageBar(
                title = name,
                onBack = { if (editing) requestClose() else nav.back() },
                subtitle = listOfNotNull(folder.ifEmpty { "/" }, data?.size?.let { sizeText(it) }).joinToString(" · "),
                actions = {
                    if (editing) {
                        Key("Save", { save(false) }, kind = KeyKind.Primary, compact = true, enabled = dirty && online && !saving)
                        IconKey("close", { requestClose() }, tint = c.ink2, label = if (dirty) "Discard edits" else "Stop editing")
                    } else if (trashed == null) {
                        if (text != null) IconKey("search", { finding = !finding }, selected = finding, label = if (finding) "Close the find bar" else "Find in file")
                        if (editable) IconKey("edit", { startEdit() }, enabled = online, label = "Edit file")
                        IconKey("more", { sheet = FileSheet.Menu }, label = "File actions")
                    }
                },
            )
            val gone = trashed
            val body = text
            if (findable) {
                FindBar(
                    query = findText,
                    onQuery = { findText = it; step = 0 },
                    count = findCount(findText, matches, at),
                    matchCase = matchCase,
                    onCase = { matchCase = !matchCase; step = 0 },
                    regex = findRegex,
                    onRegex = { findRegex = !findRegex; step = 0 },
                    canStep = !matches.isNullOrEmpty(),
                    onPrevious = { step = at - 1 },
                    onNext = { step = at + 1 },
                    onClose = { finding = false },
                )
            }
            if (gone == null && live.result != null && !live.fresh) StaleNote(live.at)
            when {
                gone != null -> Empty("trash", "Moved to the trash", current, action = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Key("Undo", { undoTrash(gone) }, kind = KeyKind.Primary, enabled = online)
                        HGap(8.dp)
                        Key("Back", nav::back, kind = KeyKind.Quiet)
                    }
                })
                live.result == null -> LiveWait(live)
                data == null -> Empty("file", "Can't read this file", "The PC sent something the phone does not know how to show.")
                data.truncated -> Empty(
                    "file",
                    "Too large for the phone",
                    "This file is ${sizeText(data.size)}. The phone reads up to ${sizeText(READ_LIMIT.toLong())}; open it on the PC.",
                )
                body == null -> {
                    val image = data.bytesB64
                    if (image != null && data.mime.startsWith("image/")) {
                        ImageFile(image, Modifier.weight(1f).fillMaxWidth())
                    } else {
                        Empty("file", "Not shown on the phone", "Binary file, ${sizeText(data.size)}. Open it on the PC.")
                    }
                }
                editing -> EditArea(draft, onChange = { draft = it })
                else -> CodeView(body, name, Modifier.weight(1f), mark = line, find = if (findable && !matches.isNullOrEmpty()) FindHits(matches, at) else null)
            }
        }
    }

    when (sheet) {
        null -> Unit
        FileSheet.Menu -> ActionSheet(
            title = name,
            subtitle = current,
            actions = listOf(
                SheetAction("Rename", "edit", enabled = online) { sheet = FileSheet.Rename },
                SheetAction("Move", "folder", enabled = online) { sheet = FileSheet.Move },
                SheetAction("Revert to HEAD", "undo", danger = true, enabled = online) { sheet = FileSheet.Revert },
                SheetAction("Move to trash", "trash", danger = true, enabled = online) { sheet = FileSheet.Trash },
            ),
            onDismiss = { sheet = null },
        )
        FileSheet.Discard -> ConfirmSheet(
            title = "Discard your changes?",
            body = "The draft is not saved.",
            confirm = "Discard",
            danger = true,
            onConfirm = { editing = false },
            onDismiss = { sheet = null },
        )
        FileSheet.Conflict -> ConflictSheet(
            onReload = {
                editing = false
                reloads++
            },
            onKeep = { save(true) },
            onDismiss = { sheet = null },
        )
        FileSheet.Rename -> NameSheet(
            title = "Rename",
            initial = name,
            confirm = "Rename",
            onSubmit = { newName ->
                sheet = null
                rename(newName)
            },
            onDismiss = { sheet = null },
        )
        FileSheet.Move -> MoveSheet(
            nav,
            projectId,
            worktree,
            TreeEntry(path = current, name = name),
            onMove = { into -> move(into) },
            onDismiss = { sheet = null },
        )
        FileSheet.Revert -> ConfirmSheet(
            title = "Revert $name to HEAD?",
            body = "Throws away every uncommitted change to this file. This cannot be undone.",
            confirm = "Revert",
            danger = true,
            onConfirm = { revert() },
            onDismiss = { sheet = null },
        )
        FileSheet.Trash -> ConfirmSheet(
            title = "Move $name to the trash?",
            body = "It goes to the project trash (.relay/trash). Undo brings it back.",
            confirm = "Move to trash",
            danger = true,
            onConfirm = { trash() },
            onDismiss = { sheet = null },
        )
    }
}

/** A plain text field that grows with its text and scrolls the whole file; the draft is the file. */
@Composable
private fun ColumnScope.EditArea(draft: String, onChange: (String) -> Unit) {
    val c = Relay.colors
    val scroll = rememberScrollState()
    Box(Modifier.weight(1f).fillMaxWidth().background(c.screen).verticalScroll(scroll).padding(12.dp)) {
        BasicTextField(
            value = draft,
            onValueChange = onChange,
            modifier = Modifier.fillMaxWidth(),
            textStyle = codeTextStyle.copy(color = c.ink),
            cursorBrush = SolidColor(c.ink),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
        )
    }
}

@Composable
private fun ConflictSheet(onReload: () -> Unit, onKeep: () -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    SheetFrame(onDismiss) {
        T("Changed on the PC", style = Relay.type.title, color = c.ink)
        Gap(6.dp)
        T(
            "The file changed or was removed since you opened it, so your edit was not written. Reload it and your draft is discarded, or keep your edits and overwrite what is on the PC.",
            style = Relay.type.ui,
            color = c.ink2,
        )
        Gap(18.dp)
        Key("Reload", { onDismiss(); onReload() }, Modifier.fillMaxWidth(), kind = KeyKind.Plain)
        Gap(8.dp)
        Key("Keep my edits (overwrite)", { onDismiss(); onKeep() }, Modifier.fillMaxWidth(), kind = KeyKind.Danger)
    }
}
