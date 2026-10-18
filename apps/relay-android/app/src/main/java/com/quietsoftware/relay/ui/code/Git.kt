package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * A worktree's git: Changes (stage, diff, commit, push, pull request), History and Branches.
 * Fetch is in the bar. Named with a session, the screen works in that session's worktree.
 * Desktop: code_git.rs.
 */
@Composable
fun GitScreen(projectId: Long, worktree: String?, session: String?, nav: Nav) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val online = rememberOnline(nav)
    val sessionFlow = remember(session) { session?.let { nav.relay.session(it) } ?: flowOf<Session?>(null) }
    val sess by sessionFlow.collectAsStateWithLifecycle(initialValue = null)
    val project by nav.relay.project(projectId).collectAsStateWithLifecycle(initialValue = null)
    var picked by rememberSaveable { mutableStateOf(worktree) }
    var picking by remember { mutableStateOf(false) }
    var tab by rememberSaveable { mutableStateOf("changes") }
    var refresh by remember { mutableIntStateOf(0) }
    var fetching by remember { mutableStateOf(false) }
    val wt: String? = if (session != null) sess?.worktree?.ifEmpty { null } else picked
    val ready = session == null || wt != null
    val listed = rememberLive(nav, "worktree.list", codePayload(projectId, null)).result?.decodeAs<WorktreeList>()?.worktrees.orEmpty()
    val primary = primaryOf(listed, project?.path)
    val current = listed.firstOrNull { it.path == wt } ?: primary
    val status = rememberLive(nav, "git.status", if (ready) codePayload(projectId, wt) else null, key = refresh)
    val st = status.result?.decodeAs<GitStatus>()
    val sync = when {
        st == null -> null
        st.upstream == null -> "no upstream"
        else -> "↑${st.ahead ?: "?"} ↓${st.behind ?: "?"}"
    }

    fun fetch() {
        if (fetching) return
        fetching = true
        scope.write(nav, "git.fetch", buildJsonObject { put("project_id", projectId) }) { r ->
            fetching = false
            if (r == null) return@write
            val out = r.decodeAs<FetchOut>()
            val ahead = out?.ahead
            val behind = out?.behind
            nav.shell.toast(if (ahead == null || behind == null) "Fetched" else "Fetched · $ahead ahead · $behind behind")
            refresh++
        }
    }

    Page {
        Column(Modifier.fillMaxSize().navigationBarsPadding()) {
            PageBar(
                title = "Git",
                onBack = nav::back,
                subtitle = listOfNotNull(listOfNotNull(st?.branch?.ifBlank { "detached HEAD" }, sync).joinToString(" ").ifBlank { null }, "fetching…".takeIf { fetching })
                    .joinToString(" · ").ifBlank { null },
                actions = {
                    IconKey("download", { fetch() }, enabled = online && ready && !fetching, label = if (fetching) "Fetching" else "Fetch")
                },
            )
            if (session != null) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Pill("Session · $session", color = c.ink2)
                }
            } else {
                WorktreeKey(
                    current = current,
                    primary = current != null && current.path == primary?.path,
                    onClick = { picking = true },
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                    online = online,
                )
            }
            Segmented(
                options = listOf("changes" to "Changes", "history" to "History", "branches" to "Branches"),
                selected = tab,
                onSelect = { tab = it },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
                fill = true,
            )
            if (!ready) {
                Empty("branch", "No worktree for this session", "The session is not open on the connected PC, or it has no worktree yet.")
            } else {
                CodeBody {
                    val onChanged: () -> Unit = { refresh++ }
                    when (tab) {
                        "history" -> HistoryTab(nav, projectId, wt, refresh)
                        "branches" -> BranchesTab(nav, projectId, wt, refresh, online, onChanged)
                        else -> ChangesTab(nav, projectId, wt, status, refresh, online, onChanged)
                    }
                }
            }
        }
    }

    if (picking) {
        WorktreeSheet(
            worktrees = listed,
            primary = primary,
            selected = wt,
            onPick = { picked = it.path },
            onDismiss = { picking = false },
        )
    }
}
