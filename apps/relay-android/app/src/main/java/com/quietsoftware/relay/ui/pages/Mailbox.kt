package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Mail
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * A project's mail between the person and the agents, newest first, filtered to one session when
 * given, with a composer for one session or everyone. A message sent offline waits in the outbox.
 */
@Composable
fun MailboxScreen(projectId: Long, session: String?, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val mail by remember(projectId) { nav.relay.mail(projectId) }.collectAsStateWithLifecycle(initialValue = emptyList())
    val project = s.projects.firstOrNull { it.num == projectId }?.name
    val sessions = s.sessions.filter { it.projectId == projectId && it.state != "closed" }.map { it.name }
    val shown = mail.filter { session == null || it.from == session || it.to == session }.asReversed()

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Mailbox", nav::back, subtitle = session?.let { "With $it" } ?: project)
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                LazyColumn(
                    Modifier.widthIn(max = 720.dp).fillMaxSize(),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                ) {
                    if (shown.isEmpty()) {
                        item { Empty("mail", "No mail yet", "Messages between you and your agents appear here.") }
                    }
                    items(shown, key = { it.id }) { m ->
                        MailRow(m)
                    }
                }
            }
            Composer(projectId, sessions, session, nav)
        }
    }
}

@Composable
private fun MailRow(m: Mail) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        val head = "${who(m.from)} → ${who(m.to)}"
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            T(
                if (m.priority) "$head · PRIORITY" else head,
                Modifier.weight(1f),
                Relay.type.mono,
                if (m.priority) c.waiting else c.ink3,
                maxLines = 1,
            )
            if (m.sentAt.isNotEmpty()) {
                Spacer(Modifier.width(8.dp))
                T(ago(epoch(m.sentAt)), style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
        }
        T(m.text, style = Relay.type.ui, color = c.ink)
    }
}

@Composable
private fun Composer(projectId: Long, sessions: List<String>, session: String?, nav: Nav) {
    val c = Relay.colors
    var to by rememberSaveable { mutableStateOf(session ?: "*") }
    // A name typed in: an agent not among the open ones (a closed session, a peer on another branch).
    var other by rememberSaveable { mutableStateOf(session != null && session !in sessions) }
    var body by rememberSaveable { mutableStateOf("") }
    var priority by rememberSaveable { mutableStateOf(false) }
    val ready = to.trim().isNotEmpty() && body.trim().isNotEmpty()

    Column(
        Modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            .imePadding()
            .padding(horizontal = 12.dp, vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Slab(Modifier.widthIn(max = 720.dp).fillMaxWidth(), padding = PaddingValues(12.dp)) {
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Pill("Everyone", selected = !other && to == "*", onClick = { other = false; to = "*" })
                sessions.forEach { name -> Pill(name, selected = !other && to == name, onClick = { other = false; to = name }) }
                Pill("Other…", selected = other, onClick = { if (!other) { other = true; to = "" } })
            }
            Gap(8.dp)
            if (other) {
                Field(to, { to = it }, Modifier.fillMaxWidth(), placeholder = "To: an agent's name", style = Relay.type.mono)
                Gap(8.dp)
            }
            Field(
                body,
                { body = it },
                Modifier.fillMaxWidth(),
                placeholder = "Write a message",
                singleLine = false,
                minLines = 2,
                maxLines = 6,
            )
            Gap(8.dp)
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Toggle(priority, { priority = it })
                Spacer(Modifier.width(10.dp))
                T("Priority: read it at the next safe point", Modifier.weight(1f), Relay.type.caption, c.ink2)
                Key(
                    "Send",
                    {
                        val recipient = to.trim()
                        val text = body.trim()
                        nav.shell.change(
                            "mailbox.send",
                            buildJsonObject {
                                put("project_id", projectId)
                                put("to", recipient)
                                put("text", text)
                                put("priority", priority)
                            },
                            "Message to ${who(recipient)}",
                        ) { body = ""; priority = false }
                    },
                    kind = KeyKind.Primary,
                    enabled = ready,
                    compact = true,
                )
            }
        }
    }
}

private fun who(name: String) = when (name) {
    "user" -> "You"
    "*" -> "everyone"
    else -> name
}
