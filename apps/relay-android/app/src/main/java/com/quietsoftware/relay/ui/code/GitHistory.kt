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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private const val LOG_PAGE = 50
private const val LOG_MAX = 500

/** The branch's commits, newest first, 50 at a time up to 500. Desktop: code_git.rs HISTORY. */
@Composable
fun ColumnScope.HistoryTab(nav: Nav, projectId: Long, wt: String?, refresh: Int) {
    var limit by rememberSaveable { mutableStateOf(LOG_PAGE) }
    var shown by remember { mutableStateOf<String?>(null) }
    val log = rememberLive(nav, "git.log", codePayload(projectId, wt) { put("limit", limit) }, key = refresh)
    val commits = log.result?.decodeAs<CommitLog>()?.commits.orEmpty()
    LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 24.dp)) {
        if (log.result == null) {
            item { LiveWait(log, page = true) }
        } else if (!log.fresh) {
            item { StaleNote(log.at) }
        }
        items(commits, key = { it.sha }) { commit ->
            CommitRow(commit, onClick = { shown = commit.sha })
        }
        if (log.result != null && commits.size >= limit && limit < LOG_MAX) {
            item {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), horizontalArrangement = Arrangement.Center) {
                    Key("Show $LOG_PAGE more", { limit = minOf(LOG_MAX, limit + LOG_PAGE) }, kind = KeyKind.Quiet)
                }
            }
        }
    }
    shown?.let { sha ->
        CommitDetailSheet(nav, projectId, sha, onDismiss = { shown = null })
    }
}

@Composable
private fun CommitRow(commit: Commit, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 12.dp, vertical = 10.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            T(commit.subject.ifBlank { "(no message)" }, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                T(commit.sha.take(7), style = Relay.type.mono, color = Palette.SHA, maxLines = 1)
                T(commit.author, Modifier.weight(1f, fill = false), Relay.type.caption, c.ink3, maxLines = 1)
                T(ago(epoch(commit.at)), style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
        }
    }
}

/** One commit: its message, and the files it touched with their counts. */
@Composable
private fun CommitDetailSheet(nav: Nav, projectId: Long, sha: String, onDismiss: () -> Unit) {
    val c = Relay.colors
    val live = rememberLive(nav, "git.show", buildJsonObject {
        put("project_id", projectId)
        put("sha", sha)
    })
    val show = live.result?.decodeAs<CommitShow>()
    val commit = show?.commit
    SheetFrame(onDismiss) {
        Column(Modifier.fillMaxWidth().heightIn(max = 560.dp).verticalScroll(rememberScrollState())) {
            T(commit?.subject?.ifBlank { null } ?: sha.take(7), style = Relay.type.title, color = c.ink)
            Gap(4.dp)
            val facts = listOfNotNull(
                commit?.sha?.take(12),
                commit?.author?.ifBlank { null },
                commit?.at?.let { ago(epoch(it)) },
            )
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                T(facts.firstOrNull().orEmpty(), style = Relay.type.mono, color = Palette.SHA, maxLines = 1)
                T(facts.drop(1).joinToString(" · "), style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
            commit?.body?.takeIf { it.isNotBlank() }?.let { body ->
                Gap(10.dp)
                T(body, style = Relay.type.ui, color = c.ink2)
            }
            commit?.refs?.takeIf { it.isNotEmpty() }?.let { refs ->
                Gap(10.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (ref in refs.take(4)) Pill(ref, color = c.ink2)
                }
            }
            if (show == null) {
                LiveWait(live)
            } else {
                SectionLabel("Files · ${show.files.size}")
                for (file in show.files) ShowFileRow(file)
            }
        }
    }
}

@Composable
private fun ShowFileRow(file: DiffFile) {
    val c = Relay.colors
    val name = file.path.substringAfterLast('/')
    val dir = file.path.substringBeforeLast('/', "")
    val (glyph, tint) = fileGlyph(name)
    ListRow(padding = PaddingValues(horizontal = 0.dp, vertical = 6.dp)) {
        Glyph(glyph, 16.dp, tint)
        Column(Modifier.weight(1f)) {
            T(name, style = Relay.type.ui, color = c.ink, maxLines = 1)
            if (dir.isNotEmpty()) T(dir, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
        if (file.binary) {
            T("binary", style = Relay.type.mono, color = c.ink3)
        } else {
            T("+${file.added}", style = Relay.type.mono, color = Palette.GIT_ADDED)
            T("-${file.removed}", style = Relay.type.mono, color = Palette.GIT_DELETED)
        }
        if (file.status.isNotBlank()) T(file.status, style = Relay.type.mono, color = statusTint(file.status, c))
    }
}
