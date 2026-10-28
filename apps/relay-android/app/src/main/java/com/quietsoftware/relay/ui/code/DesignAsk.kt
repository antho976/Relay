package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.design.DesignRow
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.dev.ChatDrafts
import com.quietsoftware.relay.ui.dev.DevSheet
import com.quietsoftware.relay.ui.dev.agentTitle
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay

/** A name or title as the prompt quotes it (ask.rs `clean`): one line, double quotes made single. */
private fun clean(s: String): String =
    s.map { if (it.isISOControl()) ' ' else if (it == '"') '\'' else it }.joinToString("").split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")

/** The request as it goes (ask.rs `clean_request`): its lines kept, blank runs as one, other control characters dropped. */
private fun cleanRequest(s: String): String {
    val out = ArrayList<String>()
    for (raw in s.lines()) {
        val l = raw.filter { it == '\t' || !it.isISOControl() }.replace('\t', ' ').split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")
        if (!(l.isEmpty() && (out.lastOrNull()?.isEmpty() ?: true))) out.add(l)
    }
    while (out.lastOrNull()?.isEmpty() == true) out.removeAt(out.size - 1)
    return out.joinToString("\n")
}

/**
 * What the agent reads, word for word as the PC's design Ask writes it (ask.rs `prompt`) for a
 * whole page with nothing selected and no picture: the phone has no selection to name.
 */
internal fun designPrompt(title: String, design: Long, page: String, pageName: String, request: String): String =
    "[Relay design] \"${clean(title)}\" (design_id $design, page \"${clean(pageName)}\" ($page)). Nothing selected: about the whole page $page." +
        " Request: ${cleanRequest(request)} — read it with design.get {\"design_id\":$design,\"page\":\"$page\"} and change it with design.apply."

/**
 * The design's Ask key (the PC's Ask on the canvas): what should change on [page], written to the
 * design's own agent (its conversation opens with the message ready to send) or to a new agent.
 */
@Composable
internal fun AskDesignKey(nav: Nav, design: DesignRow?, page: String?, pageName: String) {
    if (design == null || page == null) return
    var open by remember { mutableStateOf(false) }
    IconKey("edit", { open = true }, size = 18.dp, tint = Relay.colors.ink2, label = "Ask an agent to change this page")
    if (open) AskDesignSheet(nav, design, page, pageName) { open = false }
}

@Composable
private fun AskDesignSheet(nav: Nav, design: DesignRow, page: String, pageName: String, onDismiss: () -> Unit) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    var request by rememberSaveable(design.id, page) { mutableStateOf("") }
    // The design's own terminal, while it is open; a closed one cannot take the words.
    val agent = design.session?.let { name -> s.sessions.firstOrNull { it.name == name && it.state !in setOf("closed", "exited") } }
    val title = design.title.ifBlank { "Design" }
    DevSheet(onDismiss, "Ask about $pageName", title) { close ->
        Column(Modifier.padding(horizontal = 6.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Field(request, { request = it }, Modifier.fillMaxWidth(), placeholder = "What should change on this page?", singleLine = false, minLines = 3)
            val ready = request.isNotBlank()
            val prompt = { designPrompt(title, design.id, page, pageName, request) }
            if (agent != null) {
                Key(
                    "Write to ${agentTitle(agent).takeIf { it != agent.name }?.let { "${agent.name}: $it" } ?: agent.name}",
                    {
                        val words = prompt()
                        close {
                            // Into its message box, not sent: the person sees what goes and sends it.
                            ChatDrafts.of(agent.name).text = TextFieldValue(words, TextRange(words.length))
                            nav.chat(agent.name)
                        }
                    },
                    Modifier.fillMaxWidth(),
                    kind = KeyKind.Primary,
                    glyph = "send",
                    enabled = ready,
                )
            }
            Key(
                "Start a new agent on it",
                { val words = prompt(); close { nav.launch(design.projectId, prompt = words) } },
                Modifier.fillMaxWidth(),
                kind = if (agent == null) KeyKind.Primary else KeyKind.Plain,
                glyph = "plus",
                enabled = ready,
            )
            T(
                if (agent != null) "The message opens in its conversation, ready to send. The agent reads the page and changes it on the PC."
                else "This design has no agent at work. A new one reads the page and changes it on the PC.",
                style = Relay.type.caption,
                color = c.ink3,
            )
        }
    }
}
