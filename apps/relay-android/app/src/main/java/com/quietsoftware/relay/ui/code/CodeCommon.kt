package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.HGap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.security.MessageDigest

/**
 * A payload for an op on a project's worktree. The PC rejects unknown fields, so a key goes out
 * only when it is set; a null or empty [worktree] is the primary checkout.
 */
fun codePayload(projectId: Long, worktree: String?, extra: JsonObjectBuilder.() -> Unit = {}): JsonObject = buildJsonObject {
    put("project_id", projectId)
    if (!worktree.isNullOrEmpty()) put("worktree", worktree)
    extra()
}

/** A read the screen shows, through the replica (Relay.live). A null [payload] reads nothing yet. */
@Composable
fun rememberLive(nav: Nav, op: String, payload: JsonObject?, key: Any? = null): Live {
    val flow = remember(op, payload, key) { payload?.let { nav.relay.live(op, it) } ?: emptyFlow<Live>() }
    return flow.collectAsStateWithLifecycle(Live()).value
}

@Composable
fun rememberOnline(nav: Nav): Boolean = nav.shell.state.collectAsStateWithLifecycle().value.online

/** A write that needs the PC now. Refusals are said in a toast; [onDone] gets the answer, or null when it did not go through. */
fun CoroutineScope.write(nav: Nav, op: String, payload: JsonObject, onDone: (JsonElement?) -> Unit = {}) {
    launch {
        try {
            onDone(nav.relay.call(op, payload))
        } catch (e: BusException) {
            nav.shell.toast(refusal(e.error))
            onDone(null)
        }
    }
}

/** Brings a trashed entry back and says so. */
suspend fun restoreFromTrash(nav: Nav, projectId: Long, trashId: Long, name: String) {
    try {
        nav.relay.call("file.restore", buildJsonObject {
            put("project_id", projectId)
            put("trash_id", trashId)
        })
        nav.shell.toast("Restored $name")
    } catch (e: BusException) {
        nav.shell.toast(refusal(e.error))
    }
}

/** What a refused or failed write says to the person. */
fun refusal(e: BusError): String = when {
    e.code == "link.down" -> "Needs the PC, which is out of reach"
    e.kind == "held" -> "Held for approval on the PC. Decide it in the inbox."
    e.message.isNotBlank() -> e.hint?.let { "${e.message} ($it)" } ?: e.message
    else -> e.code
}

/** A bottom sheet in the desktop's panel style: a slab, 14dp top corners, a short grip. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SheetFrame(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    val c = Relay.colors
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = c.slab,
        shape = Radii.sheet,
        dragHandle = {
            Box(Modifier.padding(top = 10.dp, bottom = 8.dp).size(width = 36.dp, height = 4.dp).clip(CircleShape).background(c.track))
        },
    ) {
        Column(Modifier.fillMaxWidth().padding(start = 18.dp, end = 18.dp, bottom = 22.dp), content = content)
    }
}

/** One line of an action sheet: what it does, and whether it is offered now. */
class SheetAction(
    val label: String,
    val glyph: String,
    val danger: Boolean = false,
    val enabled: Boolean = true,
    val run: () -> Unit,
)

@Composable
fun ActionSheet(title: String, subtitle: String?, actions: List<SheetAction>, onDismiss: () -> Unit) {
    val c = Relay.colors
    SheetFrame(onDismiss) {
        T(title, style = Relay.type.title, color = c.ink, maxLines = 1)
        subtitle?.let { T(it, style = Relay.type.mono, color = c.ink3, maxLines = 1) }
        Gap(8.dp)
        for (a in actions) {
            val tint = when {
                a.danger -> c.heldText
                a.enabled -> c.ink2
                else -> c.ink3
            }
            ListRow(onClick = { if (a.enabled) { onDismiss(); a.run() } }) {
                Glyph(a.glyph, 18.dp, tint)
                T(a.label, Modifier.weight(1f), Relay.type.ui, if (a.danger) c.heldText else if (a.enabled) c.ink else c.ink3)
            }
        }
    }
}

@Composable
fun ConfirmSheet(title: String, body: String, confirm: String, danger: Boolean = false, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    SheetFrame(onDismiss) {
        T(title, style = Relay.type.title, color = c.ink)
        Gap(6.dp)
        T(body, style = Relay.type.ui, color = c.ink2)
        Gap(18.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            Key("Cancel", onDismiss, kind = KeyKind.Quiet)
            HGap(8.dp)
            Key(confirm, { onDismiss(); onConfirm() }, kind = if (danger) KeyKind.Danger else KeyKind.Primary)
        }
    }
}

/** A name typed into a sheet: a new file, a rename, a new branch. */
@Composable
fun NameSheet(title: String, initial: String, confirm: String, onSubmit: (String) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    var name by remember { mutableStateOf(initial) }
    val clean = name.trim()
    SheetFrame(onDismiss) {
        T(title, style = Relay.type.title, color = c.ink)
        Gap(12.dp)
        Field(
            value = name,
            onValueChange = { name = it },
            placeholder = "Name",
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (clean.isNotEmpty()) onSubmit(clean) }),
        )
        Gap(16.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            Key("Cancel", onDismiss, kind = KeyKind.Quiet)
            HGap(8.dp)
            Key(confirm, { onSubmit(clean) }, kind = KeyKind.Primary, enabled = clean.isNotEmpty())
        }
    }
}

/** The primary checkout: the first worktree git lists, or the one at the project's path. */
fun primaryOf(list: List<Worktree>, projectPath: String?): Worktree? {
    val p = projectPath?.trimEnd('/')
    return list.firstOrNull { !p.isNullOrEmpty() && it.path.trimEnd('/') == p } ?: list.firstOrNull()
}

/** The worktree a screen works in: the primary checkout, or an agent's own worktree. */
@Composable
fun WorktreeKey(current: Worktree?, primary: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, online: Boolean = true) {
    val c = Relay.colors
    ListRow(modifier = modifier, onClick = onClick, padding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)) {
        Glyph("branch", 16.dp, c.ink2)
        Column(Modifier.weight(1f)) {
            T(current?.branch?.ifBlank { "detached HEAD" } ?: "No worktree", style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            val where = when {
                current == null -> if (online) "Reading worktrees" else "Listed once the PC is in reach"
                primary -> "primary checkout"
                current.session != null -> "agent · ${current.session}"
                else -> "worktree"
            }
            T(where, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
        if (current?.dirty == true) Dot(c.waiting, 6.dp)
        Glyph("chevron-down", 14.dp, c.ink3)
    }
}

@Composable
fun WorktreeSheet(worktrees: List<Worktree>, primary: Worktree?, selected: String?, onPick: (Worktree) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    SheetFrame(onDismiss) {
        T("Worktree", style = Relay.type.title, color = c.ink)
        Gap(6.dp)
        if (worktrees.isEmpty()) T("No worktrees listed yet.", style = Relay.type.caption, color = c.ink3)
        for (w in worktrees) {
            val on = if (selected == null) w.path == primary?.path else w.path == selected
            ListRow(selected = on, onClick = { onDismiss(); onPick(w) }) {
                Column(Modifier.weight(1f)) {
                    T(w.branch.ifBlank { "detached HEAD" }, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                    T(w.path, style = Relay.type.mono, color = c.ink3, maxLines = 1)
                }
                if (w.dirty) Dot(c.waiting, 6.dp)
                w.session?.let { Pill(it, color = c.ink2) }
                if (on) Glyph("check", 16.dp, c.live)
            }
        }
    }
}

/** The file-type glyph and tint, as the desktop draws them (project_files.rs, git_files.css). */
fun fileGlyph(name: String): Pair<String, Color> {
    val lower = name.lowercase()
    if (lower in LOCK_NAMES) return "file-lock" to Color(0xFF8D9097)
    if (lower in GIT_NAMES) return "file-git" to Color(0xFFE2694A)
    if (lower in SHELL_NAMES) return "file-shell" to Color(0xFF8EC46C)
    return when (lower.substringAfterLast('.', "")) {
        "rs" -> "file-rust" to Color(0xFFDE8C5C)
        "ts", "mts", "cts" -> "file-ts" to Color(0xFF4F9AD6)
        "tsx", "jsx" -> "file-react" to Color(0xFF5CC4DC)
        "js", "mjs", "cjs" -> "file-js" to Color(0xFFDCC157)
        "json", "jsonc", "json5", "jsonl" -> "file-json" to Color(0xFFC9A24C)
        "md", "mdx", "markdown" -> "file-md" to Color(0xFF8EAED0)
        "toml", "yaml", "yml", "ini", "cfg", "conf", "env", "properties" -> "file-config" to Color(0xFFA597CB)
        "css", "scss", "sass", "less" -> "file-css" to Color(0xFF5FB3C9)
        "html", "htm", "xml", "svelte", "vue" -> "file-html" to Color(0xFFE07A5A)
        "py", "pyi" -> "file-py" to Color(0xFF6FA3DC)
        "sh", "bash", "zsh", "fish", "ps1", "bat", "nu" -> "file-shell" to Color(0xFF8EC46C)
        "lock" -> "file-lock" to Color(0xFF8D9097)
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg" -> "file-image" to Color(0xFFBB8FD9)
        else -> "file-text" to Color(0xFFA5A5A3)
    }
}

val FOLDER_TINT = Color(0xFFC8A96A)

private val LOCK_NAMES = setOf(
    "cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lockb",
    "flake.lock", "poetry.lock", "composer.lock", "gemfile.lock", "uv.lock",
)
private val GIT_NAMES = setOf(".gitignore", ".gitattributes", ".gitmodules", ".gitkeep", ".ignore")
private val SHELL_NAMES = setOf("dockerfile", "makefile", "justfile", "containerfile")

/** The colour of a git status letter (git_files.css). */
fun statusTint(letter: String, c: Palette): Color = when (letter) {
    "M" -> Palette.GIT_MODIFIED
    "A", "?" -> Palette.GIT_ADDED
    "D" -> Palette.GIT_DELETED
    "R", "C" -> Palette.GIT_RENAMED
    "U" -> c.heldText
    else -> c.ink3
}

/** The SHA-256 of a text as the PC computes it for `file.write`'s expected_sha256. */
fun sha256Hex(text: String): String =
    MessageDigest.getInstance("SHA-256").digest(text.toByteArray(Charsets.UTF_8)).joinToString("") { "%02x".format(it) }

fun sizeText(bytes: Long): String = when {
    bytes < 1024 -> "$bytes B"
    bytes < (1 shl 20) -> "${bytes / 1024} KB"
    else -> "%.1f MB".format(bytes / 1048576.0)
}

/** The advance of one monospace cell at [style], in pixels, measured rather than guessed. */
@Composable
fun monoCellPx(style: TextStyle): Float {
    val measurer = rememberTextMeasurer()
    return remember(style) { measurer.measure(AnnotatedString("0000000000"), style).size.width / 10f }
}

/** The body under a page's bar: centred, at most a readable column wide. */
@Composable
fun ColumnScope.CodeBody(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.weight(1f).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Column(Modifier.fillMaxSize().column(), content = content)
    }
}

/** A line in the body that says what the screen is waiting for, or why it has nothing. */
@Composable
fun LiveWait(live: Live, reading: String = "Reading…", page: Boolean = false) {
    val c = Relay.colors
    if (live.result != null) return
    // A whole page never read and the PC away: said as Tally and Arbiter say it, not as a footnote.
    if (page && live.error?.code == "link.down") return NotReadYet()
    val text = live.error?.let { refusal(it) } ?: reading
    T(text, Modifier.padding(horizontal = 16.dp, vertical = 12.dp), Relay.type.caption, c.ink3)
}

/** A page this phone has never read, with the PC out of reach. */
@Composable
fun NotReadYet() = Empty("cloud", "Not on this phone yet", "The PC is out of reach. This shows here once it has been read with the PC in reach.")

