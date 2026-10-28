package com.quietsoftware.relay.ui.dev

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Mail
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.Spinner
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

/**
 * One agent's facts and settings, as the PC's agent menu has them (agent_menu.rs): where it
 * stands and its lifecycle keys, its details, the launch brief it was given, model, effort and
 * permissions, and its mail. Reads the phone's copy, so it opens with the PC away.
 */
@Composable
fun SessionScreen(name: String, nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val row by remember(name) { nav.relay.session(name) }.collectAsStateWithLifecycle(null)
    val got by remember(name) { nav.relay.live("session.get", named(name)) }.collectAsStateWithLifecycle(RelayData.Live())
    val full = got.result as? JsonObject
    val session = row ?: full?.decodeAs<Session>()
    val life = rememberLifecycle(nav) { nav.leaveAgent(it) }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(name, nav::back, subtitle = session?.let { "${providerName(it.provider)} · ${it.role}" }) {
                if (session != null && session.state != "closed" && !session.pending) {
                    IconKey("notes", { nav.chat(name) }, size = 18.dp, tint = Relay.colors.ink2, label = "Conversation")
                }
            }
            if (session == null) {
                if (got.loading) Empty("terminal", "Reading $name", action = { Spinner(16.dp) }) else Empty("terminal", "No such agent", got.error?.let(::refusal) ?: "It may have been closed on the PC.")
                return@Column
            }
            Column(
                Modifier.weight(1f).fillMaxWidth().navigationBarsPadding().verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                    if (row == null && !got.fresh) StaleNote(got.at)
                    StateCard(session, s.projects.firstOrNull { it.num == session.projectId }?.name)
                    Actions(session, nav, life, s.online)
                    Details(session, full?.s("closed_at"), nav)
                    BriefFold(nav, name)
                    if (session.state != "closed" && !session.pending) Settings(session, nav)
                    MailSection(session, nav)
                }
            }
        }
    }
    LifecycleSheets(life)
}

// ---- Where it stands ----

@Composable
private fun StateCard(session: Session, project: String?) {
    val c = Relay.colors
    val held = session.state == "blocked"
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(14.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LampDot(session.lamp, 7.dp)
            T(session.stateLabel, style = Relay.type.plateState.copy(letterSpacing = 0.1.em), color = if (held) c.heldText else c.ink)
        }
        val line = listOfNotNull(
            project,
            session.pid?.let { "pid $it" },
            epoch(session.lastOutputAt).takeIf { it > 0 }?.let { "output ${ago(it)}" },
        ).joinToString(" · ")
        if (line.isNotEmpty()) {
            Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                Glyph("folder", 13.dp, c.ink3)
                T(line, style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
        }
        session.intent?.takeIf { it.isNotBlank() }?.let {
            T(it, Modifier.padding(top = 8.dp), Relay.type.ui.copy(fontStyle = FontStyle.Italic), c.ink2)
        }
    }
}

/**
 * Where to look at the agent, side by side and the width of the page; under them the lifecycle
 * keys for its state, as the PC's agent menu orders them, wrapped so none is cut off.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Actions(session: Session, nav: Nav, life: Lifecycle, online: Boolean) {
    if (session.pending) {
        T(slateHint(session, online), Modifier.padding(vertical = 12.dp), Relay.type.caption, Relay.colors.ink3)
        return
    }
    Column(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (session.state != "closed") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Open conversation", { nav.chat(session.name) }, Modifier.weight(1f), kind = KeyKind.Primary, glyph = "message")
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            for (act in actsFor(session.state)) {
                Key(
                    if (act == Act.Start) "Start session" else act.label,
                    { life.run(act, session) },
                    Modifier.alpha(if (online) 1f else .5f),
                    kind = if (act.danger) KeyKind.Danger else KeyKind.Plain,
                    glyph = act.glyph,
                    enabled = life.busy == null,
                    compact = true,
                )
            }
        }
    }
}

// ---- Details ----

private class Fact(val label: String, val value: String, val mono: Boolean = false, val open: (() -> Unit)? = null)

private val WHEN = DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM, FormatStyle.SHORT)

private fun stamp(ts: String?): String? {
    val ms = epoch(ts).takeIf { it > 0 } ?: return null
    return WHEN.format(Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault())) + " · " + ago(ms)
}

@Composable
private fun Details(session: Session, closedAt: String?, nav: Nav) {
    val facts = listOfNotNull(
        session.branch.takeIf { it.isNotBlank() }?.let { Fact("Branch", it, mono = true) },
        session.worktree.takeIf { it.isNotBlank() }?.let { Fact("Worktree", it, mono = true) },
        session.taskId?.let { id -> Fact("Task", "#$id") { nav.task(id.toString()) } },
        session.pairWith?.takeIf { it.isNotBlank() }?.let { p -> Fact("Paired with", p) { nav.session(p) } },
        Fact("Model", modelName(session.model.orEmpty())),
        Fact("Effort", session.effort?.takeIf { it.isNotBlank() } ?: "Provider default"),
        session.pid?.let { Fact("Process", "pid $it") },
        session.exitCode?.let { Fact("Exit code", it.toString()) },
        stamp(session.createdAt)?.let { Fact("Created", it) },
        stamp(session.spawnedAt)?.let { Fact("Started", it) },
        stamp(session.lastOutputAt)?.let { Fact("Last output", it) },
        stamp(closedAt)?.let { Fact("Closed", it) },
    )
    SectionLabel("Details")
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(vertical = 4.dp)) {
        facts.forEachIndexed { i, f ->
            if (i > 0) Hairline(Modifier.padding(horizontal = 12.dp))
            FactRow(f, nav)
        }
    }
    T("Long-press a value to copy it.", Modifier.padding(start = 4.dp, top = 6.dp), Relay.type.caption, Relay.colors.ink3)
}

/** A label and its value; paths cut in the middle so both ends stay readable; a long press copies it. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun FactRow(f: Fact, nav: Nav) {
    val c = Relay.colors
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .combinedClickable(
                onClick = { f.open?.invoke() },
                onLongClickLabel = "Copy",
                onLongClick = {
                    scope.launch {
                        clipboard.copy(f.value)
                        nav.shell.toast("Copied ${f.label.lowercase()}")
                    }
                },
            )
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        T(f.label, Modifier.width(96.dp), Relay.type.caption, c.ink3, maxLines = 1)
        BasicText(
            f.value,
            Modifier.weight(1f),
            style = (if (f.mono) Relay.type.mono else Relay.type.ui).copy(color = if (f.open != null) c.ink else c.ink2),
            maxLines = if (f.mono) 1 else 3,
            overflow = if (f.mono) TextOverflow.MiddleEllipsis else TextOverflow.Ellipsis,
        )
        if (f.open != null) Glyph("chevron-right", 13.dp, c.ink3)
    }
}

// ---- Launch brief ----

/** What the agent is told at launch (`session.brief`), read only when asked for. */
@Composable
private fun BriefFold(nav: Nav, name: String) {
    val c = Relay.colors
    var open by rememberSaveable(name) { mutableStateOf(false) }
    Slab(Modifier.fillMaxWidth().padding(top = 18.dp), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
        ListRow(onClick = { open = !open }) {
            Column(Modifier.weight(1f)) {
                T("Launch brief", style = Relay.type.uiMedium, color = c.ink)
                T("What the agent is told at launch: state, peers, nearby tasks and skills.", style = Relay.type.caption, color = c.ink3)
            }
            Glyph(if (open) "chevron-up" else "chevron-down", 13.dp, c.ink3)
        }
        if (open) Brief(nav, name)
    }
}

@Composable
private fun ColumnScope.Brief(nav: Nav, name: String) {
    val c = Relay.colors
    val brief by remember(name) { nav.relay.live("session.brief", named(name)) }.collectAsStateWithLifecycle(RelayData.Live())
    val text = (brief.result as? JsonObject)?.s("text")
    Column(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 8.dp)) {
        when {
            text != null -> {
                if (!brief.fresh) StaleNote(brief.at)
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(max = 420.dp)
                        .clip(Radii.key)
                        .background(c.screen)
                        .border(1.dp, c.edge, Radii.key)
                        .verticalScroll(rememberScrollState())
                        .padding(12.dp),
                ) {
                    BasicText(text.ifBlank { "The brief is empty." }, style = Relay.type.mono.copy(color = c.ink2))
                }
            }
            brief.error != null -> T(refusal(brief.error!!), Modifier.padding(8.dp), Relay.type.caption, c.heldText)
            else -> Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Spinner(11.dp)
                T("Reading the brief…", style = Relay.type.caption, color = c.ink3)
            }
        }
    }
}

// ---- Settings ----

private data class Draft(val model: String, val effort: String, val writes: Boolean, val ui: Boolean)

/** Claude's and Codex's reasoning efforts, as the engine validates them (providers.rs). */
internal fun effortsFor(provider: String): List<String> =
    if (provider == "codex") listOf("minimal", "low", "medium", "high", "xhigh") else listOf("low", "medium", "high", "xhigh", "max")

/**
 * Model and effort (fixed once the agent has started: the engine refuses them after), and the
 * two permissions. Only what changed is sent; model and effort cannot be set back to empty.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Settings(session: Session, nav: Nav) {
    val c = Relay.colors
    var draft by remember(session.name) { mutableStateOf<Draft?>(null) }
    val form = draft ?: Draft(session.model.orEmpty(), session.effort.orEmpty(), session.busWrites, session.allowUi)
    val spawned = session.spawnedAt != null
    fun edit(next: Draft) {
        draft = next
    }
    SectionLabel("Settings")
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(14.dp)) {
        Eyebrow("Model", Modifier.padding(bottom = 6.dp))
        if (spawned) {
            T(modelName(form.model), style = Relay.type.ui, color = c.ink2)
        } else {
            Models(session.provider, form.model, owner = session.name) { edit(form.copy(model = it)) }
        }
        Eyebrow("Effort", Modifier.padding(top = 14.dp))
        FlowRow(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            for (e in effortsFor(session.provider)) {
                Pill(e, Modifier.heightIn(min = 36.dp), selected = form.effort == e, onClick = if (spawned) null else ({ edit(form.copy(effort = e)) }))
            }
        }
        if (spawned) T("Model and effort are fixed once the agent has started.", Modifier.padding(top = 8.dp), Relay.type.caption, c.ink3)
        Box(Modifier.padding(top = 10.dp)) {
            Column {
                SwitchRow("Agent bus writes", "Let it write files, stage and commit through Relay, with guardrails checked as it goes.", form.writes, { edit(form.copy(writes = it)) })
                SwitchRow("UI control", "Let it drive the Relay window.", form.ui, { edit(form.copy(ui = it)) })
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.End) {
            Key(
                "Save changes",
                {
                    val payload = buildJsonObject {
                        put("session", session.name)
                        put("bus_writes", form.writes)
                        put("allow_ui", form.ui)
                        if (!spawned) {
                            form.model.trim().takeIf { it.isNotEmpty() && it != session.model.orEmpty() }?.let { put("model", it) }
                            form.effort.takeIf { it.isNotEmpty() && it != session.effort.orEmpty() }?.let { put("effort", it) }
                        }
                    }
                    nav.shell.act("session.update", payload, done = "Settings saved for ${session.name}") { draft = null }
                },
                kind = KeyKind.Primary,
                enabled = draft != null && draft != Draft(session.model.orEmpty(), session.effort.orEmpty(), session.busWrites, session.allowUi),
            )
        }
    }
}

// ---- Mail ----

/** Mail to and from this agent, oldest first, and a line to write to it. */
@Composable
private fun MailSection(session: Session, nav: Nav) {
    val c = Relay.colors
    val all by remember(session.projectId) { nav.relay.mail(session.projectId) }.collectAsStateWithLifecycle(emptyList())
    val mine = all.filter { it.from == session.name || it.to == session.name }
    SectionLabel("Mail") {
        if (mine.size > 8) {
            Key("All", { nav.mailbox(session.projectId, session.name) }, kind = KeyKind.Quiet, compact = true)
        }
    }
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 6.dp)) {
        if (mine.isEmpty()) {
            T("No mail to or from ${session.name} yet.", Modifier.padding(vertical = 10.dp), Relay.type.caption, c.ink3)
        }
        mine.takeLast(8).forEachIndexed { i, m ->
            if (i > 0) Hairline()
            MailLine(m)
        }
    }
    if (session.state != "closed" && !session.pending) MailComposer(session, nav)
}

@Composable
private fun MailLine(m: Mail) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val head = "${who(m.from)} → ${who(m.to)}" + if (m.priority) " · PRIORITY" else ""
            T(head, Modifier.weight(1f), Relay.type.mono, if (m.priority) c.waiting else c.ink3, maxLines = 1)
            epoch(m.sentAt).takeIf { it > 0 }?.let { T(ago(it), style = Relay.type.caption, color = c.ink3) }
        }
        T(m.text, style = Relay.type.ui, color = c.ink)
    }
}

private fun who(name: String) = when (name) {
    "user" -> "You"
    "*" -> "everyone"
    else -> name
}

@Composable
private fun MailComposer(session: Session, nav: Nav) {
    val c = Relay.colors
    var body by rememberSaveable(session.name) { mutableStateOf("") }
    var priority by rememberSaveable(session.name) { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().padding(top = 10.dp, bottom = 24.dp)) {
        Field(body, { body = it }, Modifier.fillMaxWidth(), placeholder = "Write to ${session.name}", singleLine = false, minLines = 2, maxLines = 6)
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            // The switch and its words are one target; Send stays a key of its own.
            Row(Modifier.weight(1f).heightIn(min = 44.dp).clip(Radii.key).switchRow(priority) { priority = it }, verticalAlignment = Alignment.CenterVertically) {
                Toggle(priority, null)
                T("Priority: ask this agent to read it at the next safe point", Modifier.weight(1f).padding(start = 10.dp), Relay.type.caption, c.ink2)
            }
            Key(
                "Send",
                {
                    val text = body.trim()
                    nav.shell.change(
                        "mailbox.send",
                        buildJsonObject {
                            put("project_id", session.projectId)
                            put("to", session.name)
                            put("text", text)
                            put("priority", priority)
                        },
                        "Mail ${session.name}",
                    ) {
                        body = ""
                        priority = false
                    }
                },
                kind = KeyKind.Primary,
                glyph = "send",
                enabled = body.isNotBlank(),
                compact = true,
            )
        }
    }
}
