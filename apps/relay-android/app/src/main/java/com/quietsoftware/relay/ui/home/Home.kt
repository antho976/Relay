package com.quietsoftware.relay.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.code.SheetFrame
import com.quietsoftware.relay.ui.dev.SheetRow
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.shell.ShellState
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.ThreadSource
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.LocalTime

/** Where Home's message box sends: a project (a new agent there) or one of the threads' spaces. */
private sealed interface Target {
    val label: String
    data class Project(val num: Long, override val label: String) : Target
    data class Space(val key: String, override val label: String) : Target
}

private val SPACES = listOf("tally" to "Tally", "arbiter" to "Arbiter", "avex" to "Avex")
private val SPACE_GLYPH = mapOf("tally" to "wallet", "arbiter" to "arbiter", "avex" to "avex")

/**
 * Home, as Claude's app opens (design 14, "Home · new chat"): a greeting, what needs the person
 * in one pill, a row of things to start, and one message box. The chip at the top says where the
 * box sends: a project, where the words become a new agent's task, or Tally, Arbiter or Avex,
 * where they start a thread. Nothing here is a terminal.
 */
@Composable
fun HomeScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // "p:<project>" or "s:<space>"; empty follows the last choice (the space and project the app opened last).
    var pick by rememberSaveable { mutableStateOf("") }
    var choosing by remember { mutableStateOf(false) }
    val target = remember(pick, s.projects, s.prefs.space, s.prefs.lastProject) { targetOf(pick, s) }
    SpaceFrame(nav, center = { TargetChip(target, s.online) { choosing = true } }) {
        Home(nav, s, target)
    }
    if (choosing) {
        SheetFrame({ choosing = false }) {
            T("Send to", Modifier.padding(start = 6.dp, bottom = 6.dp), Relay.type.title, Relay.colors.ink)
            Eyebrow("A new agent in", Modifier.padding(start = 6.dp, top = 6.dp, bottom = 4.dp))
            for (p in s.projects) {
                val num = p.num ?: continue
                SheetRow(p.name, "folder", {
                    pick = "p:$num"
                    nav.shell.pickProject(num)
                    nav.shell.pickSpace("dev")
                    choosing = false
                }, detail = s.liveIn(num).takeIf { it > 0 }?.let { "$it working" }, selected = target is Target.Project && target.num == num)
            }
            if (s.projects.isEmpty()) T("No projects yet.", Modifier.padding(6.dp), Relay.type.caption, Relay.colors.ink3)
            Eyebrow("A chat about", Modifier.padding(start = 6.dp, top = 12.dp, bottom = 4.dp))
            for ((key, label) in SPACES) {
                SheetRow(label, SPACE_GLYPH.getValue(key), {
                    pick = "s:$key"
                    ThreadSource.key = key
                    nav.shell.pickSpace("threads")
                    choosing = false
                }, selected = target is Target.Space && target.key == key)
            }
        }
    }
}

private fun targetOf(pick: String, s: ShellState): Target {
    val space = { key: String -> Target.Space(key, SPACES.first { it.first == key }.second) }
    when {
        pick.startsWith("p:") -> pick.removePrefix("p:").toLongOrNull()?.let { n -> s.projects.firstOrNull { it.num == n } }?.let { return Target.Project(it.num ?: 0, it.name) }
        pick.startsWith("s:") -> pick.removePrefix("s:").takeIf { k -> SPACES.any { it.first == k } }?.let { return space(it) }
    }
    if (s.prefs.space == "threads") return space(ThreadSource.key.takeIf { k -> SPACES.any { it.first == k } } ?: "tally")
    return s.project?.let { Target.Project(it.num ?: 0, it.name) } ?: space("tally")
}

@Composable
private fun TargetChip(target: Target, online: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.heightIn(min = 36.dp).clip(Radii.pill).background(c.slab).clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = "Sending to ${target.label}. Change" }
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        when (target) {
            is Target.Project -> Dot(if (online) c.live else c.ink3, 7.dp)
            is Target.Space -> Glyph(SPACE_GLYPH.getValue(target.key), 15.dp, c.ink2)
        }
        T(target.label, style = Relay.type.uiMedium, color = c.ink, weight = FontWeight.SemiBold, maxLines = 1)
        Glyph("chevron-down", 14.dp, c.ink3)
    }
}

@Composable
private fun Home(nav: Nav, s: ShellState, target: Target) {
    val c = Relay.colors
    var draft by rememberSaveable { mutableStateOf("") }
    val working = s.sessions.count { it.state == "running" || it.state == "spawning" }
    val blocked = s.sessions.filter { it.state == "blocked" }
    val needs = s.holds.size + blocked.size

    fun send(text: String) {
        val t = text.trim()
        if (t.isEmpty()) return
        when (target) {
            is Target.Project -> nav.launch(target.num, prompt = t)
            is Target.Space -> startThread(nav, s, target.key, t)
        }
        if (t == draft.trim()) draft = ""
    }

    Column(Modifier.fillMaxSize()) {
        Column(
            Modifier.fillMaxWidth().weight(1f).padding(horizontal = 28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
        ) {
            Box(Modifier.size(48.dp).clip(CircleShape).background(c.accent), contentAlignment = Alignment.Center) {
                T("r", Modifier.padding(bottom = 3.dp), Relay.type.brand.copy(fontSize = 24.sp), c.onAccent)
            }
            T(
                greeting(s.prefs.name),
                Modifier.fillMaxWidth(),
                Relay.type.heading.copy(fontFamily = FontFamily.Serif, fontWeight = FontWeight.Normal, fontSize = 30.sp, lineHeight = 36.sp, textAlign = TextAlign.Center),
                c.ink,
            )
            T(
                when {
                    working == 0 && needs == 0 -> "Nothing running. What should we start?"
                    needs == 0 -> plural(working, "agent") + " working"
                    else -> plural(working, "agent") + " working · $needs need" + (if (needs == 1) "s" else "") + " you"
                },
                style = Relay.type.ui.copy(textAlign = TextAlign.Center),
                color = c.ink2,
            )
            val hold = s.holds.firstOrNull()
            when {
                hold != null -> NeedsPill(holdLine(hold), "Review") { nav.inbox() }
                blocked.isNotEmpty() -> NeedsPill("${blocked.first().name} needs you", "Open") { nav.chat(blocked.first().name) }
            }
        }
        LazyRow(
            Modifier.fillMaxWidth().padding(bottom = 12.dp),
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(starters(target, nav), key = { it.label }) { st -> StarterChip(st) }
        }
        HomeComposer(
            value = draft,
            onValue = { draft = it },
            placeholder = when (target) {
                is Target.Project -> "Tell a new agent what to do in ${target.label}…"
                is Target.Space -> "Ask ${target.label} anything…"
            },
            onMore = { if (target is Target.Project) nav.launch(target.num, prompt = draft.trim().ifEmpty { null }) },
            moreLabel = if (target is Target.Project) "Agent options" else null,
            onSend = { send(draft) },
            modifier = Modifier.padding(start = 12.dp, end = 12.dp, bottom = 8.dp),
        )
    }
}

/**
 * Starts a thread about [space] with [text], through the outbox so it starts with the PC away too,
 * on the newest thread's model and effort, and opens it.
 */
internal fun startThread(nav: Nav, s: ShellState, space: String, text: String) {
    val t = text.trim()
    if (t.isEmpty()) return
    ThreadSource.key = space
    val last = s.threads.firstOrNull()
    val payload = buildJsonObject {
        put("text", t)
        last?.model?.takeIf { it.isNotEmpty() }?.let { put("model", it) }
        last?.effort?.takeIf { it.isNotEmpty() }?.let { put("effort", it) }
    }
    nav.shell.change("thread.create", payload, "New thread") { change ->
        when (change) {
            is Hub.Change.Queued -> nav.thread(Optimistic.tempId(change.entry.id))
            is Hub.Change.Now -> (change.result as? JsonObject)?.l("id")?.let { nav.thread(it.toString()) }
        }
    }
}

/** A one-line ask at the foot of a space's page (design 14, "Ask Tally"): the words start a thread about it. */
@Composable
internal fun AskBar(nav: Nav, space: String, placeholder: String, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    var text by rememberSaveable(space) { mutableStateOf("") }
    Row(
        modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(c.slab).border(1.dp, c.strong, RoundedCornerShape(24.dp))
            .padding(start = 18.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        BasicTextField(
            value = text,
            onValueChange = { text = it },
            modifier = Modifier.weight(1f).semantics { contentDescription = placeholder },
            textStyle = Relay.type.body.copy(color = c.ink),
            cursorBrush = SolidColor(c.accent),
            maxLines = 4,
            decorationBox = { inner ->
                Box {
                    if (text.isEmpty()) T(placeholder, style = Relay.type.body, color = c.ink3, maxLines = 1)
                    inner()
                }
            },
        )
        val can = text.isNotBlank()
        Box(
            Modifier.size(36.dp).alpha(if (can) 1f else .45f).clip(CircleShape).background(c.accent)
                .clickable(enabled = can, role = Role.Button) {
                    startThread(nav, s, space, text)
                    text = ""
                }.semantics { contentDescription = "Ask" },
            contentAlignment = Alignment.Center,
        ) { Glyph("arrow-up", 17.dp, c.onAccent) }
    }
}

private class Starter(val label: String, val glyph: String, val run: () -> Unit, val lead: Boolean = false)

private fun starters(target: Target, nav: Nav): List<Starter> = when (target) {
    is Target.Project -> listOf(
        Starter("Start an agent on a task", "zap", { nav.launch(target.num) }, lead = true),
        Starter("Open the board", "board", { nav.board(target.num) }),
        Starter("What changed?", "branch", { nav.git(target.num) }),
        Starter("Agents", "agents", { nav.agents() }),
    )
    is Target.Space -> when (target.key) {
        "arbiter" -> listOf(Starter("Open Arbiter", "arbiter", { nav.arbiter() }, lead = true))
        "avex" -> listOf(Starter("Open Avex", "avex", { nav.avex() }, lead = true))
        else -> listOf(Starter("Open Tally", "wallet", { nav.tally() }, lead = true))
    } + Starter("All threads", "message", { nav.newThread() })
}

@Composable
private fun StarterChip(st: Starter) {
    val c = Relay.colors
    Row(
        Modifier.heightIn(min = 40.dp).clip(Radii.pill).border(1.dp, c.strong, Radii.pill).clickable(role = Role.Button, onClick = st.run).padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        Glyph(st.glyph, 15.dp, if (st.lead) c.accent else c.ink3)
        T(st.label, style = Relay.type.ui, color = c.ink, maxLines = 1)
    }
}

/** What needs the person, in one line with the key that deals with it. */
@Composable
private fun NeedsPill(text: String, key: String, onClick: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.clip(Radii.pill).background(c.slab).clickable(onClick = onClick).padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Dot(c.waiting, 7.dp)
        T(text, Modifier.weight(1f, fill = false), Relay.type.ui, c.ink, maxLines = 1, weight = FontWeight.Medium)
        T(key, Modifier.clip(Radii.pill).background(c.accent).padding(horizontal = 12.dp, vertical = 6.dp), Relay.type.caption, c.onAccent, weight = FontWeight.SemiBold)
    }
}

private fun holdLine(h: Hold): String {
    val who = h.session?.ifBlank { null } ?: h.actor.ifBlank { "An agent" }
    val what = when {
        h.op.startsWith("git.push") || h.op.contains("push") -> "wants to push"
        h.op.contains("exec") || h.op.contains("command") -> "wants to run a command"
        h.op.startsWith("guardrail.request") -> "asks for an exception"
        else -> "is waiting on you"
    }
    return "$who $what"
}

/**
 * The message box (design 14, "Composer"): the words on top, then the keys: options on the left,
 * send on the right in the accent.
 */
@Composable
internal fun HomeComposer(
    value: String,
    onValue: (String) -> Unit,
    placeholder: String,
    onMore: () -> Unit,
    moreLabel: String?,
    onSend: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = Relay.colors
    val shape = RoundedCornerShape(26.dp)
    Column(
        modifier.fillMaxWidth().clip(shape).background(c.slab).border(1.dp, c.strong, shape).padding(start = 16.dp, end = 10.dp, top = 14.dp, bottom = 10.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        BasicTextField(
            value = value,
            onValueChange = onValue,
            modifier = Modifier.fillMaxWidth().heightIn(min = 24.dp).semantics { contentDescription = placeholder },
            textStyle = Relay.type.body.copy(fontSize = 16.sp, color = c.ink),
            cursorBrush = SolidColor(c.accent),
            maxLines = 6,
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) T(placeholder, style = Relay.type.body.copy(fontSize = 16.sp), color = c.ink3, maxLines = 1)
                    inner()
                }
            },
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (moreLabel != null) {
                Box(
                    Modifier.size(36.dp).clip(CircleShape).border(1.dp, c.strong, CircleShape).clickable(role = Role.Button, onClick = onMore)
                        .semantics { contentDescription = moreLabel },
                    contentAlignment = Alignment.Center,
                ) { Glyph("sliders", 16.dp, c.ink) }
            }
            Box(Modifier.weight(1f))
            val can = value.isNotBlank()
            Box(
                Modifier.size(TOUCH - 8.dp).alpha(if (can) 1f else .45f).clip(CircleShape).background(c.accent)
                    .clickable(enabled = can, role = Role.Button, onClick = onSend).semantics { contentDescription = "Send" },
                contentAlignment = Alignment.Center,
            ) { Glyph("arrow-up", 17.dp, c.onAccent) }
        }
    }
}

private fun greeting(name: String): String {
    val part = when (LocalTime.now().hour) {
        in 5..11 -> "Morning"
        in 12..17 -> "Afternoon"
        in 18..22 -> "Evening"
        else -> "Hello"
    }
    val first = name.trim().substringBefore(' ')
    return if (first.isEmpty()) part else "$part, $first"
}
