package com.quietsoftware.relay.ui.pages

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
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.size
import com.quietsoftware.relay.ui.dev.AgentBadge
import com.quietsoftware.relay.ui.dev.agentTitle
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Eyebrow
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Notification
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.HoldCard
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * What needs the person: guardrail holds first (an agent stops until someone answers), tasks
 * waiting for review, then the notification feed. Tapping a notification marks it read on the
 * PC and opens what it is about.
 */
@Composable
fun InboxScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // Null until the phone's copy answers, so "You're all caught up" never shows before what is there.
    val notified by remember { nav.relay.notifications() }.collectAsStateWithLifecycle(initialValue = null)
    val listed by remember { nav.relay.allTasks() }.collectAsStateWithLifecycle(initialValue = null)
    val projects by remember { nav.relay.projects() }.collectAsStateWithLifecycle(initialValue = emptyList())
    val notifications = notified.orEmpty()

    val names = projects.associate { it.num to it.name }
    // By project, so each row opens the board its tasks are on.
    val review = listed.orEmpty().filter { it.column == "in_review" }.groupBy { it.projectId }
    val unread = notifications.count { !it.read }

    fun open(n: Notification) {
        if (!n.read) nav.shell.change("notify.ack", buildJsonObject { put("notification_id", n.id) }, "Mark read")
        val link = n.link as? JsonObject
        val op = link?.s("op")
        val payload = link?.o("payload")
        val taskId = payload?.l("task_id")
        val session = payload?.s("session")
        when {
            op == "task.get" && taskId != null -> nav.task(taskId.toString())
            // A hold is answered beside its agent, as the PC's notice opens its tile (notification_center.rs `route`).
            (op == "session.get" || op == "session.brief" || op == "guardrail.confirm") && session != null -> nav.chat(session)
            op == "guardrail.confirm" -> nav.guardrails(n.projectId)
            n.category == "agent_done" || n.category == "agent_blocked" -> nav.board(n.projectId)
            n.category == "aide" -> nav.aide()
        }
    }

    SpaceFrame(nav) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
            LazyColumn(
                Modifier.column().fillMaxSize(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp),
            ) {
                item {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        T("Inbox", Modifier.weight(1f), Relay.type.heading.copy(fontSize = 24.sp, lineHeight = 30.sp), Relay.colors.ink)
                        if (unread > 0) Key("Mark all read", { nav.shell.change("notify.ack_all", buildJsonObject { }, "Mark all read") }, kind = KeyKind.Quiet, compact = true)
                    }
                }

                val stuck = s.sessions.filter { it.blocked }
                if (s.holds.isNotEmpty() || stuck.isNotEmpty()) {
                    item { Eyebrow("Needs you · ${s.holds.size + stuck.size}", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 8.dp), Relay.colors.waiting) }
                    items(stuck, key = { "stuck:${it.name}" }) { a ->
                        Slab(Modifier.fillMaxWidth().padding(bottom = 8.dp), padding = PaddingValues(14.dp), onClick = { nav.chat(a.name) }) {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                AgentBadge(a, 34.dp)
                                Column(Modifier.weight(1f)) {
                                    val title = agentTitle(a)
                                    T(title, style = Relay.type.uiMedium, color = Relay.colors.ink, weight = FontWeight.SemiBold, maxLines = 1)
                                    T(
                                        listOfNotNull(a.name.takeIf { it != title }, names[a.projectId].takeIf { projects.size > 1 }, "needs you").joinToString(" · "),
                                        style = Relay.type.caption, color = Relay.colors.waiting, maxLines = 1,
                                    )
                                }
                                Key("Answer", { nav.chat(a.name) }, kind = KeyKind.Primary, compact = true)
                            }
                        }
                    }
                }
                if (s.holds.isNotEmpty()) {
                    items(s.holds, key = { "hold:${it.id}" }) { hold ->
                        HoldCard(
                            hold,
                            more = 0,
                            model = nav.shell,
                            onLater = null,
                            onOpen = { nav.guardrails(hold.projectId) },
                            modifier = Modifier.padding(bottom = 8.dp),
                            openLabel = "Open Guardrails",
                        )
                    }
                }

                if (review.isNotEmpty()) {
                    item { Eyebrow("Finished · waiting for review", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 8.dp)) }
                    itemsIndexed(review.entries.toList(), key = { _, e -> "review:${e.key}" }) { index, (project, waiting) ->
                        // With tasks in several projects, each row names whose board it opens.
                        val board = names[project]?.takeIf { review.size > 1 && it.isNotBlank() }?.let { "the $it board" } ?: "the board"
                        Slab(
                            Modifier.fillMaxWidth().padding(top = if (index > 0) 8.dp else 0.dp),
                            padding = PaddingValues(14.dp),
                            onClick = { nav.board(project) },
                        ) {
                            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    T(
                                        if (waiting.size == 1) "1 task waits for review" else "${waiting.size} tasks wait for review",
                                        style = Relay.type.uiMedium,
                                        color = Relay.colors.ink,
                                    )
                                    T("Open $board to look and approve.", style = Relay.type.caption, color = Relay.colors.ink3)
                                }
                                Key("Review", { nav.board(project) }, compact = true)
                            }
                        }
                    }
                }

                if (notifications.isNotEmpty()) {
                    item { Eyebrow("Earlier", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 4.dp)) }
                    itemsIndexed(notifications, key = { _, n -> "notify:${n.id}" }) { index, n ->
                        if (index > 0) Hairline()
                        NotificationRow(n, n.projectId?.let { names[it] }, onClick = { open(n) })
                    }
                }

                if (s.loaded && notified != null && listed != null && s.holds.isEmpty() && review.isEmpty() && notifications.isEmpty()) {
                    item { Empty("inbox", "You're all caught up", "Finished, blocked and held agents report here.") }
                }
            }
        }
    }
}

@Composable
private fun NotificationRow(n: Notification, project: String?, onClick: () -> Unit) {
    val c = Relay.colors
    val dot: Color? = if (n.read) null else when (n.category) {
        "agent_blocked", "guardrail" -> c.waiting
        "agent_done" -> c.live
        "aide" -> c.waiting
        else -> c.ink3
    }
    // What kind, then where, as the PC's: a long project name is cut, not the kind.
    val meta = listOfNotNull(categoryLabel(n.category), project?.takeIf { it.isNotBlank() }).joinToString(" · ").uppercase()
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .semantics { stateDescription = if (n.read) "Read" else "Unread" }
            .padding(horizontal = 4.dp, vertical = 12.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Box(Modifier.size(32.dp).clip(Radii.key).background(c.slab), contentAlignment = Alignment.Center) {
            Glyph(
                when (n.category) {
                    "agent_done" -> "check"
                    "agent_blocked" -> "help"
                    "guardrail" -> "shield"
                    else -> "bell"
                },
                15.dp,
                dot ?: c.ink3,
            )
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                T(
                    n.title,
                    Modifier.weight(1f),
                    Relay.type.ui,
                    if (n.read) c.ink2 else c.ink,
                    weight = if (n.read) FontWeight.Medium else FontWeight.SemiBold,
                )
                Spacer(Modifier.width(8.dp))
                T(ago(epoch(n.createdAt)), style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
            if (n.body.isNotBlank()) T(n.body, style = Relay.type.ui, color = c.ink2, maxLines = 3)
            T(meta.lowercase().replaceFirstChar { it.uppercase() }, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
    }
}

/**
 * A kind as the PC's notification settings name it (tools_settings.rs), so a row reads like the
 * switch that silences it; but a refusal is filed as "guardrail" too, so that is only Guardrail,
 * and a row of the PC's "Everything else" reads System.
 */
private fun categoryLabel(category: String) = when (category) {
    "agent_done" -> "Agent finished"
    "agent_blocked" -> "Agent needs you"
    "aide" -> "Aide"
    "guardrail" -> "Guardrail"
    "integration" -> "Integrations"
    "provider" -> "Providers"
    "disk" -> "Disk"
    "system" -> "System"
    else -> category.replace('_', ' ')
}
