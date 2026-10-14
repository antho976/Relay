package com.quietsoftware.relay.ui.dev

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.ThreadMessage
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.data.SessionChat
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.Spinner
import com.quietsoftware.relay.ui.threads.WorkingLamp
import com.quietsoftware.relay.ui.threads.dayOf
import com.quietsoftware.relay.ui.threads.humanDay
import com.quietsoftware.relay.ui.threads.said
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import java.time.LocalDate

/**
 * One terminal agent as a conversation, drawn as a thread is: the person's messages, the agent's
 * words, and what it did as quiet lines that open to show the answer. Typing goes to the agent's
 * terminal through the PC, as on the terminal screen; when the agent stops to ask (a permission,
 * a menu), its question is read off the bottom of its screen and answered with keys. The terminal
 * itself is one tap away in the header, for whatever a conversation cannot show.
 */
@Composable
fun ChatScreen(name: String, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val session by remember(name) { nav.relay.session(name) }.collectAsStateWithLifecycle(null)
    val chat = remember(name) { SessionChat(nav.relay, name, Feeds.scope) }
    DisposableEffect(chat) {
        chat.start()
        onDispose { chat.stop() }
    }
    val messages by chat.messages.collectAsStateWithLifecycle()
    val sending by chat.sending.collectAsStateWithLifecycle()
    val status by chat.status.collectAsStateWithLifecycle()
    val found by chat.found.collectAsStateWithLifecycle()
    val error by chat.error.collectAsStateWithLifecycle()
    val life = rememberLifecycle(nav) { nav.back() }
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboard.current
    val focus = remember { FocusRequester() }
    var menu by remember { mutableStateOf(false) }
    var mailing by remember { mutableStateOf(false) }
    var text by rememberSaveable(name) { mutableStateOf("") }

    val ready = session?.attachable == true && s.online
    val working = session?.state == "running" || session?.state == "spawning"
    val held = session?.state == "blocked"
    val pieces = remember(messages, sending, working) { chatPieces(messages, sending, working, LocalDate.now()) }
    val doing = remember(pieces, working) { if (working) doingNow(pieces) else null }

    KeepScreenOn(s.prefs.keepScreenOn)

    fun send() {
        val typed = text.trim()
        if (typed.isEmpty()) {
            // Empty: Esc stops an agent mid-turn; otherwise it is the Enter a prompt waits for.
            chat.key(if (working) Keys.ESC else Keys.ENTER)
            if (working) nav.shell.toast("Asked ${session?.provider ?: "the agent"} to stop")
            return
        }
        text = ""
        scope.launch {
            if (!chat.send(typed)) {
                if (text.isEmpty()) text = typed
                nav.shell.toast("Not sent: the PC did not take it")
            }
        }
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            Header(session, name, onBack = nav::back, onDetails = { nav.session(name) }, onMenu = { menu = true }) {
                IconKey("terminal", { nav.console(name) }, size = 18.dp, tint = c.ink2)
            }
            Banner(status, session, s.online, life)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                val openTerminal = @Composable { Key("Open the terminal", { nav.console(name) }, glyph = "terminal", compact = true) }
                when {
                    pieces.isNotEmpty() -> Conversation(pieces, doing, held && ready, chat, nav, s.online)
                    error != null && status != TerminalFeed.Status.Saved -> Empty(
                        "terminal", "Can't read the conversation",
                        said(error!!.error, "The PC's Relay is older than this app: update it to see agents as a conversation. The terminal works meanwhile."),
                        action = openTerminal,
                    )
                    found == false -> Empty(
                        "terminal", "Nothing said yet",
                        "The conversation shows here once ${session?.provider ?: "the agent"} starts. The terminal shows everything meanwhile.",
                        action = openTerminal,
                    )
                    status == TerminalFeed.Status.Connecting -> Empty("terminal", "Reading the conversation")
                    else -> Empty("terminal", "Nothing said yet", if (s.online) null else "The PC is away, and this phone has not read this agent before.", action = openTerminal)
                }
            }
            QuickKeys(chat, ready)
            ChatComposer(
                text,
                { text = it },
                ready,
                working,
                placeholder = when {
                    session == null -> "This agent is closed"
                    !s.online -> "Needs the PC to talk to the agent"
                    !ready -> "The agent is not running"
                    else -> "Message ${session?.name ?: "the agent"}"
                },
                focus,
                ::send,
            )
        }
    }

    LifecycleSheets(life)
    if (menu) {
        DevSheet({ menu = false }, name, session?.let { "${it.provider} · ${it.role}" }) { close ->
            SheetRow("Open the terminal", "terminal", { close { nav.console(name) } })
            session?.let { ss ->
                SheetRow("Changes", "commit", { close { nav.git(ss.projectId, ss.worktree.ifBlank { null }, name) } })
                if (ss.state != "closed" && !ss.pending) SheetRow("Mail the agent", "send", { close { mailing = true } })
            }
            SheetRow("Copy the conversation", "copy", {
                close {
                    val all = transcriptText(messages)
                    scope.launch {
                        clipboard.copy(all)
                        nav.shell.toast("Copied")
                    }
                }
            })
            SheetRow("Session details", "sliders", { close { nav.session(name) } })
            session?.takeUnless { it.pending }?.let { ss ->
                for (act in actsFor(ss.state)) {
                    SheetRow(act.label, act.glyph, { close { life.run(act, ss) } }, danger = act.danger, dim = !s.online)
                }
            }
        }
    }
    if (mailing) {
        session?.let { ss -> MailSheet(ss, nav) { mailing = false } }
    }
}

// ---- The conversation as pieces ----

/** Where a tool call stands: answered, failed, still running, or cut off with no answer. */
internal enum class Step { Running, Done, Failed, Cut }

internal data class Call(val key: String, val name: String, val summary: String, val step: Step, val answer: String?)

internal sealed interface ChatPiece {
    val key: String

    data class Day(override val key: String, val label: String) : ChatPiece
    data class User(override val key: String, val text: String, val sending: Boolean) : ChatPiece
    data class Reply(override val key: String, val text: String) : ChatPiece
    data class Tool(override val key: String, val call: Call) : ChatPiece
    /** Three or more steps in a row, folded under one line: "Ran 3 commands, read 2 files". */
    data class Steps(override val key: String, val calls: List<Call>) : ChatPiece
    data class Note(override val key: String, val text: String) : ChatPiece
    data class Error(override val key: String, val text: String) : ChatPiece
}

private fun JsonObject.str(key: String) = (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content.orEmpty()

internal fun chatPieces(messages: List<ThreadMessage>, sending: List<ThreadMessage>, working: Boolean, today: LocalDate): List<ChatPiece> {
    val answers = HashMap<String, Pair<Boolean, String>>()
    for (m in messages) if (m.role == "tool") {
        val b = m.body as? JsonObject ?: continue
        answers[b.str("tool_use_id")] = ((b["is_error"] as? JsonPrimitive)?.booleanOrNull == true) to b.str("text")
    }
    val lastUser = messages.indexOfLast { it.role == "user" }
    val out = ArrayList<ChatPiece>()
    val steps = ArrayList<Call>()
    var lastDay: LocalDate? = null
    fun flush() {
        if (steps.size >= 3) out += ChatPiece.Steps("g:" + steps.first().key, steps.toList())
        else steps.forEach { out += ChatPiece.Tool(it.key, it) }
        steps.clear()
    }
    fun day(m: ThreadMessage) {
        val d = dayOf(m.createdAt) ?: return
        if (d != lastDay) {
            if (!(lastDay == null && d == today)) {
                flush()
                out += ChatPiece.Day("d:${m.id}", humanDay(d, today))
            }
            lastDay = d
        }
    }
    messages.forEachIndexed { index, m ->
        if (m.role == "tool") return@forEachIndexed
        day(m)
        when (m.role) {
            "user" -> {
                flush()
                out += ChatPiece.User("m:${m.id}", m.text, sending = false)
            }
            "assistant" -> ((m.body as? JsonObject)?.get("blocks") as? JsonArray).orEmpty().forEachIndexed { i, blk ->
                val o = blk as? JsonObject ?: return@forEachIndexed
                when (o.str("type")) {
                    "text" -> o.str("text").takeIf { it.isNotBlank() }?.let {
                        flush()
                        out += ChatPiece.Reply("m:${m.id}:$i", it.trim())
                    }
                    "tool_use" -> {
                        val id = o.str("id")
                        val answer = answers[id]
                        val step = when {
                            answer != null -> if (answer.first) Step.Failed else Step.Done
                            working && index > lastUser -> Step.Running
                            else -> Step.Cut
                        }
                        steps += Call("t:${id.ifEmpty { "${m.id}:$i" }}", o.str("name"), o.str("summary"), step, answer?.second)
                    }
                }
            }
            "note" -> {
                flush()
                out += ChatPiece.Note("m:${m.id}", m.text)
            }
            "error" -> {
                flush()
                out += ChatPiece.Error("m:${m.id}", m.text.ifBlank { "Something went wrong" })
            }
        }
    }
    flush()
    for (m in sending) out += ChatPiece.User("m:${m.id}", m.text, sending = true)
    return out
}

/** What the working row says: the step running now, in words. */
private fun doingNow(pieces: List<ChatPiece>): String {
    val call = when (val last = pieces.lastOrNull()) {
        is ChatPiece.Tool -> last.call
        is ChatPiece.Steps -> last.calls.last()
        else -> null
    }?.takeIf { it.step == Step.Running } ?: return "Working…"
    return "${verb(call.name).doing}${call.summary.takeIf { it.isNotBlank() }?.let { " $it" }.orEmpty()}…"
}

/** A tool, in words: what it did, what it is doing, how a count of it reads, its glyph. */
private class Verb(val did: String, val doing: String, val tally: String, val one: String, val many: String, val glyph: String)

private fun verb(name: String): Verb = when {
    name == "Bash" || name == "exec" || name == "shell" || name == "exec_command" || name == "local_shell" -> Verb("Ran", "Running", "ran", "command", "commands", "terminal")
    name == "Read" || name == "NotebookRead" -> Verb("Read", "Reading", "read", "file", "files", "file")
    name == "Edit" || name == "MultiEdit" || name == "NotebookEdit" || name == "apply_patch" -> Verb("Edited", "Editing", "edited", "file", "files", "edit")
    name == "Write" -> Verb("Wrote", "Writing", "wrote", "file", "files", "file-plus")
    name == "Grep" || name == "Glob" -> Verb("Searched", "Searching", "ran", "search", "searches", "search")
    name == "WebFetch" -> Verb("Fetched", "Fetching", "fetched", "page", "pages", "external")
    name == "WebSearch" || name == "web_search" -> Verb("Searched the web for", "Searching the web for", "ran", "web search", "web searches", "search")
    name == "Task" || name == "Agent" -> Verb("Asked a subagent:", "A subagent is on", "started", "subagent", "subagents", "user")
    name == "TodoWrite" || name == "update_plan" -> Verb("Updated the plan", "Updating the plan", "made", "plan update", "plan updates", "check")
    name.startsWith("mcp__relay__") -> name.removePrefix("mcp__relay__").replace('_', '.').let { Verb("Relay: $it", "Relay: $it", "made", "Relay call", "Relay calls", "dashboard") }
    name.startsWith("mcp__") -> name.removePrefix("mcp__").replace("__", ": ").let { Verb("Used $it", "Using $it", "used", "tool", "tools", "plugins") }
    else -> Verb("Used $name", "Using $name", "used", "tool", "tools", "code")
}

/** "Ran 3 commands, edited 2 files": a folded group's line, kinds in the order they first came. */
internal fun stepsLine(calls: List<Call>): String {
    val counts = LinkedHashMap<String, Pair<Verb, Int>>()
    for (call in calls) {
        val v = verb(call.name)
        val key = "${v.tally} ${v.many}"
        counts[key] = v to ((counts[key]?.second ?: 0) + 1)
    }
    return counts.values.joinToString(", ") { (v, n) -> "${v.tally} $n ${if (n == 1) v.one else v.many}" }.replaceFirstChar(Char::uppercase)
}

private fun transcriptText(messages: List<ThreadMessage>): String = messages.mapNotNull { m ->
    when (m.role) {
        "user" -> "> ${m.text}"
        "assistant" -> m.text.takeIf { it.isNotBlank() }
        "error" -> "! ${m.text}"
        else -> null
    }
}.joinToString("\n\n")

// ---- Drawing it ----

@Composable
private fun Conversation(pieces: List<ChatPiece>, doing: String?, asking: Boolean, chat: SessionChat, nav: Nav, online: Boolean) {
    val state = rememberLazyListState()
    // Newest at the bottom, and kept in view as it grows while the person reads the end.
    val shown = remember(pieces) { pieces.asReversed() }
    LaunchedEffect(pieces.size, doing, asking) { if (state.firstVisibleItemIndex <= 1) state.animateScrollToItem(0) }
    LazyColumn(Modifier.fillMaxSize(), state = state, reverseLayout = true, horizontalAlignment = Alignment.CenterHorizontally) {
        val cell = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 16.dp)
        item(key = "end") { Box(Modifier.height(12.dp)) }
        if (asking) item(key = "ask") { AskCard(chat, nav, online, cell.padding(top = 14.dp)) }
        if (doing != null) item(key = "doing") { WorkingRow(doing, cell.padding(top = 12.dp)) }
        items(shown, key = { it.key }) { p -> PieceView(p, cell, online) }
        item(key = "start") { Box(Modifier.height(8.dp)) }
    }
}

@Composable
private fun PieceView(p: ChatPiece, cell: Modifier, online: Boolean) {
    val c = Relay.colors
    when (p) {
        is ChatPiece.Day -> T(p.label, cell.padding(top = 18.dp, bottom = 2.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
        is ChatPiece.User -> UserBubble(p.text, p.sending, online, cell.padding(top = 18.dp))
        is ChatPiece.Reply -> SelectionContainer(cell.padding(top = 12.dp)) { Markdown(p.text) }
        is ChatPiece.Tool -> Box(cell.padding(top = 2.dp)) { ToolLine(p.call) }
        is ChatPiece.Steps -> Box(cell.padding(top = 2.dp)) { StepsView(p) }
        is ChatPiece.Note -> T(p.text, cell.padding(top = 14.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
        is ChatPiece.Error -> Row(cell.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph("close", 13.dp, c.heldText, Modifier.padding(top = 4.dp))
            SelectionContainer { T(p.text, style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp), color = c.heldText) }
        }
    }
}

/** The person's message: a bubble on the right, as a thread draws it. */
@Composable
private fun UserBubble(text: String, sending: Boolean, online: Boolean, modifier: Modifier) {
    val c = Relay.colors
    Column(modifier, horizontalAlignment = Alignment.End) {
        Box(Modifier.fillMaxWidth(.86f), contentAlignment = Alignment.CenterEnd) {
            SelectionContainer {
                Box(Modifier.clip(Radii.card).background(c.wash).padding(horizontal = 14.dp, vertical = 10.dp)) {
                    T(text, style = Relay.type.body, color = c.ink)
                }
            }
        }
        if (sending) Row(Modifier.padding(top = 5.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Dot(c.waiting, 5.dp)
            T(if (online) "Sent to the terminal" else "Waiting for the PC", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
        }
    }
}

@Composable
private fun WorkingRow(text: String, modifier: Modifier) {
    Row(modifier.heightIn(min = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(9.dp)) {
        WorkingLamp()
        T(text, Modifier.weight(1f), Relay.type.caption, Relay.colors.ink3, maxLines = 1)
    }
}

@Composable
private fun StepMark(step: Step, glyph: String) {
    val c = Relay.colors
    Box(Modifier.size(14.dp), contentAlignment = Alignment.Center) {
        when (step) {
            Step.Running -> Spinner(11.dp)
            Step.Done -> Glyph(glyph, 13.dp, c.inkDim)
            Step.Failed -> Glyph("close", 13.dp, c.heldText)
            Step.Cut -> Glyph("minus", 13.dp, c.inkDim)
        }
    }
}

/** A tool call as one quiet line, the command or file in mono; it opens to show the answer's head. */
@Composable
private fun ToolLine(call: Call) {
    val c = Relay.colors
    val v = verb(call.name)
    var open by rememberSaveable(call.key) { mutableStateOf(false) }
    val answer = call.answer?.trim().orEmpty()
    Column {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 30.dp).clip(Radii.icon).clickable(enabled = answer.isNotEmpty(), onClickLabel = if (open) "Hide the answer" else "Show the answer") { open = !open }.padding(end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(7.dp),
        ) {
            StepMark(call.step, v.glyph)
            T(v.did, style = Relay.type.caption, color = if (call.step == Step.Failed) c.heldText else c.ink3, maxLines = 1)
            if (call.summary.isNotBlank()) T(call.summary, Modifier.weight(1f), Relay.type.mono, c.ink2, maxLines = 1) else Box(Modifier.weight(1f))
            if (answer.isNotEmpty()) Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            SelectionContainer {
                Box(
                    Modifier.padding(start = 21.dp, top = 2.dp, bottom = 6.dp).fillMaxWidth().heightIn(max = 260.dp)
                        .clip(Radii.keycap).background(c.screen).border(1.dp, c.edge, Radii.keycap)
                        .verticalScroll(rememberScrollState()).padding(10.dp),
                ) {
                    T(answer, style = Relay.type.mono.copy(fontSize = 11.sp, lineHeight = 15.sp), color = if (call.step == Step.Failed) c.heldText else c.ink2)
                }
            }
        }
    }
}

/** Steps in a row: from three, one line counting them, opening to show each. */
@Composable
private fun StepsView(g: ChatPiece.Steps) {
    val c = Relay.colors
    var open by rememberSaveable(g.key) { mutableStateOf(false) }
    val step = when {
        g.calls.any { it.step == Step.Running } -> Step.Running
        g.calls.any { it.step == Step.Failed } -> Step.Failed
        else -> Step.Done
    }
    Column {
        Row(
            Modifier.heightIn(min = TOUCH).clip(Radii.icon).clickable(onClickLabel = if (open) "Fold the steps" else "Show each step") { open = !open }.padding(end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(7.dp),
        ) {
            StepMark(step, "check")
            T(stepsLine(g.calls), Modifier.weight(1f, fill = false), Relay.type.caption, if (step == Step.Failed) c.heldText else c.ink3, maxLines = 2)
            Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            Column(Modifier.padding(start = 2.dp, bottom = 4.dp)) { for (call in g.calls) ToolLine(call) }
        }
    }
}

// ---- The agent asking ----

/** A numbered choice on the agent's screen: "❯ 1. Yes". */
private val OPTION = Regex("""^\s*(?:[❯›>]\s*)?(\d)[.)]\s+(.+?)\s*$""")
private val FRAME = Regex("""[│┃╭╮╰╯─━┌┐└┘├┤]""")

/** The question at the bottom of the agent's screen and its numbered answers, frame lines taken out. */
internal fun askOf(screen: String): Pair<List<String>, List<Pair<String, String>>> {
    val lines = screen.lines().map { FRAME.replace(it, " ").trimEnd() }.dropLastWhile { it.isBlank() }
    val first = lines.indexOfFirst { OPTION.matches(it) }
    if (first < 0) return lines.filter { it.isNotBlank() }.takeLast(10).map { it.trim() } to emptyList()
    val options = lines.drop(first).mapNotNull { OPTION.matchEntire(it)?.destructured?.let { (n, label) -> n to label } }.distinctBy { it.first }
    val question = lines.take(first).filter { it.isNotBlank() }.takeLast(6).map { it.trim() }
    return question to options
}

/**
 * The agent is held at a question in its terminal: the question as its screen shows it, and a
 * key per answer. A number picks the answer as it would at the PC; Esc declines. The screen is
 * the terminal's own, streamed while the card shows: a TUI draws its prompt with cursor moves,
 * so only an emulator knows what it looks like.
 */
@Composable
private fun AskCard(chat: SessionChat, nav: Nav, online: Boolean, modifier: Modifier) {
    val c = Relay.colors
    val feed = rememberFeed(nav.relay, chat.name)
    val version by feed.version.collectAsStateWithLifecycle()
    val status by feed.status.collectAsStateWithLifecycle()
    val screen = remember(version, status) {
        if (status == TerminalFeed.Status.Connecting) null else synchronized(feed.lock) { feed.terminal.plainText(feed.terminal.rows) }
    }
    val (question, options) = remember(screen) { askOf(screen.orEmpty()) }
    Column(
        modifier.clip(Radii.card).background(c.slab).border(1.dp, c.held.copy(alpha = .45f), Radii.card).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dot(c.held, 6.dp)
            T("Waiting for you", style = Relay.type.ui, color = c.heldText, weight = androidx.compose.ui.text.font.FontWeight.Medium)
        }
        if (screen == null) T("Reading the agent's screen…", style = Relay.type.caption, color = c.ink3)
        else if (question.isNotEmpty()) T(question.joinToString("\n"), style = Relay.type.mono.copy(fontSize = 11.5.sp, lineHeight = 16.sp), color = c.ink2)
        if (options.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((n, label) in options) {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = TOUCH).alpha(if (online) 1f else .5f).clip(Radii.keycap).background(c.wash)
                            .clickable(enabled = online, role = Role.Button, onClickLabel = label) { chat.key(n) }.padding(horizontal = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        T(n, style = Relay.type.mono, color = c.ink3)
                        T(label, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 2)
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (options.isEmpty()) {
                Key("Yes", { chat.key("y") }, enabled = online, compact = true)
                Key("No", { chat.key("n") }, enabled = online, compact = true)
                Key("Enter", { chat.key(Keys.ENTER) }, enabled = online, compact = true)
            }
            Key("Esc", { chat.key(Keys.ESC) }, kind = KeyKind.Quiet, enabled = online, compact = true)
        }
    }
}

// ---- Keys and composer ----

private class ChatKey(val label: String, val spoken: String, val data: String)

/** The keys an agent CLI still asks for in a conversation: stop, the mode switch, a menu's arrows. */
private val CHAT_KEYS = listOf(
    ChatKey("Esc", "Escape", Keys.ESC),
    ChatKey("⇧Tab", "Shift Tab: switch mode", Keys.SHIFT_TAB),
    ChatKey("↑", "Up", "\u001b[A"),
    ChatKey("↓", "Down", "\u001b[B"),
    ChatKey("⏎", "Enter", Keys.ENTER),
    ChatKey("/", "Slash", "/"),
    ChatKey("^C", "Control C", Keys.CTRL_C),
)

@Composable
private fun QuickKeys(chat: SessionChat, enabled: Boolean) {
    val c = Relay.colors
    val face = remember { TextStyle(fontFamily = Fonts.FiraMono, fontSize = 13.sp) }
    Row(
        Modifier.fillMaxWidth().background(c.wall).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        for (k in CHAT_KEYS) {
            Box(
                Modifier.height(32.dp).widthIn(min = 40.dp).alpha(if (enabled) 1f else .4f).clip(Radii.keycap).background(c.slab).border(1.dp, c.edge, Radii.keycap)
                    .clickable(enabled = enabled, onClickLabel = k.spoken, role = Role.Button) { chat.key(k.data) }.padding(horizontal = 9.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(k.label, style = face, color = c.ink2, maxLines = 1)
            }
        }
    }
}

/** The message box: up to six lines; the round key sends, or with nothing typed stops a working agent. */
@Composable
private fun ChatComposer(text: String, onText: (String) -> Unit, enabled: Boolean, working: Boolean, placeholder: String, focus: FocusRequester, onSend: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding().padding(start = 10.dp, end = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Field(
            text,
            onText,
            Modifier.weight(1f).alpha(if (enabled) 1f else .6f).focusRequester(focus).focusProperties { canFocus = enabled },
            placeholder = placeholder,
            singleLine = false,
            maxLines = 6,
            style = Relay.type.body,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
        )
        val stop = working && text.isBlank()
        Box(
            Modifier.size(44.dp).alpha(if (enabled) 1f else .4f).clip(CircleShape).background(c.ink)
                .clickable(enabled = enabled, onClickLabel = if (stop) "Stop" else if (text.isBlank()) "Enter" else "Send", role = Role.Button, onClick = onSend),
            contentAlignment = Alignment.Center,
        ) {
            when {
                stop -> Glyph("stop", 16.dp, c.wall)
                text.isBlank() -> ReturnGlyph(18.dp, c.wall)
                else -> Glyph("arrow-up", 18.dp, c.wall)
            }
        }
    }
}
