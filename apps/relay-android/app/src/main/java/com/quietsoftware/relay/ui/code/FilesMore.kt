package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.HGap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// ---- Move ----

/**
 * Where to move [entry]: a folder of the worktree, picked one level at a time (`file.tree`), as
 * the desktop's drop of a file on a folder. Into itself, or where it already is, is no move.
 */
@Composable
fun MoveSheet(nav: Nav, projectId: Long, worktree: String?, entry: TreeEntry, onMove: (String) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    val from = entry.path.substringBeforeLast('/', "")
    var at by rememberSaveable(entry.path) { mutableStateOf(from) }
    val live = rememberLive(nav, "file.tree", codePayload(projectId, worktree) {
        put("path", at)
        put("depth", 1)
    })
    val folders = live.result?.decodeAs<TreeOut>()?.entries.orEmpty().filter { it.kind == "dir" && it.path != entry.path }
    val can = at != from && at != entry.path && !at.startsWith(entry.path + "/")
    SheetFrame(onDismiss) {
        T("Move ${entry.name}", style = Relay.type.title, color = c.ink, maxLines = 1)
        T("Into ${at.ifEmpty { "the worktree root" }}", style = Relay.type.mono, color = c.ink3, maxLines = 1)
        Gap(8.dp)
        LazyColumn(Modifier.fillMaxWidth().heightIn(max = 360.dp)) {
            if (at.isNotEmpty()) {
                item(key = "up") {
                    ListRow(onClick = { at = at.substringBeforeLast('/', "") }) {
                        Glyph("arrow-up", 16.dp, c.ink2)
                        T(at.substringBeforeLast('/', "").ifEmpty { "Worktree root" }, Modifier.weight(1f), Relay.type.ui, c.ink2, maxLines = 1)
                    }
                }
            }
            if (live.result == null) {
                item(key = "wait") { LiveWait(live) }
            } else if (folders.isEmpty()) {
                item(key = "none") { T("No folders in here.", Modifier.padding(horizontal = 12.dp, vertical = 10.dp), Relay.type.caption, c.ink3) }
            }
            items(folders, key = { it.path }) { f ->
                ListRow(onClick = { at = f.path }) {
                    Glyph("folder", 16.dp, FOLDER_TINT)
                    T(f.name, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                    Glyph("chevron-right", 13.dp, c.ink3)
                }
            }
        }
        Gap(14.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            Key("Cancel", onDismiss, kind = KeyKind.Quiet)
            HGap(8.dp)
            Key("Move here", { onDismiss(); onMove(at) }, kind = KeyKind.Primary, enabled = can)
        }
    }
}

// ---- Recently trashed ----

/** A file in Relay trash (`file.trash.list`). */
@Serializable
data class TrashEntry(
    val id: Long = 0,
    @SerialName("original_path") val originalPath: String = "",
    val worktree: String = "",
    @SerialName("created_at") val createdAt: String = "",
    val available: Boolean = true,
)

@Serializable
private data class TrashList(val entries: List<TrashEntry> = emptyList())

/**
 * Files moved to Relay trash in this project, newest first, each with Restore (editor.rs
 * "Recently trashed"). Restore puts one back into the checkout it was deleted from.
 */
@Composable
fun TrashSheet(nav: Nav, projectId: Long, projectPath: String?, online: Boolean, onRestore: (TrashEntry) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    val live = rememberLive(nav, "file.trash.list", buildJsonObject { put("project_id", projectId) })
    val entries = live.result?.decodeAs<TrashList>()?.entries
    SheetFrame(onDismiss) {
        T("Recently trashed", style = Relay.type.title, color = c.ink)
        Gap(4.dp)
        T("Files moved to Relay trash in this project, newest first. Each copy is kept in the checkout it was deleted from.", style = Relay.type.caption, color = c.ink3)
        Gap(8.dp)
        when {
            entries == null -> LiveWait(live)
            entries.isEmpty() -> T("Nothing from this project is in Relay trash.", Modifier.padding(vertical = 10.dp), Relay.type.caption, c.ink3)
            else -> LazyColumn(Modifier.fillMaxWidth().heightIn(max = 420.dp)) {
                items(entries, key = { it.id }) { e ->
                    Column {
                        if (e != entries.first()) Hairline()
                        Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                T(e.originalPath, style = Relay.type.mono, color = c.ink, maxLines = 2)
                                val place = listOfNotNull(checkoutName(e.worktree, projectPath), epoch(e.createdAt).takeIf { it > 0 }?.let { ago(it) }).joinToString(" · ")
                                T(if (e.available) place else "$place · no longer on disk", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                            }
                            HGap(8.dp)
                            Key("Restore", { onRestore(e) }, kind = KeyKind.Quiet, compact = true, enabled = online && e.available)
                        }
                    }
                }
            }
        }
    }
}

/** The checkout a trashed file came from, as the desktop names it. */
private fun checkoutName(worktree: String, projectPath: String?): String {
    val w = worktree.trimEnd('/')
    return if (w.isEmpty() || w == "@project" || w == projectPath?.trimEnd('/')) "Primary checkout" else w.substringAfterLast('/')
}

// ---- Find in file ----

/** One match: a line of the file as drawn (tabs as four spaces), and its columns there. */
data class Match(val line: Int, val start: Int, val end: Int)

/** What CodeView marks: every match, and the one stepped to. */
class FindHits(val matches: List<Match>, val current: Int)

/** At most this many matches are marked, so a one-letter search of a big file stays quick. */
const val FIND_MAX = 10_000

/**
 * Every match of [query] in [lines] (find_bar.rs: match case, regular expression), in order;
 * null when [regex] is on and the pattern does not parse.
 */
fun findIn(lines: List<String>, query: String, matchCase: Boolean, regex: Boolean): List<Match>? {
    if (query.isEmpty()) return emptyList()
    val pattern = if (regex) {
        try {
            if (matchCase) Regex(query) else Regex(query, RegexOption.IGNORE_CASE)
        } catch (e: IllegalArgumentException) {
            return null
        }
    } else {
        null
    }
    val out = ArrayList<Match>()
    for ((i, line) in lines.withIndex()) {
        if (pattern != null) {
            for (m in pattern.findAll(line)) {
                if (m.range.isEmpty()) continue
                out += Match(i, m.range.first, m.range.last + 1)
                if (out.size >= FIND_MAX) return out
            }
        } else {
            var from = 0
            while (true) {
                val at = line.indexOf(query, from, ignoreCase = !matchCase)
                if (at < 0) break
                out += Match(i, at, at + query.length)
                if (out.size >= FIND_MAX) return out
                from = at + query.length
            }
        }
    }
    return out
}

/** "3 of 12", "No results", "Invalid pattern", as the desktop's count says it. */
fun findCount(query: String, matches: List<Match>?, current: Int): String = when {
    query.isEmpty() -> ""
    matches == null -> "Invalid pattern"
    matches.isEmpty() -> "No results"
    else -> "${current + 1} of ${matches.size}${if (matches.size >= FIND_MAX) "+" else ""}"
}

/**
 * The find bar over an open file (find_bar.rs): the query with its count, Previous and Next,
 * Match case and Regular expression, and Close. It takes the keyboard when it opens.
 */
@Composable
fun FindBar(
    query: String,
    onQuery: (String) -> Unit,
    count: String,
    matchCase: Boolean,
    onCase: () -> Unit,
    regex: Boolean,
    onRegex: () -> Unit,
    canStep: Boolean,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
) {
    val c = Relay.colors
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(Modifier.fillMaxWidth().background(c.wall).padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Field(
                value = query,
                onValueChange = onQuery,
                modifier = Modifier.weight(1f).focusRequester(focus),
                placeholder = "Find in file",
                leading = "search",
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { onNext() }),
                trailing = {
                    if (count.isNotEmpty()) T(count, style = Relay.type.mono, color = if (count == "No results" || count == "Invalid pattern") c.heldText else c.ink3, maxLines = 1)
                },
            )
            IconKey("chevron-up", onPrevious, enabled = canStep, label = "Previous match")
            IconKey("chevron-down", onNext, enabled = canStep, label = "Next match")
            IconKey("close", onClose, label = "Close")
        }
        Row(Modifier.padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Pill("Aa", selected = matchCase, onClick = onCase)
            Pill(".*", selected = regex, onClick = onRegex)
        }
    }
}
