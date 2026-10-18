package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.HoldCard
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** Two sessions changing the same file or symbol (`overlap.list`). */
@Serializable
private data class OverlapRow(
    val id: Long,
    val sessions: List<String> = emptyList(),
    val path: String = "",
    val symbol: String? = null,
    val kind: String = "file",
    @SerialName("acked_by") val ackedBy: List<String> = emptyList(),
    @SerialName("first_seen") val firstSeen: String = "",
)

/**
 * A project's guardrails: holds waiting for a decision, every earlier hold with how it ended, and
 * the files two sessions both changed or claimed, with a note to each of them.
 */
@Composable
fun GuardrailsScreen(projectId: Long?, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val pid = projectId ?: s.project?.num
    val projectName = s.projects.firstOrNull { it.num == pid }?.name
    val holds by remember { nav.relay.holds() }.collectAsStateWithLifecycle(initialValue = emptyList())
    val history = remember(pid) {
        nav.relay.live("guardrail.holds.list", buildJsonObject { if (pid != null) put("project_id", pid); put("open_only", false) })
    }
    val historyLive by history.collectAsStateWithLifecycle(initialValue = Live())
    val overlapFlow = remember(pid) {
        if (pid == null) flowOf(Live()) else nav.relay.live("overlap.list", buildJsonObject { put("project_id", pid) })
    }
    val overlapLive by overlapFlow.collectAsStateWithLifecycle(initialValue = Live())

    val open = holds.filter { pid == null || it.projectId == null || it.projectId == pid }
    val past = (historyLive.result as? JsonObject)?.arr("holds")?.mapNotNull { it.decodeAs<Hold>() }.orEmpty().filter { it.state != "open" }
    val overlaps = (overlapLive.result as? JsonObject)?.arr("overlaps")?.mapNotNull { it.decodeAs<OverlapRow>() }.orEmpty()
    var tell by remember { mutableStateOf<OverlapRow?>(null) }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Guardrails", nav::back, subtitle = projectName)
            ScrollBody {
                SectionLabel(if (open.isEmpty()) "Waiting for you" else "Waiting for you · ${open.size}")
                if (open.isEmpty()) {
                    Empty("shield", "Nothing waiting", "No agent in this project is waiting on a guardrail.")
                } else {
                    open.forEach { hold ->
                        HoldCard(
                            hold,
                            more = 0,
                            model = nav.shell,
                            onLater = null,
                            onOpen = null,
                            modifier = Modifier.padding(bottom = 8.dp),
                        )
                    }
                }

                SectionLabel("History")
                if (historyLive.result == null) {
                    T(historyLive.error?.message ?: "Loading…", style = Relay.type.caption, color = c.ink3)
                } else if (past.isEmpty()) {
                    T("No hold has ended yet.", style = Relay.type.caption, color = c.ink3)
                } else {
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                        past.forEachIndexed { index, h ->
                            if (index > 0) Hairline()
                            HistoryRow(h)
                        }
                    }
                }
                if (!historyLive.fresh && historyLive.result != null) StaleNote(historyLive.at)

                SectionLabel("Shared file activity")
                when {
                    pid == null -> T("Open a project to see the files its sessions share.", style = Relay.type.caption, color = c.ink3)
                    overlapLive.result == null -> T(overlapLive.error?.message ?: "Loading…", style = Relay.type.caption, color = c.ink3)
                    overlaps.isEmpty() -> Empty("merge", "No overlap", "No two sessions are changing the same files right now.")
                    else -> Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                        overlaps.forEachIndexed { index, o ->
                            if (index > 0) Hairline()
                            OverlapRowView(o, onTell = { tell = o })
                        }
                    }
                }
                if (!overlapLive.fresh && overlapLive.result != null) StaleNote(overlapLive.at)
            }
        }
    }

    tell?.let { o ->
        val subject = o.symbol?.let { "$it in ${o.path}" } ?: o.path
        Ask(
            title = "Tell these sessions?",
            body = "Sends each of ${o.sessions.joinToString(", ")} a message that the others are changing $subject too.",
            onDismiss = { tell = null },
        ) {
            Key("Cancel", { tell = null }, kind = KeyKind.Quiet, compact = true)
            Key(
                "Send",
                {
                    val p = pid
                    if (p != null) {
                        val verb = if (o.sessions.size > 2) "are" else "is"
                        for (session in o.sessions) {
                            val others = o.sessions.filter { it != session }.joinToString(", ")
                            nav.shell.change(
                                "mailbox.send",
                                buildJsonObject {
                                    put("project_id", p)
                                    put("to", session)
                                    put("text", "Heads up: $others $verb also changing $subject. Coordinate before you commit.")
                                },
                                "Tell $session",
                            )
                        }
                    }
                    tell = null
                },
                kind = KeyKind.Primary,
                compact = true,
            )
        }
    }
}

@Composable
private fun HistoryRow(h: Hold) {
    val c = Relay.colors
    Row(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(h.op, style = Relay.type.mono, color = c.ink, maxLines = 1)
            T(
                listOf(h.session ?: h.actor, h.policy, ago(epoch(h.createdAt))).filter { it.isNotBlank() }.joinToString(" · "),
                style = Relay.type.caption,
                color = c.ink3,
                maxLines = 2,
            )
        }
        Spacer(Modifier.width(10.dp))
        Pill(
            holdLabel(h.state),
            color = when (h.state) {
                "confirmed" -> c.live
                "rejected" -> c.heldText
                else -> c.ink3
            },
        )
    }
}

@Composable
private fun OverlapRowView(o: OverlapRow, onTell: () -> Unit) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            T(
                o.symbol?.let { "$it · ${o.path}" } ?: o.path,
                Modifier.weight(1f),
                Relay.type.mono,
                c.ink,
                maxLines = 2,
            )
            Spacer(Modifier.width(8.dp))
            Pill(o.kind)
        }
        T(
            "${o.sessions.joinToString(" ↔ ")} · since ${ago(epoch(o.firstSeen))}",
            style = Relay.type.caption,
            color = c.ink3,
            maxLines = 2,
        )
        if (o.ackedBy.isNotEmpty()) T("Seen by ${o.ackedBy.joinToString(", ")}", style = Relay.type.caption, color = c.ink3, maxLines = 2)
        Gap(4.dp)
        Key("Tell them", onTell, glyph = "mail", kind = KeyKind.Quiet, compact = true, enabled = o.sessions.size > 1)
    }
}

private fun holdLabel(state: String) = when (state) {
    "open" -> "Waiting"
    "confirmed" -> "Allowed"
    "rejected" -> "Denied"
    "expired" -> "Expired"
    else -> state
}
