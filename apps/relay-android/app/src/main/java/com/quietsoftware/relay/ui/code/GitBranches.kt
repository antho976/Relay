package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Fill
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private sealed interface BranchSheet {
    class Actions(val branch: Branch) : BranchSheet
    class Switch(val branch: Branch) : BranchSheet
    class Carry(val name: String, val message: String) : BranchSheet
    class Delete(val branch: Branch) : BranchSheet
    class Create(val from: String?) : BranchSheet
    class Clean(val found: CleanOut) : BranchSheet
}

/** What `git.branch.clean_merged` deleted (or, on a dry run, would), and the merged ones it kept with why. */
@Serializable
private data class CleanOut(val deleted: List<String> = emptyList(), val failed: List<CleanFailed> = emptyList())

@Serializable
private data class CleanFailed(val branch: String = "", val reason: String = "")

private fun branches(n: Int) = if (n == 1) "1 merged branch" else "$n merged branches"

/**
 * The local branches of a worktree: the current one checked, its sync with the remote, which are
 * merged, and the session that owns each. Switch, branch from one, delete a merged one, or clean
 * every closed session's merged branch at once (`git.branch.clean_merged`, named before it goes).
 * Desktop: code_git.rs branch popover.
 */
@Composable
fun ColumnScope.BranchesTab(nav: Nav, projectId: Long, wt: String?, refresh: Int, online: Boolean, onChanged: () -> Unit) {
    val scope = rememberCoroutineScope()
    val live = rememberLive(nav, "git.branches", codePayload(projectId, wt), key = refresh)
    val data = live.result?.decodeAs<Branches>()
    var sheet by remember { mutableStateOf<BranchSheet?>(null) }

    // Uncommitted changes refuse a switch; asked, they come along as `git switch` brings them (code_git.rs).
    fun switchTo(name: String, carry: Boolean) {
        scope.launch {
            try {
                nav.relay.call("git.branch.switch", codePayload(projectId, wt) {
                    put("name", name)
                    if (carry) put("carry_changes", true)
                })
                onChanged()
                nav.shell.toast("On $name")
            } catch (e: BusException) {
                if (e.error.code == "git.checkout_dirty" && !carry) sheet = BranchSheet.Carry(name, e.error.message) else nav.shell.toast(refusal(e.error))
            }
        }
    }

    // A dry run first, so the confirmation names what goes; then the run itself.
    fun findMerged() {
        scope.write(nav, "git.branch.clean_merged", buildJsonObject {
            put("project_id", projectId)
            put("dry_run", true)
        }) { r ->
            val found = r?.decodeAs<CleanOut>() ?: return@write
            if (found.deleted.isEmpty()) {
                nav.shell.toast(if (found.failed.isEmpty()) "No merged branches to clean" else "No merged branch can go now: ${found.failed.joinToString("; ") { "${it.branch}, ${it.reason}" }}")
            } else {
                sheet = BranchSheet.Clean(found)
            }
        }
    }

    fun clean() {
        scope.write(nav, "git.branch.clean_merged", buildJsonObject { put("project_id", projectId) }) { r ->
            val done = r?.decodeAs<CleanOut>() ?: return@write
            onChanged()
            nav.shell.toast(
                "Deleted ${branches(done.deleted.size)}" + if (done.failed.isNotEmpty()) " · ${done.failed.size} kept" else "",
            )
        }
    }

    LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
        if (data == null) {
            item { LiveWait(live, page = true) }
        } else {
            if (!live.fresh) item { StaleNote(live.at) }
            item {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Fill()
                    Key("Clean merged", { findMerged() }, kind = KeyKind.Quiet, compact = true, enabled = online, glyph = "clear")
                    Key("New branch", { sheet = BranchSheet.Create(null) }, kind = KeyKind.Plain, compact = true, enabled = online, glyph = "branch")
                }
            }
        }
        items(data?.branches.orEmpty(), key = { it.name }) { branch ->
            BranchRow(branch, onClick = { sheet = BranchSheet.Actions(branch) })
        }
    }

    when (val s = sheet) {
        null -> Unit
        is BranchSheet.Actions -> ActionSheet(
            title = s.branch.name,
            subtitle = null,
            actions = buildList<SheetAction> {
                if (!s.branch.current) {
                    add(SheetAction("Switch to ${s.branch.name}", "branch", enabled = online) { sheet = BranchSheet.Switch(s.branch) })
                }
                add(SheetAction("New branch from ${s.branch.name}", "commit", enabled = online) { sheet = BranchSheet.Create(s.branch.name) })
                // Only a merged branch no session holds can go (code_git.rs); the engine refuses the rest.
                if (!s.branch.current && s.branch.merged && s.branch.session == null) {
                    add(SheetAction("Delete ${s.branch.name}", "trash", danger = true, enabled = online) { sheet = BranchSheet.Delete(s.branch) })
                }
            },
            onDismiss = { sheet = null },
        )
        is BranchSheet.Switch -> ConfirmSheet(
            title = "Switch to ${s.branch.name}?",
            body = "No live session may own this checkout.",
            confirm = "Switch",
            onConfirm = { switchTo(s.branch.name, carry = false) },
            onDismiss = { sheet = null },
        )
        is BranchSheet.Carry -> ConfirmSheet(
            title = "Uncommitted changes",
            body = "${s.message}\n\nBringing them along works like git switch: they stay uncommitted on the other branch, and git refuses if that branch changes the same files. Nothing is discarded.",
            confirm = "Bring changes to ${s.name}",
            onConfirm = { switchTo(s.name, carry = true) },
            onDismiss = { sheet = null },
        )
        is BranchSheet.Delete -> ConfirmSheet(
            title = "Delete ${s.branch.name}?",
            body = "Only a merged local branch that is not checked out and not owned by a session can be deleted.",
            confirm = "Delete",
            danger = true,
            onConfirm = {
                scope.write(nav, "git.branch.delete", buildJsonObject {
                    put("project_id", projectId)
                    put("name", s.branch.name)
                }) { r ->
                    if (r != null) {
                        onChanged()
                        nav.shell.toast("Deleted ${s.branch.name}")
                    }
                }
            },
            onDismiss = { sheet = null },
        )
        is BranchSheet.Clean -> ConfirmSheet(
            title = "Delete ${branches(s.found.deleted.size)}?",
            body = buildString {
                append(s.found.deleted.joinToString("\n"))
                append("\n\nEach is a closed session's branch whose work is merged, into the base branch or by a merged pull request; its worktree goes with it. Unmerged work is kept.")
                if (s.found.failed.isNotEmpty()) {
                    append("\n\nKept: ")
                    append(s.found.failed.joinToString("; ") { "${it.branch}, ${it.reason}" })
                }
            },
            confirm = "Delete",
            danger = true,
            onConfirm = { clean() },
            onDismiss = { sheet = null },
        )
        is BranchSheet.Create -> NameSheet(
            title = if (s.from == null) "New branch" else "New branch from ${s.from}",
            initial = "",
            confirm = "Create branch",
            onSubmit = { name ->
                sheet = null
                scope.write(nav, "git.branch.create", codePayload(projectId, wt) {
                    put("name", name)
                    put("checkout", true)
                    if (s.from != null) put("start_point", s.from)
                }) { r ->
                    if (r != null) {
                        onChanged()
                        nav.shell.toast("On $name")
                    }
                }
            },
            onDismiss = { sheet = null },
        )
    }
}

@Composable
private fun BranchRow(branch: Branch, onClick: () -> Unit) {
    val c = Relay.colors
    val sync = branch.upstream?.let { "↑${branch.ahead ?: 0} ↓${branch.behind ?: 0}" }
    val detail = listOfNotNull(
        sync,
        if (branch.merged) "merged" else null,
        branch.session?.let { "session $it" },
    ).joinToString(" · ")
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 12.dp, vertical = 9.dp)) {
        if (branch.current) Glyph("check", 16.dp, c.live) else Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            T(branch.name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            if (detail.isNotEmpty()) T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
    }
}
