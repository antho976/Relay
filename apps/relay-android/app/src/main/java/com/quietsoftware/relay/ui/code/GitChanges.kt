package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewModelScope
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.HGap
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray

private sealed interface ChangesSheet {
    data object Commit : ChangesSheet
    data object Push : ChangesSheet
    data object OpenPr : ChangesSheet
}

/**
 * The working tree's changes, staged and not, each file's hunks on a tap, and the publish card:
 * commit, push or publish, and the pull request for the branch. Desktop: code_git.rs.
 */
@Composable
fun ColumnScope.ChangesTab(nav: Nav, projectId: Long, wt: String?, status: Live, refresh: Int, online: Boolean, onChanged: () -> Unit) {
    val c = Relay.colors
    val uri = LocalUriHandler.current
    val scope = rememberCoroutineScope()
    val st = status.result?.decodeAs<GitStatus>()
    val files = st?.files.orEmpty()
    val staged = files.filter { it.index.trim().isNotEmpty() && it.index != "?" }
    val unstaged = files.filter { it.worktree.trim().isNotEmpty() }
    val work = rememberLive(nav, "git.diff", codePayload(projectId, wt), key = refresh)
    val index = rememberLive(nav, "git.diff", codePayload(projectId, wt) { put("staged", true) }, key = refresh)
    val pr = rememberLive(nav, "git.pr.list", buildJsonObject { put("project_id", projectId) }, key = refresh)
    val workCounts = work.result?.decodeAs<DiffFiles>()?.files.orEmpty().associateBy { it.path }
    val indexCounts = index.result?.decodeAs<DiffFiles>()?.files.orEmpty().associateBy { it.path }
    val branch = st?.branch.orEmpty()
    val mine = pr.result?.decodeAs<PrList>()?.pullRequests.orEmpty()
        .filter { it.sameRepository && branch.isNotEmpty() && it.branch == branch }
    val onPr = mine.firstOrNull { it.state == "open" } ?: mine.firstOrNull()
    var openKey by remember(wt) { mutableStateOf<String?>(null) }
    var sheet by remember { mutableStateOf<ChangesSheet?>(null) }

    fun stage(paths: List<String>, stageThem: Boolean) {
        if (paths.isEmpty()) return
        scope.write(nav, if (stageThem) "git.stage" else "git.unstage", codePayload(projectId, wt) {
            putJsonArray("paths") { paths.forEach { add(JsonPrimitive(it)) } }
        }) { r ->
            if (r != null) onChanged()
        }
    }

    LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
        if (st == null) {
            item { LiveWait(status, page = true) }
        } else {
            if (!status.fresh) item { StaleNote(status.at) }
            if (files.isNotEmpty()) item {
                val added = files.sumOf { (indexCounts[it.path]?.added ?: 0) + (workCounts[it.path]?.added ?: 0) }
                val removed = files.sumOf { (indexCounts[it.path]?.removed ?: 0) + (workCounts[it.path]?.removed ?: 0) }
                Summary(added, removed, files.size, branch)
            }
            if (files.isEmpty()) {
                item { Empty("check", "No changes", "Nothing changed in this worktree.") }
            }
        }
        if (staged.isNotEmpty()) {
            item {
                SectionLabel("In the next commit · ${staged.size}", Modifier.padding(horizontal = 14.dp)) {
                    Key("Leave all out", { stage(staged.map { it.path }, false) }, kind = KeyKind.Quiet, compact = true, enabled = online)
                }
            }
            item(key = "staged") {
                FileCard {
                    for (file in staged) {
                        val key = "s:" + file.path
                        ChangeRow(
                            nav = nav,
                            projectId = projectId,
                            wt = wt,
                            refresh = refresh,
                            file = file,
                            staged = true,
                            count = indexCounts[file.path],
                            expanded = openKey == key,
                            online = online,
                            onToggle = { openKey = if (openKey == key) null else key },
                            onStage = { stage(listOf(file.path), false) },
                        )
                    }
                }
            }
        }
        if (unstaged.isNotEmpty()) {
            item {
                SectionLabel((if (staged.isEmpty()) "Changed files" else "Not in the commit") + " · ${unstaged.size}", Modifier.padding(horizontal = 14.dp)) {
                    Key("Include all", { stage(unstaged.map { it.path }, true) }, kind = KeyKind.Quiet, compact = true, enabled = online)
                }
            }
            item(key = "unstaged") {
                FileCard {
                    for (file in unstaged) {
                        val key = "w:" + file.path
                        ChangeRow(
                            nav = nav,
                            projectId = projectId,
                            wt = wt,
                            refresh = refresh,
                            file = file,
                            staged = false,
                            count = workCounts[file.path],
                            expanded = openKey == key,
                            online = online,
                            onToggle = { openKey = if (openKey == key) null else key },
                            onStage = { stage(listOf(file.path), true) },
                        )
                    }
                }
            }
        }
        if (st != null) {
            item {
                PublishCard(
                    st = st,
                    fileCount = files.size,
                    stagedCount = staged.size,
                    online = online,
                    pr = onPr,
                    prState = pr,
                    onCommit = { sheet = ChangesSheet.Commit },
                    onPush = { sheet = ChangesSheet.Push },
                    onOpenPr = { sheet = ChangesSheet.OpenPr },
                    onViewPr = { url -> runCatching { uri.openUri(url) } },
                )
            }
        }
    }

    when (sheet) {
        null -> Unit
        ChangesSheet.Commit -> CommitSheet(
            nav = nav,
            projectId = projectId,
            wt = wt,
            stagedCount = staged.size,
            changedCount = files.size,
            online = online,
            onDone = {
                sheet = null
                onChanged()
            },
            onDismiss = { sheet = null },
        )
        ChangesSheet.Push -> if (st != null) {
            val publish = st.upstream == null
            ConfirmSheet(
                title = if (publish) "Publish this branch?" else "Push this branch?",
                body = pushBody(st, branch),
                confirm = if (publish) "Publish" else "Push",
                onConfirm = {
                    scope.write(nav, "git.push", codePayload(projectId, wt) { if (publish) put("set_upstream", true) }) { r ->
                        if (r != null) {
                            onChanged()
                            nav.shell.toast(if (publish) "Published $branch" else "Pushed $branch")
                        }
                    }
                },
                onDismiss = { sheet = null },
            )
        }
        ChangesSheet.OpenPr -> OpenPrSheet(
            nav = nav,
            projectId = projectId,
            wt = wt,
            branch = branch,
            online = online,
            onOpened = { url ->
                sheet = null
                runCatching { uri.openUri(url) }
                onChanged()
            },
            onDismiss = { sheet = null },
        )
    }
}

private fun pushBody(st: GitStatus, branch: String): String {
    val upstream = st.upstream ?: return "Push $branch to origin and set it as the upstream."
    val ahead = st.ahead ?: 0
    val behind = st.behind ?: 0
    val commits = "$ahead commit${if (ahead == 1) "" else "s"}"
    val warning = if (behind > 0) " The remote is $behind ahead; the push will likely be refused until you pull on the PC." else ""
    return "Push $commits on $branch to $upstream.$warning"
}

/** The change at a glance (design 14, "Summary"): lines added and taken out, across how many files, on which branch. */
@Composable
private fun Summary(added: Int, removed: Int, files: Int, branch: String) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), padding = PaddingValues(16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            T("+$added", style = Relay.type.code.copy(fontSize = 17.sp), color = Palette.GIT_ADDED, weight = FontWeight.Medium)
            T("−$removed", style = Relay.type.code.copy(fontSize = 17.sp), color = Palette.GIT_DELETED, weight = FontWeight.Medium)
        }
        T(
            (if (files == 1) "1 file changed" else "$files files changed") + branch.takeIf { it.isNotEmpty() && it != "HEAD" }?.let { " on $it" }.orEmpty(),
            Modifier.padding(top = 4.dp), Relay.type.ui, c.ink2,
        )
    }
}

/** A group of files on one card; a file opens to its diff in place. */
@Composable
private fun FileCard(content: @Composable ColumnScope.() -> Unit) {
    Slab(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 2.dp), padding = PaddingValues(vertical = 4.dp), content = content)
}

@Composable
private fun PublishCard(
    st: GitStatus,
    fileCount: Int,
    stagedCount: Int,
    online: Boolean,
    pr: PullRequest?,
    prState: Live,
    onCommit: () -> Unit,
    onPush: () -> Unit,
    onOpenPr: () -> Unit,
    onViewPr: (String) -> Unit,
) {
    val upstream = st.upstream
    val ahead = st.ahead ?: 0
    val behind = st.behind ?: 0
    // git.status names a detached head "HEAD": no branch to publish (code_git.rs does the same).
    val detached = st.branch.isEmpty() || st.branch == "HEAD"
    val canPush = !detached && online && (upstream == null || (ahead > 0 && behind == 0))
    // The desktop's words (code_git.rs): what the key would do, else why it does nothing.
    val pushLabel = when {
        detached -> "No branch"
        upstream == null -> "Publish"
        ahead > 0 && behind == 0 -> "Push $ahead"
        behind > 0 -> "Pull first"
        st.ahead == null || st.behind == null -> "Push"
        else -> "Synced"
    }
    val pushDetail = when {
        detached -> "A detached checkout has no branch to push"
        upstream == null -> "This branch is not on a remote yet"
        st.ahead == null || st.behind == null -> "$upstream · fetch to compare"
        ahead == 0 && behind == 0 -> "Up to date with $upstream"
        else -> "$ahead to push · $behind to pull · $upstream"
    }
    val prDetail = pr?.let { "#${it.number} · ${if (it.draft && it.state == "open") "draft" else it.state} · ${it.title}" }
        ?: when {
            prState.error != null -> "GitHub unavailable · PR status unknown"
            prState.result == null -> "Looking up pull requests…"
            upstream == null -> "No PR · push the branch first"
            else -> "No PR for this branch"
        }
    SectionLabel("Save and share", Modifier.padding(horizontal = 14.dp))
    Slab(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 2.dp), padding = PaddingValues(vertical = 4.dp)) {
        PublishRow(
            glyph = "commit",
            label = "Commit…",
            detail = if (fileCount == 0) "Nothing to commit" else "$stagedCount of $fileCount files included · message written for you",
            enabled = fileCount > 0 && online,
            onClick = onCommit,
        )
        PublishRow(
            glyph = "send",
            label = pushLabel,
            detail = pushDetail,
            enabled = canPush,
            onClick = onPush,
        )
        PublishRow(
            glyph = "merge",
            label = if (pr != null) "View pull request" else "Open pull request…",
            detail = prDetail,
            enabled = pr != null || (upstream != null && online),
            onClick = { if (pr != null) onViewPr(pr.url) else onOpenPr() },
        )
    }
}

@Composable
private fun PublishRow(glyph: String, label: String, detail: String, enabled: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(
        modifier = Modifier.alpha(if (enabled) 1f else .4f),
        onClick = if (enabled) onClick else null,
        padding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Glyph(glyph, 18.dp, c.ink2)
        Column(Modifier.weight(1f)) {
            T(label, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
    }
}

@Composable
private fun ChangeRow(
    nav: Nav,
    projectId: Long,
    wt: String?,
    refresh: Int,
    file: FileStatus,
    staged: Boolean,
    count: DiffFile?,
    expanded: Boolean,
    online: Boolean,
    onToggle: () -> Unit,
    onStage: () -> Unit,
) {
    val c = Relay.colors
    val name = file.path.substringAfterLast('/')
    val dir = file.path.substringBeforeLast('/', "")
    val letter = (if (staged) file.index else file.worktree).trim()
    val untracked = !staged && file.worktree == "?"
    val (glyph, tint) = fileGlyph(name)
    Column {
        ListRow(onClick = onToggle, padding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)) {
            Glyph(glyph, 16.dp, tint)
            Column(Modifier.weight(1f)) {
                T(name, style = Relay.type.ui, color = c.ink, maxLines = 1)
                if (dir.isNotEmpty()) T(dir, style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
            if (count?.binary == true) {
                T("binary", style = Relay.type.mono, color = c.ink3)
            } else if (count != null) {
                T("+${count.added}", style = Relay.type.mono, color = Palette.GIT_ADDED)
                T("-${count.removed}", style = Relay.type.mono, color = Palette.GIT_DELETED)
            } else if (untracked) {
                T("new", style = Relay.type.caption, color = c.ink3)
            }
            if (letter.isNotEmpty()) T(letter, style = Relay.type.mono, color = statusTint(letter, c), weight = FontWeight.Medium)
            IconKey(if (staged) "minus" else "plus", onStage, size = 16.dp, enabled = online, label = if (staged) "Leave out of the commit" else "Include in the commit")
        }
        if (expanded) DiffHunks(nav, projectId, wt, refresh, file.path, file.renamedFrom, staged, count?.binary == true)
    }
}

@Composable
private fun DiffHunks(nav: Nav, projectId: Long, wt: String?, refresh: Int, path: String, oldPath: String?, staged: Boolean, binary: Boolean) {
    val c = Relay.colors
    val live = rememberLive(nav, "git.diff.file", if (binary) null else codePayload(projectId, wt) {
        put("path", path)
        // A rename's old side is its source; without it the whole file reads as added.
        if (!oldPath.isNullOrEmpty()) put("old_path", oldPath)
        if (staged) put("staged", true)
    }, key = refresh)
    val hunks = live.result?.decodeAs<FileDiff>()?.hunks
    when {
        binary -> T("Binary file. No text diff.", Modifier.padding(horizontal = 16.dp, vertical = 8.dp), Relay.type.caption, c.ink3)
        hunks == null -> LiveWait(live)
        hunks.isEmpty() -> T("No textual changes.", Modifier.padding(horizontal = 16.dp, vertical = 8.dp), Relay.type.caption, c.ink3)
        else -> DiffBlock(hunks)
    }
}

@Composable
private fun CommitSheet(
    nav: Nav,
    projectId: Long,
    wt: String?,
    stagedCount: Int,
    changedCount: Int,
    online: Boolean,
    onDone: () -> Unit,
    onDismiss: () -> Unit,
) {
    val c = Relay.colors
    val suggest = rememberLive(nav, "git.suggest_message", codePayload(projectId, wt))
    val suggested = suggest.result?.decodeAs<Suggestion>()?.message
    var message by remember { mutableStateOf("") }
    var touched by remember { mutableStateOf(false) }
    var all by remember { mutableStateOf(stagedCount == 0) }
    var busy by remember { mutableStateOf(false) }
    LaunchedEffect(suggested) {
        if (!touched && !suggested.isNullOrBlank()) message = suggested
    }
    val nothing = if (all) changedCount == 0 else stagedCount == 0
    val canCommit = message.isNotBlank() && !nothing && online && !busy
    SheetFrame(onDismiss) {
        T("Commit", style = Relay.type.title, color = c.ink)
        Gap(10.dp)
        Field(
            value = message,
            onValueChange = {
                message = it
                touched = true
            },
            placeholder = "Commit message",
            singleLine = false,
            minLines = 3,
            maxLines = 8,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
        )
        Gap(12.dp)
        Row(Modifier.fillMaxWidth().heightIn(min = TOUCH).switchRow(all, enabled = online) { all = it }, verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                T("Stage everything", style = Relay.type.uiMedium, color = c.ink)
                T("Commit every change, not only what is staged.", style = Relay.type.caption, color = c.ink3)
            }
            Toggle(all, null, enabled = online)
        }
        Gap(16.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            Key("Cancel", onDismiss, kind = KeyKind.Quiet)
            HGap(8.dp)
            Key(
                if (busy) "Committing…" else "Commit",
                {
                    busy = true
                    // On the app's scope: a sheet swiped away mid-commit still says how it went.
                    nav.shell.viewModelScope.write(nav, "git.commit", codePayload(projectId, wt) {
                        put("message", message.trim())
                        put("all", all)
                    }) { r ->
                        busy = false
                        if (r != null) {
                            onDone()
                            nav.shell.toast("Committed " + (r.decodeAs<CommitOut>()?.sha ?: "").take(7))
                        }
                    }
                },
                kind = KeyKind.Primary,
                enabled = canCommit,
            )
        }
    }
}

@Composable
private fun OpenPrSheet(
    nav: Nav,
    projectId: Long,
    wt: String?,
    branch: String,
    online: Boolean,
    onOpened: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val c = Relay.colors
    var title by remember { mutableStateOf("") }
    var body by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    SheetFrame(onDismiss) {
        T("Open a pull request", style = Relay.type.title, color = c.ink)
        Gap(6.dp)
        T(
            "The GitHub CLI on the PC opens a pull request for $branch. An empty title and body are filled from the commits.",
            style = Relay.type.caption,
            color = c.ink3,
        )
        Gap(12.dp)
        Field(value = title, onValueChange = { title = it }, placeholder = "Title", keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
        Gap(8.dp)
        Field(value = body, onValueChange = { body = it }, placeholder = "Body", singleLine = false, minLines = 4, maxLines = 10, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
        Gap(16.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            Key("Cancel", onDismiss, kind = KeyKind.Quiet)
            HGap(8.dp)
            Key(
                if (busy) "Opening…" else "Open pull request",
                {
                    busy = true
                    // On the app's scope: a sheet swiped away meanwhile still opens the pull request it asked for.
                    nav.shell.viewModelScope.write(nav, "git.pr.open", codePayload(projectId, wt) {
                        if (title.isNotBlank()) put("title", title.trim())
                        if (body.isNotBlank()) put("body", body.trim())
                    }) { r ->
                        busy = false
                        val url = r?.decodeAs<PrOpened>()?.url
                        if (!url.isNullOrEmpty()) onOpened(url)
                    }
                },
                kind = KeyKind.Primary,
                enabled = online && !busy,
            )
        }
    }
}
