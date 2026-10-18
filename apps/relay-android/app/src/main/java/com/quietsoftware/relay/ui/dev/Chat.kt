package com.quietsoftware.relay.ui.dev

import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.ThreadMessage
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.data.SessionChat
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.code.ADD_TEXT
import com.quietsoftware.relay.ui.code.ADD_WASH
import com.quietsoftware.relay.ui.code.DEL_TEXT
import com.quietsoftware.relay.ui.code.DEL_WASH
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.term.TerminalView
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.Spinner
import com.quietsoftware.relay.ui.threads.WorkingLamp
import com.quietsoftware.relay.ui.threads.dayOf
import com.quietsoftware.relay.ui.threads.humanDay
import com.quietsoftware.relay.ui.threads.said
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.conflate
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.LocalDate

/**
 * One terminal agent as a conversation, drawn as a thread is: the person's messages, the agent's
 * words and reasoning, its edits as diffs, and its other steps as quiet lines that open to show
 * the whole command and the answer. Typing goes to the agent's terminal through the PC, as on the
 * terminal screen. The agent's screen is followed while it runs, so whenever it shows a menu (a
 * permission, a question, a plan to approve, a slash command's picker) the menu is answered here
 * with keys, whatever the hooks reported; the screen itself opens under the conversation from the
 * header. The terminal screen stays one tap away, and nothing here needs it opened first.
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
    var peek by rememberSaveable(name) { mutableStateOf(false) }
    // A value, not a string: a picked command puts the cursor after itself.
    var field by rememberSaveable(name, stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue()) }
    val text = field.text
    fun type(next: String) {
        field = TextFieldValue(next, TextRange(next.length))
    }
    // Pictures for the next message: each sent to the agent's checkout as soon as it is picked.
    val context = LocalContext.current
    val attached = remember(name) { mutableStateListOf<Attachment>() }
    fun update(id: Long, change: (Attachment) -> Attachment) {
        val i = attached.indexOfFirst { it.id == id }
        if (i >= 0) attached[i] = change(attached[i])
    }
    // Dictation through the phone's own recognizer: what was said joins what is typed.
    val dictate = rememberDictation { said -> type(joinSaid(field.text, said)) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(MAX_PICTURES)) { uris ->
        for (uri in uris.take(MAX_PICTURES - attached.size)) {
            val id = System.nanoTime()
            attached += Attachment(id, null)
            scope.launch {
                val shrunk = withContext(Dispatchers.IO) { shrink(context, uri) }
                if (shrunk == null) return@launch update(id) { it.copy(failed = true) }
                update(id) { it.copy(thumb = shrunk.second) }
                val path = chat.upload(shrunk.first)
                update(id) { if (path != null) it.copy(path = path) else it.copy(failed = true) }
            }
        }
    }

    val ready = session?.attachable == true && s.online
    val working = session?.state == "running" || session?.state == "spawning"
    val held = session?.state == "blocked"
    // Held at a prompt, the turn is still open: its last call waits on the answer, it was not cut off.
    val pieces = remember(messages, sending, working, held) { chatPieces(messages, sending, working || held, LocalDate.now()) }
    val askTool = remember(pieces) { waitsOnAsk(pieces) }

    // The agent's own screen, followed while it can be answered: a menu on it is a question even
    // when no hook said so (a slash command's picker leaves the agent "running"). Only the cards
    // that draw it read the text, so a redraw on the PC does not recompose the conversation.
    val screen = remember(name) { mutableStateOf<String?>(null) }
    if (ready) WatchScreen(nav, name, screen)
    val onScreen by remember(screen) { derivedStateOf { screen.value?.let(::screenAsks) == true } }
    val unread by remember(screen) { derivedStateOf { screen.value == null } }
    val asking = ready && (onScreen || askTool || (held && unread))
    val doing = remember(pieces, working, asking) { if (working && !asking) doingNow(pieces) else null }
    val mode by remember(screen) { derivedStateOf { screen.value?.let(::modeOf) } }
    // When the turn the agent is working on began: the person's last message.
    val since = remember(messages, sending) { epoch((sending.lastOrNull() ?: messages.lastOrNull { it.role == "user" })?.createdAt).takeIf { it > 0 } }

    KeepScreenOn(s.prefs.keepScreenOn)

    fun send() {
        val typed = text.trim()
        if (attached.any { it.sending }) {
            nav.shell.toast("A picture is still on its way to the PC")
            return
        }
        if (attached.any { it.failed }) {
            nav.shell.toast("A picture did not reach the PC: take it out, or pick it again")
            return
        }
        val pictures = attached.toList()
        val paths = pictures.mapNotNull { it.path }
        if (typed.isEmpty() && paths.isEmpty()) {
            // Empty: Esc stops an agent mid-turn; otherwise (a menu up, a prompt) it is the Enter it waits for.
            val stop = working && !asking
            chat.key(if (stop) Keys.ESC else Keys.ENTER)
            if (stop) nav.shell.toast("Asked ${session?.provider ?: "the agent"} to stop")
            return
        }
        type("")
        attached.clear()
        scope.launch {
            if (!chat.send(withAttachments(typed, paths))) {
                if (field.text.isEmpty()) type(typed)
                if (attached.isEmpty()) attached.addAll(pictures)
                nav.shell.toast("Not sent: the PC did not take it")
            }
        }
    }

    fun resend(again: String) {
        scope.launch { if (!chat.send(again)) nav.shell.toast("Not sent: the PC did not take it") }
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            Header(session, name, onBack = nav::back, onDetails = { nav.session(name) }, onMenu = { menu = true }) {
                if (ready) IconKey("focus", { peek = !peek }, size = 18.dp, tint = if (peek) c.ink else c.ink2)
                IconKey("terminal", { nav.console(name) }, size = 18.dp, tint = c.ink2)
            }
            Banner(status, session, s.online, life)
            // The agent's plan, pinned: what it is on, and how far along, while there is any left.
            val plan = remember(messages) { latestPlan(messages) }
            if (plan != null && (working || plan.any { it.state != 'x' })) PlanStrip(name, plan)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                // The agent's screen here before the terminal: it answers what the conversation cannot show yet.
                val openTerminal = @Composable {
                    if (ready && !peek) Key("Show its screen", { peek = true }, glyph = "focus", compact = true)
                    else Key("Open the terminal", { nav.console(name) }, glyph = "terminal", compact = true)
                }
                when {
                    pieces.isNotEmpty() || asking -> Conversation(
                        pieces, doing, since, asking, screen, chat, s.online,
                        onEdit = { words ->
                            type(words)
                            focus.requestFocus()
                        },
                        onResend = if (ready) ::resend else null,
                    )
                    error != null && status != TerminalFeed.Status.Saved -> Empty(
                        "terminal", "Can't read the conversation",
                        said(error!!.error, "The PC's Relay is older than this app: update it to see agents as a conversation. The terminal works meanwhile."),
                        action = openTerminal,
                    )
                    found == false -> Empty(
                        "terminal", "Nothing said yet",
                        "The conversation shows here once ${session?.provider ?: "the agent"} starts. Its screen shows everything meanwhile.",
                        action = openTerminal,
                    )
                    status == TerminalFeed.Status.Connecting -> Empty("terminal", "Reading the conversation")
                    else -> Empty("terminal", "Nothing said yet", if (s.online) null else "The PC is away, and this phone has not read this agent before.", action = openTerminal)
                }
            }
            if (peek && ready) ScreenPanel(nav, name, screen) { peek = false }
            if (s.prefs.chatKeys) QuickKeys(chat, ready)
            val asked = commandAsked(text)
            if (asked != null && ready) Commands(nav, session?.provider, session?.projectId, asked) { type("/$it ") }
            ChatComposer(
                field,
                { field = it },
                ready,
                working && !asking,
                placeholder = when {
                    session == null -> "This agent is closed"
                    !s.online -> "Needs the PC to talk to the agent"
                    !ready -> "The agent is not running"
                    else -> "Message ${session?.name ?: "the agent"}"
                },
                focus,
                ::send,
                pictures = attached.isNotEmpty(),
                above = { AttachmentStrip(attached) { id -> attached.removeAll { it.id == id } } },
            ) {
                Chip("", "Attach a photo or a screenshot", ready && attached.size < MAX_PICTURES, glyph = "file-image") {
                    picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                }
                if (dictate != null) Chip("", "Dictate a message", ready, icon = { MicGlyph(15.dp, c.ink2) }, onClick = dictate)
                Chip("/", "The agent's commands", ready) {
                    if (text.isBlank()) type("/")
                    focus.requestFocus()
                }
                Chip(mode ?: "Mode", "Shift Tab: the agent's next mode", ready, glyph = "shield") { chat.key(Keys.SHIFT_TAB) }
                Chip("Esc", "Escape", ready) { chat.key(Keys.ESC) }
                Chip("Keys", if (s.prefs.chatKeys) "Hide the extra keys" else "Show the extra keys", true, trailing = if (s.prefs.chatKeys) "chevron-down" else "chevron-up") {
                    nav.shell.update { it.copy(chatKeys = !it.chatKeys) }
                }
            }
        }
    }

    LifecycleSheets(life)
    if (menu) {
        DevSheet({ menu = false }, name, session?.let { "${it.provider} · ${it.role}" }) { close ->
            if (ready) SheetRow(if (peek) "Hide the agent's screen" else "Show the agent's screen", "focus", { close { peek = !peek } })
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

/** [detail] is what the one-line summary leaves out: an edit's `-`/`+` lines, the whole command, the questions. */
internal data class Call(val key: String, val name: String, val summary: String, val step: Step, val answer: String?, val detail: String? = null)

/** A call that changes files: drawn with its diff, never folded among the steps. */
internal fun isEdit(name: String) = name == "Edit" || name == "MultiEdit" || name == "Write" || name == "NotebookEdit" || name == "apply_patch"

/** A call that asks the person something: its question or plan is shown, not folded. */
internal fun isAsk(name: String) = name == "AskUserQuestion" || name == "ExitPlanMode" || name == "request_user_input"

internal sealed interface ChatPiece {
    val key: String

    data class Day(override val key: String, val label: String) : ChatPiece
    data class User(override val key: String, val text: String, val sending: Boolean) : ChatPiece
    data class Reply(override val key: String, val text: String) : ChatPiece
    /** The model's reasoning, where its provider logged it in words. */
    data class Thought(override val key: String, val text: String) : ChatPiece
    data class Tool(override val key: String, val call: Call) : ChatPiece
    /** Three or more steps in a row, folded under one line: "Ran 3 commands, read 2 files". */
    data class Steps(override val key: String, val calls: List<Call>) : ChatPiece
    data class Note(override val key: String, val text: String) : ChatPiece
    data class Error(override val key: String, val text: String) : ChatPiece
    /** The end of a finished turn: a quiet Copy key for the agent's words in it. */
    data class Copy(override val key: String, val text: String) : ChatPiece
}

private fun JsonObject.str(key: String) = (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content.orEmpty()

/** [working]: the agent's turn is still open (running, or held at a prompt), so its last unanswered calls are waiting, not cut off. */
internal fun chatPieces(messages: List<ThreadMessage>, sending: List<ThreadMessage>, working: Boolean, today: LocalDate): List<ChatPiece> {
    val answers = HashMap<String, Pair<Boolean, String>>()
    for (m in messages) if (m.role == "tool") {
        val b = m.body as? JsonObject ?: continue
        answers[b.str("tool_use_id")] = ((b["is_error"] as? JsonPrimitive)?.booleanOrNull == true) to b.str("text")
    }
    val lastUser = messages.indexOfLast { it.role == "user" }
    val out = ArrayList<ChatPiece>()
    val steps = ArrayList<Call>()
    // The agent's words since the person last spoke: copied together at the end of the turn.
    val said = ArrayList<String>()
    var turn = ""
    var lastDay: LocalDate? = null
    fun flush() {
        if (steps.size >= 3) out += ChatPiece.Steps("g:" + steps.first().key, steps.toList())
        else steps.forEach { out += ChatPiece.Tool(it.key, it) }
        steps.clear()
    }
    fun endTurn() {
        if (said.isNotEmpty()) out += ChatPiece.Copy("c:$turn", said.joinToString("\n\n"))
        said.clear()
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
        // The turn before ends (its Copy key) ahead of the day the next one starts on.
        if (m.role == "user") {
            flush()
            endTurn()
        }
        day(m)
        when (m.role) {
            "user" -> {
                turn = m.id
                out += ChatPiece.User("m:${m.id}", m.text, sending = false)
            }
            "assistant" -> ((m.body as? JsonObject)?.get("blocks") as? JsonArray).orEmpty().forEachIndexed { i, blk ->
                val o = blk as? JsonObject ?: return@forEachIndexed
                when (o.str("type")) {
                    "text" -> o.str("text").takeIf { it.isNotBlank() }?.let {
                        flush()
                        out += ChatPiece.Reply("m:${m.id}:$i", it.trim())
                        said += it.trim()
                    }
                    "thinking" -> o.str("text").takeIf { it.isNotBlank() }?.let {
                        flush()
                        out += ChatPiece.Thought("m:${m.id}:$i", it.trim())
                    }
                    "tool_use" -> {
                        val id = o.str("id")
                        val answer = answers[id]
                        val step = when {
                            answer != null -> if (answer.first) Step.Failed else Step.Done
                            working && index > lastUser -> Step.Running
                            else -> Step.Cut
                        }
                        val call = Call("t:${id.ifEmpty { "${m.id}:$i" }}", o.str("name"), o.str("summary"), step, answer?.second, o.str("detail").ifBlank { null })
                        if (isEdit(call.name) || isAsk(call.name)) {
                            flush()
                            out += ChatPiece.Tool(call.key, call)
                        } else {
                            steps += call
                        }
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
    if (!working || sending.isNotEmpty()) endTurn()
    for (m in sending) out += ChatPiece.User("m:${m.id}", m.text, sending = true)
    return out
}

/** A step of the agent's plan: its words, and `x` done, `~` under way or ` ` to do. */
internal data class PlanStep(val text: String, val state: Char)

private val PLAN_LINE = Regex("""^\[([x~ ])] (.*)$""")

/** The agent's latest plan (Claude Code's to-do list, Codex's plan), from its log; null while it has none. */
internal fun latestPlan(messages: List<ThreadMessage>): List<PlanStep>? {
    for (m in messages.asReversed()) {
        if (m.role != "assistant") continue
        val blocks = (m.body as? JsonObject)?.get("blocks") as? JsonArray ?: continue
        for (b in blocks.asReversed()) {
            val o = b as? JsonObject ?: continue
            if (o.str("type") != "tool_use" || o.str("name") !in PLAN_TOOLS) continue
            return o.str("detail").lines().mapNotNull { line ->
                PLAN_LINE.matchEntire(line.trimEnd())?.let { PlanStep(it.groupValues[2], it.groupValues[1][0]) }
            }.takeIf { it.isNotEmpty() }
        }
    }
    return null
}

private val PLAN_TOOLS = setOf("TodoWrite", "update_plan")

/** The agent's last step is a question or a plan still waiting on the person. */
internal fun waitsOnAsk(pieces: List<ChatPiece>): Boolean =
    (pieces.lastOrNull { it !is ChatPiece.User && it !is ChatPiece.Copy } as? ChatPiece.Tool)?.call?.let { isAsk(it.name) && it.step == Step.Running } == true

/** An edit's size: the lines it put in and took out. */
internal fun diffCounts(lines: List<String>): Pair<Int, Int> =
    lines.count { it.startsWith("+") && !it.startsWith("+++") } to lines.count { it.startsWith("-") && !it.startsWith("---") }

/**
 * What the working row says: the step running now, in words, when it is folded out of sight in a
 * group of steps; a step on a line of its own already says so there.
 */
internal fun doingNow(pieces: List<ChatPiece>): String {
    val call = (pieces.lastOrNull() as? ChatPiece.Steps)?.calls?.last()?.takeIf { it.step == Step.Running } ?: return "Working…"
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
    name == "AskUserQuestion" || name == "request_user_input" -> Verb("Asked you", "Asking you", "asked", "question", "questions", "user")
    name == "ExitPlanMode" -> Verb("Proposed a plan", "Proposing a plan", "proposed", "plan", "plans", "brief")
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
private fun Conversation(
    pieces: List<ChatPiece>,
    doing: String?,
    since: Long?,
    asking: Boolean,
    screen: State<String?>,
    chat: SessionChat,
    online: Boolean,
    onEdit: (String) -> Unit,
    onResend: ((String) -> Unit)?,
) {
    val c = Relay.colors
    val state = rememberLazyListState()
    val scope = rememberCoroutineScope()
    // Newest at the bottom, and kept in view as it grows while the person reads the end.
    val shown = remember(pieces) { pieces.asReversed() }
    LaunchedEffect(pieces.size, doing, asking) { if (state.firstVisibleItemIndex <= 1) state.animateScrollToItem(0) }
    // Scrolled up to read: a key back to the newest, counting what arrived meanwhile.
    val away by remember { derivedStateOf { state.firstVisibleItemIndex > 2 } }
    var seen by remember { mutableIntStateOf(0) }
    LaunchedEffect(away) { if (away) seen = pieces.size }
    val fresh = if (away) (pieces.size - seen).coerceAtLeast(0) else 0
    // The one message of the person's showing its keys, if any.
    var opened by remember { mutableStateOf<String?>(null) }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize(), state = state, reverseLayout = true, horizontalAlignment = Alignment.CenterHorizontally) {
            val cell = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 16.dp)
            item(key = "end") { Box(Modifier.height(12.dp)) }
            if (asking) item(key = "ask") { AskCard(chat, screen, online, cell.padding(top = 14.dp)) }
            if (doing != null) item(key = "doing") { WorkingRow(doing, since, cell.padding(top = 12.dp)) }
            items(shown, key = { it.key }, contentType = { it::class }) { p ->
                if (p is ChatPiece.User) UserBubble(
                    p.text,
                    p.sending,
                    online,
                    cell.padding(top = 18.dp),
                    open = opened == p.key,
                    onOpen = { opened = if (opened == p.key) null else p.key },
                    onEdit = { opened = null; onEdit(it) },
                    onResend = onResend?.takeIf { !p.sending }?.let { r -> { again -> opened = null; r(again) } },
                )
                else PieceView(p, cell, online)
            }
            item(key = "start") { Box(Modifier.height(8.dp)) }
        }
        AnimatedVisibility(away, Modifier.align(Alignment.BottomCenter).padding(bottom = 10.dp), enter = fadeIn(), exit = fadeOut()) {
            Row(
                Modifier.heightIn(min = TOUCH).clip(Radii.pill).clickable(onClickLabel = "Go to the newest", role = Role.Button) { scope.launch { state.animateScrollToItem(0) } },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Row(
                    Modifier.height(36.dp).shadow(10.dp, Radii.pill).clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill)
                        .padding(horizontal = if (fresh > 0) 14.dp else 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Glyph("arrow-down", 16.dp, c.ink2)
                    if (fresh > 0) T(if (fresh == 1) "1 new" else "$fresh new", style = Relay.type.caption, color = c.ink2, weight = FontWeight.Medium)
                }
            }
        }
    }
}

@Composable
private fun PieceView(p: ChatPiece, cell: Modifier, online: Boolean) {
    val c = Relay.colors
    when (p) {
        is ChatPiece.Day -> T(p.label, cell.padding(top = 18.dp, bottom = 2.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
        is ChatPiece.User -> UserBubble(p.text, p.sending, online, cell.padding(top = 18.dp))
        is ChatPiece.Reply -> SelectionContainer(cell.padding(top = 12.dp)) { Markdown(p.text) }
        is ChatPiece.Thought -> Box(cell.padding(top = 10.dp, bottom = 2.dp)) { ThoughtView(p) }
        is ChatPiece.Tool -> Box(cell.padding(top = 2.dp)) {
            when {
                isEdit(p.call.name) -> EditView(p.call)
                isAsk(p.call.name) -> ToolLine(p.call, startOpen = true)
                else -> ToolLine(p.call)
            }
        }
        is ChatPiece.Steps -> Box(cell.padding(top = 2.dp)) { StepsView(p) }
        is ChatPiece.Note -> T(p.text, cell.padding(top = 14.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
        is ChatPiece.Error -> Row(cell.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph("close", 13.dp, c.heldText, Modifier.padding(top = 4.dp))
            SelectionContainer { T(p.text, style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp), color = c.heldText) }
        }
        is ChatPiece.Copy -> Box(cell) { CopyKey(p.text) }
    }
}

/** A finished turn's Copy key: the agent's words in it, as Markdown. */
@Composable
private fun CopyKey(text: String) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var copied by remember { mutableStateOf(false) }
    Box(
        Modifier.size(TOUCH).clip(Radii.icon).clickable(onClickLabel = "Copy the reply", role = Role.Button) {
            scope.launch {
                clipboard.copy(text)
                copied = true
                delay(1400)
                copied = false
            }
        },
        contentAlignment = Alignment.CenterStart,
    ) {
        Glyph(if (copied) "check" else "copy", 15.dp, Relay.colors.inkDim)
    }
}

/**
 * The agent's plan under the header: one line with how far along it is and the step under way,
 * opening to every step, done ones ticked and quiet.
 */
@Composable
private fun PlanStrip(name: String, plan: List<PlanStep>) {
    val c = Relay.colors
    var open by rememberSaveable(name) { mutableStateOf(false) }
    val done = plan.count { it.state == 'x' }
    val now = plan.firstOrNull { it.state == '~' } ?: plan.firstOrNull { it.state != 'x' }
    Column(Modifier.fillMaxWidth().background(c.slab)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 40.dp).clickable(onClickLabel = if (open) "Fold the plan" else "Show the whole plan") { open = !open }
                .padding(horizontal = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Glyph("check", 13.dp, if (done == plan.size) c.live else c.ink3)
            T("Plan", style = Relay.type.caption, color = c.ink2, weight = FontWeight.Medium)
            T("$done of ${plan.size}", style = Relay.type.mono.copy(fontSize = 11.sp), color = c.ink3)
            T(now?.text.orEmpty(), Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
            Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            Column(
                Modifier.fillMaxWidth().heightIn(max = 260.dp).verticalScroll(rememberScrollState()).padding(start = 14.dp, end = 14.dp, bottom = 10.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                for (step in plan) {
                    Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(9.dp)) {
                        Box(Modifier.padding(top = 3.dp).size(13.dp), contentAlignment = Alignment.Center) {
                            when (step.state) {
                                'x' -> Glyph("check", 12.dp, c.live)
                                '~' -> Dot(c.waiting, 7.dp)
                                else -> Box(Modifier.size(9.dp).border(1.dp, c.inkDim, CircleShape))
                            }
                        }
                        T(
                            step.text,
                            Modifier.weight(1f),
                            Relay.type.caption.copy(textDecoration = if (step.state == 'x') TextDecoration.LineThrough else null),
                            if (step.state == 'x') c.ink3 else c.ink2,
                        )
                    }
                }
            }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
    }
}

/**
 * The person's message: a bubble on the right, as a thread draws it. A tap or a long press shows
 * its keys under it: Copy, Edit (back into the message box) and Send again.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun UserBubble(
    text: String,
    sending: Boolean,
    online: Boolean,
    modifier: Modifier,
    open: Boolean = false,
    onOpen: (() -> Unit)? = null,
    onEdit: ((String) -> Unit)? = null,
    onResend: ((String) -> Unit)? = null,
) {
    val c = Relay.colors
    val (words, pictures) = remember(text) { splitAttached(text) }
    val haptic = LocalHapticFeedback.current
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    val touch = if (onOpen == null) Modifier else Modifier.combinedClickable(
        onClickLabel = if (open) "Hide this message's keys" else "Show this message's keys",
        onLongClick = {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            if (!open) onOpen()
        },
        onClick = onOpen,
    )
    Column(modifier, horizontalAlignment = Alignment.End) {
        if (pictures.isNotEmpty()) Row(Modifier.padding(bottom = 6.dp).clip(Radii.pill).then(if (words.isEmpty()) touch else Modifier), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for (p in pictures) AttachedPill(p)
        }
        if (words.isNotEmpty()) Box(Modifier.fillMaxWidth(.86f), contentAlignment = Alignment.CenterEnd) {
            Box(
                Modifier.clip(Radii.card).background(c.wash).then(if (open) Modifier.border(1.dp, c.strong, Radii.card) else Modifier).then(touch)
                    .padding(horizontal = 14.dp, vertical = 10.dp),
            ) {
                T(words, style = Relay.type.body, color = c.ink)
            }
        }
        if (sending) Row(Modifier.padding(top = 5.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Dot(c.waiting, 5.dp)
            T(if (online) "Sent to the terminal" else "Waiting for the PC", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
        }
        AnimatedVisibility(open, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (words.isNotEmpty()) {
                    Chip("Copy", "Copy this message", true, glyph = "copy") {
                        scope.launch { clipboard.copy(words) }
                        onOpen?.invoke()
                    }
                    onEdit?.let { edit -> Chip("Edit", "Edit it in the message box", true, glyph = "edit") { edit(words) } }
                }
                onResend?.let { again -> Chip("Send again", "Send this message again", true, glyph = "refresh") { again(text) } }
            }
        }
    }
}

/** The working row: the lamp, the step running now, and how long the turn has run, as Claude Code counts it. */
@Composable
private fun WorkingRow(text: String, since: Long?, modifier: Modifier) {
    val c = Relay.colors
    val now by produceState(System.currentTimeMillis(), since) {
        while (since != null) {
            delay(1_000)
            value = System.currentTimeMillis()
        }
    }
    val took = since?.let { took(now - it) }
    Row(modifier.heightIn(min = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(9.dp)) {
        WorkingLamp()
        T(text, Modifier.weight(1f, fill = false), Relay.type.caption, c.ink3, maxLines = 1)
        if (took != null) T(took, style = Relay.type.mono.copy(fontSize = 11.sp), color = c.inkDim, maxLines = 1)
    }
}

/** "42s", "3m 12s", "1h 4m": a turn's time so far; nothing for one that cannot be this turn's. */
internal fun took(ms: Long): String? {
    if (ms < 0 || ms > TURN_MAX_MS) return null
    val s = ms / 1000
    return when {
        s < 60 -> "${s}s"
        s < 3600 -> "${s / 60}m ${s % 60}s"
        else -> "${s / 3600}h ${s % 3600 / 60}m"
    }
}

private const val TURN_MAX_MS = 12 * 3_600_000L

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

/**
 * A tool call as one quiet line, the command or file in mono. It opens to show what the line
 * leaves out (the whole command, the questions asked, the plan) and the answer's head.
 */
@Composable
private fun ToolLine(call: Call, startOpen: Boolean = false) {
    val c = Relay.colors
    val v = verb(call.name)
    var open by rememberSaveable(call.key) { mutableStateOf(startOpen) }
    val answer = call.answer?.trim().orEmpty()
    val detail = call.detail?.trim().orEmpty()
    val more = answer.isNotEmpty() || detail.isNotEmpty()
    Column {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 30.dp).clip(Radii.icon).clickable(enabled = more, onClickLabel = if (open) "Fold the step" else "Show the step") { open = !open }.padding(end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(7.dp),
        ) {
            StepMark(call.step, v.glyph)
            // Running, it says what the agent is doing; after, what it did.
            T(if (call.step == Step.Running) v.doing else v.did, style = Relay.type.caption, color = if (call.step == Step.Failed) c.heldText else c.ink3, maxLines = 1)
            if (call.summary.isNotBlank()) T(call.summary, Modifier.weight(1f), Relay.type.mono, c.ink2, maxLines = 1) else Box(Modifier.weight(1f))
            if (more) Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            Column(Modifier.padding(start = 21.dp, top = 2.dp, bottom = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (detail.isNotEmpty()) {
                    if (call.name == "ExitPlanMode") {
                        Box(Modifier.fillMaxWidth().clip(Radii.keycap).border(1.dp, c.edge, Radii.keycap).padding(12.dp)) { Markdown(detail, style = Relay.type.ui) }
                    } else {
                        CodeBox(detail, c.ink, maxHeight = 320.dp)
                    }
                }
                if (answer.isNotEmpty()) CodeBox(answer, if (call.step == Step.Failed) c.heldText else c.ink2, maxHeight = 260.dp)
            }
        }
    }
}

/** Monospaced text on the console's black, wrapped to the phone's width, scrolling past [maxHeight]. */
@Composable
private fun CodeBox(text: String, color: Color, maxHeight: Dp) {
    val c = Relay.colors
    SelectionContainer {
        Box(
            Modifier.fillMaxWidth().heightIn(max = maxHeight)
                .clip(Radii.keycap).background(c.screen).border(1.dp, c.edge, Radii.keycap)
                .verticalScroll(rememberScrollState()).padding(10.dp),
        ) {
            T(text, style = Relay.type.mono.copy(fontSize = 11.sp, lineHeight = 15.sp), color = color)
        }
    }
}

/** Reasoning: a few lines in quiet italics beside a rule, opening to all of it. */
@Composable
private fun ThoughtView(p: ChatPiece.Thought) {
    val c = Relay.colors
    var open by rememberSaveable(p.key) { mutableStateOf(false) }
    val long = remember(p.text) { p.text.length > THOUGHT_PREVIEW_CHARS || p.text.lines().size > THOUGHT_PREVIEW_LINES }
    val style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp, fontStyle = FontStyle.Italic)
    val rule = c.edge
    Column(
        Modifier.fillMaxWidth()
            .drawBehind { drawRect(rule, size = Size(2.dp.toPx(), size.height)) }
            .clip(Radii.icon)
            .clickable(enabled = long, onClickLabel = if (open) "Fold the reasoning" else "Show all the reasoning") { open = !open }
            .padding(start = 12.dp, end = 6.dp, top = 2.dp, bottom = 2.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            T("Thinking", Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
            if (long) Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        if (open || !long) Markdown(p.text, style = style, color = c.ink3)
        else T(p.text, style = style, color = c.ink3, maxLines = THOUGHT_PREVIEW_LINES)
    }
}

private const val THOUGHT_PREVIEW_LINES = 3
private const val THOUGHT_PREVIEW_CHARS = 240
private const val EDIT_PREVIEW_LINES = 12

/**
 * An edit as the PC's Git panel draws one, shown rather than folded: the file and its size on one
 * line, the lines taken out in red and put in in green, wrapped to the phone, the first dozen
 * with the rest a tap away. A failed edit says why underneath.
 */
@Composable
private fun EditView(call: Call) {
    val c = Relay.colors
    val v = verb(call.name)
    val lines = remember(call.detail) { call.detail?.trimEnd()?.lines().orEmpty() }
    val (added, removed) = remember(lines) { diffCounts(lines) }
    var all by rememberSaveable(call.key) { mutableStateOf(false) }
    val answer = call.answer?.trim().orEmpty()
    Column(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 30.dp).padding(end = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
            StepMark(call.step, v.glyph)
            T(if (call.step == Step.Running) v.doing else v.did, style = Relay.type.caption, color = if (call.step == Step.Failed) c.heldText else c.ink3, maxLines = 1)
            T(call.summary, Modifier.weight(1f), Relay.type.mono, c.ink2, maxLines = 1)
            if (added > 0) T("+$added", style = Relay.type.mono.copy(fontSize = 11.sp), color = ADD_TEXT, maxLines = 1)
            if (removed > 0) T("−$removed", style = Relay.type.mono.copy(fontSize = 11.sp), color = DEL_TEXT, maxLines = 1)
        }
        if (lines.isNotEmpty()) {
            val shown = if (all) lines else lines.take(EDIT_PREVIEW_LINES)
            SelectionContainer(Modifier.padding(start = 21.dp, top = 2.dp)) {
                Column(Modifier.fillMaxWidth().clip(Radii.keycap).background(c.screen).border(1.dp, c.edge, Radii.keycap).padding(vertical = 6.dp)) {
                    for (line in shown) DiffText(line)
                }
            }
            if (lines.size > EDIT_PREVIEW_LINES) {
                T(
                    if (all) "Show less" else "Show all ${lines.size} lines",
                    Modifier.padding(start = 21.dp).heightIn(min = TOUCH).clip(Radii.icon).clickable(role = Role.Button) { all = !all }.padding(horizontal = 4.dp, vertical = 12.dp),
                    Relay.type.caption,
                    c.ink2,
                )
            }
        }
        if (call.step == Step.Failed && answer.isNotEmpty()) {
            T(answer, Modifier.padding(start = 21.dp, top = 4.dp), Relay.type.caption, c.heldText, maxLines = 6)
        }
    }
}

@Composable
private fun DiffText(line: String) {
    val c = Relay.colors
    val (ink, wash) = when {
        line.startsWith("@@") || line.startsWith("***") -> c.ink3 to Color.Transparent
        line.startsWith("+") -> ADD_TEXT to ADD_WASH
        line.startsWith("-") -> DEL_TEXT to DEL_WASH
        line.startsWith("…") -> c.ink3 to Color.Transparent
        else -> c.ink2 to Color.Transparent
    }
    T(line, Modifier.fillMaxWidth().background(wash).padding(horizontal = 10.dp, vertical = 1.dp), Relay.type.mono.copy(fontSize = 11.sp, lineHeight = 15.sp), ink)
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
            T(stepsLine(g.calls), Modifier.weight(1f, fill = false), Relay.type.caption, c.ink3, maxLines = 2)
            val failed = g.calls.count { it.step == Step.Failed }
            if (failed > 0) T("· $failed failed", style = Relay.type.caption, color = c.heldText, maxLines = 1)
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

/** A menu's highlighted choice, as Claude Code (❯) and Codex (›) mark it: what tells a menu from a numbered list. */
private val PICKED = Regex("""^\s*[❯›]\s*\d[.)]\s+\S""")
private val YES_NO = Regex("""\((?:y/n|Y/n|y/N)\)|\[(?:y/n|Y/n|y/N)\]""")

/**
 * The bottom of the agent's screen asks the person something: a menu of two or more choices with
 * one highlighted, or a y/n question. Only the last lines count, where a prompt is drawn.
 */
internal fun screenAsks(screen: String): Boolean {
    val tail = screen.lines().map { FRAME.replace(it, " ").trimEnd() }.filter { it.isNotBlank() }.takeLast(ASK_TAIL)
    val menu = tail.any { PICKED.containsMatchIn(it) } && tail.mapNotNull { OPTION.matchEntire(it)?.groupValues?.get(1) }.distinct().size >= 2
    return menu || tail.takeLast(4).any { YES_NO.containsMatchIn(it) }
}

private const val ASK_TAIL = 24

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
 * The agent is waiting on the person: the question as its screen shows it, and a key per
 * answer. A number picks the answer as it would at the PC; the arrows and Enter work a menu the
 * card could not read; Esc declines. The screen is the terminal's own, followed by
 * [WatchScreen]: a TUI draws its prompt with cursor moves, so only an emulator knows what it
 * looks like.
 */
@Composable
private fun AskCard(chat: SessionChat, screen: State<String?>, online: Boolean, modifier: Modifier) {
    val c = Relay.colors
    val text = screen.value
    val (question, options) = remember(text) { askOf(text.orEmpty()) }
    Column(
        modifier.clip(Radii.card).background(c.slab).border(1.dp, c.held.copy(alpha = .45f), Radii.card).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dot(c.held, 6.dp)
            T("Waiting for you", style = Relay.type.ui, color = c.heldText, weight = androidx.compose.ui.text.font.FontWeight.Medium)
        }
        if (text == null) T("Reading the agent's screen…", style = Relay.type.caption, color = c.ink3)
        else if (question.isNotEmpty()) T(question.joinToString("\n"), style = Relay.type.mono.copy(fontSize = 11.5.sp, lineHeight = 16.sp), color = c.ink2)
        if (options.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((n, label) in options) {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = TOUCH).alpha(if (online) 1f else .5f).clip(Radii.keycap).background(c.wash)
                            .clickable(enabled = online, role = Role.Button, onClickLabel = label) { chat.key(n) }.padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        T(n, style = Relay.type.mono, color = c.ink3)
                        T(label, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 3)
                    }
                }
            }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (options.isEmpty()) {
                Key("Yes", { chat.key("y") }, enabled = online, compact = true)
                Key("No", { chat.key("n") }, enabled = online, compact = true)
            } else {
                Key("↑", { chat.key(ARROW_UP) }, enabled = online, compact = true)
                Key("↓", { chat.key(ARROW_DOWN) }, enabled = online, compact = true)
            }
            Key("Enter", { chat.key(Keys.ENTER) }, enabled = online, compact = true)
            Key("Esc", { chat.key(Keys.ESC) }, kind = KeyKind.Quiet, enabled = online, compact = true)
        }
    }
}

/**
 * Follows the agent's terminal while this is on screen and writes the text of its rows into
 * [into]: at most [SCREEN_READS_PER_S] reads a second however fast the PC redraws, read off the
 * main thread. The feed is the one the terminal screen shares, so opening that keeps the stream.
 */
@Composable
private fun WatchScreen(nav: Nav, name: String, into: MutableState<String?>) {
    val feed = rememberFeed(nav.relay, name)
    LaunchedEffect(feed) {
        try {
            combine(feed.version, feed.status) { _, status -> status }.conflate().collect { status ->
                val text = if (status == TerminalFeed.Status.Connecting) null
                else withContext(Dispatchers.Default) { synchronized(feed.lock) { feed.terminal.plainText(feed.terminal.rows) } }
                if (text != into.value) into.value = text
                delay(1_000L / SCREEN_READS_PER_S)
            }
        } finally {
            into.value = null
        }
    }
}

private const val SCREEN_READS_PER_S = 8

/**
 * The agent's screen under the conversation, drawn as its terminal is: the PC's colours and
 * columns, at a size that reads (the PC's width fitted when it can be, panned sideways when not,
 * a pinch to change it), the newest line in view. A drag scrolls back, a long press copies, a tap
 * on an address opens it. A fresh TUI leaves its bottom rows blank; they are drawn under the edge.
 */
@Composable
internal fun ScreenPanel(nav: Nav, name: String, screen: State<String?>, onClose: () -> Unit) {
    val c = Relay.colors
    val feed = rememberFeed(nav.relay, name)
    val reading = screen.value == null
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboard.current
    val uris = LocalUriHandler.current
    val density = LocalDensity.current
    val ratio = rememberMonoRatio()
    val shape by produceState(0 to 0, feed) {
        feed.version.collect {
            value = synchronized(feed.lock) { (feed.pcSize?.first ?: feed.terminal.cols) to blankBelow(feed.terminal) }
            delay(1_000L / SCREEN_READS_PER_S)
        }
    }
    var pinched by remember { mutableStateOf<Float?>(null) }
    Column(Modifier.fillMaxWidth().background(c.screen)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
        Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dot(if (reading) c.waiting else c.live, 5.dp)
            T(if (reading) "Reading the agent's screen…" else "The agent's screen", Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
            IconKey("close", onClose, size = 14.dp, tint = c.ink3)
        }
        BoxWithConstraints(Modifier.fillMaxWidth().height(SCREEN_PANEL_H).clipToBounds()) {
            val widthPx = constraints.maxWidth.toFloat()
            val (cols, blank) = shape
            val sp = pinched ?: fitSp(widthPx, cols, ratio, density).takeIf { it > 0f }?.coerceIn(SCREEN_MIN_SP, SCREEN_SP) ?: SCREEN_SP
            val needPx = if (cols <= 0) 0f else cols * ratio * with(density) { sp.sp.toPx() } + with(density) { 12.dp.toPx() } + 2f
            val extra = with(density) { (blank * cellHeightPx(sp, density)).toDp() }
            Box(Modifier.fillMaxSize().horizontalScroll(rememberScrollState())) {
                TerminalView(
                    feed,
                    sp,
                    onFontSp = { pinched = it },
                    onFit = { _, _ -> },
                    modifier = Modifier.width(with(density) { maxOf(widthPx, needPx).toDp() }).wrapContentHeight(Alignment.Top, unbounded = true).height(SCREEN_PANEL_H + extra),
                    onCopy = { picked ->
                        scope.launch {
                            clipboard.copy(picked)
                            nav.shell.toast("Copied")
                        }
                    },
                    onLink = { url -> runCatching { uris.openUri(url) }.onFailure { nav.shell.toast("No app here opens $url") } },
                )
            }
        }
    }
}

private val SCREEN_PANEL_H = 240.dp
private const val SCREEN_SP = 10.5f
private const val SCREEN_MIN_SP = 8f

// ---- Keys and composer ----

private class ChatKey(val label: String, val spoken: String, val data: String)

/** Arrows as a normal-mode cursor sends them: with no screen to ask, both CLIs read these. */
private const val ARROW_UP = "\u001b[A"
private const val ARROW_DOWN = "\u001b[B"
private const val ARROW_RIGHT = "\u001b[C"
private const val ARROW_LEFT = "\u001b[D"

/** The keys an agent CLI still asks for in a conversation besides Esc and the mode switch, which the message box holds: a menu's arrows and ticks. */
private val CHAT_KEYS = listOf(
    ChatKey("↑", "Up", ARROW_UP),
    ChatKey("↓", "Down", ARROW_DOWN),
    ChatKey("←", "Left: the question before", ARROW_LEFT),
    ChatKey("→", "Right: the next question", ARROW_RIGHT),
    ChatKey("⏎", "Enter", Keys.ENTER),
    ChatKey("Tab", "Tab", Keys.TAB),
    ChatKey("Space", "Space: tick a choice", " "),
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

/**
 * The message box, as a thread's (threads.css `.threads-composer`): raised, an edge that lights
 * when focused, up to six lines, and under them the agent's own keys as chips and the round key,
 * which sends, or with nothing typed stops a working agent or gives a waiting one its Enter.
 * The terminal's message box is this one too, with fewer lines and no autocorrect.
 */
@Composable
internal fun ChatComposer(
    field: TextFieldValue,
    onText: (TextFieldValue) -> Unit,
    enabled: Boolean,
    working: Boolean,
    placeholder: String,
    focus: FocusRequester,
    onSend: () -> Unit,
    pictures: Boolean = false,
    lines: Int = 6,
    keyboard: KeyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
    above: @Composable () -> Unit = {},
    chips: @Composable RowScope.() -> Unit,
) {
    val c = Relay.colors
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val haptic = LocalHapticFeedback.current
    val text = field.text
    Column(Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding().padding(start = 10.dp, end = 10.dp, top = 4.dp, bottom = 8.dp)) {
        Column(
            Modifier.fillMaxWidth().clip(Radii.card).background(c.raised)
                .border(1.dp, if (focused) c.lineFocus else c.strong, Radii.card)
                .padding(start = 14.dp, end = 6.dp, top = 12.dp, bottom = 4.dp),
        ) {
            if (pictures) above()
            BasicTextField(
                value = field,
                onValueChange = onText,
                modifier = Modifier.fillMaxWidth().padding(end = 8.dp).heightIn(min = 24.dp).alpha(if (enabled) 1f else .6f)
                    .focusRequester(focus).focusProperties { canFocus = enabled },
                textStyle = Relay.type.body.copy(color = c.ink),
                cursorBrush = SolidColor(c.ink),
                minLines = 1,
                maxLines = lines,
                interactionSource = source,
                keyboardOptions = keyboard,
                decorationBox = { inner ->
                    Box {
                        if (text.isEmpty()) T(placeholder, style = Relay.type.body, color = c.ink3, maxLines = 1)
                        inner()
                    }
                },
            )
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp), content = chips)
                val empty = text.isBlank() && !pictures
                val stop = working && empty
                Box(
                    Modifier.size(TOUCH).clip(Radii.key)
                        .clickable(enabled = enabled, onClickLabel = if (stop) "Stop" else if (empty) "Enter" else "Send", role = Role.Button) {
                            if (!empty) haptic.performHapticFeedback(HapticFeedbackType.Confirm)
                            onSend()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(Modifier.size(34.dp).clip(Radii.key).background(if (enabled) c.ink else c.track), contentAlignment = Alignment.Center) {
                        val tint = if (enabled) c.wall else c.ink3
                        when {
                            stop -> Glyph("stop", 15.dp, tint)
                            empty -> ReturnGlyph(17.dp, tint)
                            else -> Glyph("arrow-up", 16.dp, tint)
                        }
                    }
                }
            }
        }
    }
}

/** A key of the agent's under the message box, drawn as a thread's chips are: a pill in a full-size target. */
@Composable
internal fun Chip(label: String, spoken: String, enabled: Boolean, glyph: String? = null, trailing: String? = null, icon: (@Composable () -> Unit)? = null, onClick: () -> Unit) {
    val c = Relay.colors
    val haptic = LocalHapticFeedback.current
    Box(
        Modifier.heightIn(min = TOUCH).clickable(enabled = enabled, onClickLabel = spoken, role = Role.Button) {
            haptic.performHapticFeedback(HapticFeedbackType.KeyboardTap)
            onClick()
        },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier.alpha(if (enabled) 1f else .45f).heightIn(min = 30.dp).clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill)
                .padding(start = if (glyph != null || icon != null) 8.dp else 10.dp, end = if (trailing != null || label.isEmpty()) 8.dp else 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(5.dp),
        ) {
            glyph?.let { Glyph(it, if (label.isEmpty()) 15.dp else 13.dp, if (label.isEmpty()) c.ink2 else c.ink3) }
            icon?.invoke()
            if (label.isNotEmpty()) T(label, style = Relay.type.caption, color = c.ink2, maxLines = 1, weight = FontWeight.Medium)
            trailing?.let { Glyph(it, 11.dp, c.ink3) }
        }
    }
}

/**
 * The phone's speech recognizer as a key's action, what was said handed to [onSaid]; null on a
 * phone without one, so the key is not shown.
 */
@Composable
internal fun rememberDictation(onSaid: (String) -> Unit): (() -> Unit)? {
    val context = LocalContext.current
    val can = remember { Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).resolveActivity(context.packageManager) != null }
    val said by rememberUpdatedState(onSaid)
    val speech = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        r.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.trim()?.takeIf { it.isNotEmpty() }?.let(said)
    }
    if (!can) return null
    return { speech.launch(Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)) }
}

/** What was typed, then what was said after a space. */
internal fun joinSaid(typed: String, said: String): String = typed.trimEnd().let { if (it.isEmpty()) said else "$it $said" }

/** A microphone on the glyphs' 16-unit grid: its capsule, the cradle under it, the stand. */
@Composable
internal fun MicGlyph(size: Dp, tint: Color) {
    Box(
        Modifier.size(size).drawWithCache {
            val u = this.size.minDimension / 16f
            val stroke = Stroke(width = 1.3f * u, cap = StrokeCap.Round, join = StrokeJoin.Round)
            val capsule = Path().apply { addRoundRect(RoundRect(5.75f * u, 1.75f * u, 10.25f * u, 9.75f * u, 2.25f * u, 2.25f * u)) }
            val cradle = Path().apply {
                moveTo(3.5f * u, 7.5f * u)
                cubicTo(3.5f * u, 13f * u, 12.5f * u, 13f * u, 12.5f * u, 7.5f * u)
                moveTo(8f * u, 11.6f * u)
                lineTo(8f * u, 14.25f * u)
            }
            onDrawBehind {
                drawPath(capsule, tint, style = stroke)
                drawPath(cradle, tint, style = stroke)
            }
        },
    )
}

/**
 * Claude Code's permission mode, as its footer says it ("⏵⏵ accept edits on (shift+tab to
 * cycle)"); null when the footer names none, which for Claude Code is its default.
 */
internal fun modeOf(screen: String): String? {
    val foot = screen.lines().filter { it.isNotBlank() }.takeLast(MODE_TAIL).joinToString("\n").lowercase()
    return when {
        "plan mode on" in foot -> "Plan"
        "bypass permissions on" in foot -> "Bypass"
        "auto mode on" in foot -> "Auto"
        "accept edits on" in foot -> "Accept edits"
        "shift+tab to cycle" in foot || "? for shortcuts" in foot -> "Default"
        else -> null
    }
}

private const val MODE_TAIL = 4

/** Pictures one message may carry. */
private const val MAX_PICTURES = 4

// ---- The agent's commands ----

/** Claude Code's slash commands a phone is likely to want, in words. */
private val CLAUDE_COMMANDS = listOf(
    "compact" to "Summarise the conversation to free its context",
    "clear" to "Start over with an empty conversation",
    "context" to "What fills the context window",
    "model" to "Choose the model",
    "review" to "Review a pull request",
    "init" to "Write a CLAUDE.md for this project",
    "memory" to "Edit what it remembers",
    "rewind" to "Go back to an earlier point",
    "resume" to "Pick up an earlier conversation",
    "usage" to "Plan limits and what is left",
    "cost" to "What this conversation has cost",
    "status" to "Version, model and account",
    "permissions" to "What it may do without asking",
    "agents" to "Its subagents",
    "mcp" to "MCP servers",
    "todos" to "Its to-do list",
    "export" to "Save the conversation to a file",
    "help" to "Every command",
)

/** Codex's. */
private val CODEX_COMMANDS = listOf(
    "model" to "Choose the model and its effort",
    "approvals" to "What it may do without asking",
    "review" to "Review the changes",
    "diff" to "Show what changed",
    "compact" to "Summarise the conversation to free its context",
    "new" to "Start a new conversation",
    "init" to "Write an AGENTS.md for this project",
    "mention" to "Mention a file",
    "status" to "Model, approvals and token use",
    "mcp" to "MCP tools",
)

/** What the person is typing after a leading slash, while it is still one word; null otherwise. */
internal fun commandAsked(text: String): String? =
    text.takeIf { it.startsWith("/") }?.drop(1)?.takeIf { rest -> rest.none { it.isWhitespace() } }

/** [all] matching [asked]: names that start with it first, then names and words that hold it. */
internal fun matchCommands(all: List<Pair<String, String>>, asked: String): List<Pair<String, String>> {
    val q = asked.lowercase()
    if (q.isEmpty()) return all
    val starts = all.filter { it.first.lowercase().startsWith(q) }
    val holds = all.filter { it !in starts && (q in it.first.lowercase() || q in it.second.lowercase()) }
    return starts + holds
}

/**
 * The agent's commands over the message box while the person types `/` and a word: its
 * provider's own and, for Claude Code, the project's skills, narrowed as they type. A tap puts
 * the command in the box to send, with room for what follows it.
 */
@Composable
private fun Commands(nav: Nav, provider: String?, projectId: Long?, asked: String, onPick: (String) -> Unit) {
    val c = Relay.colors
    val claude = provider != "codex"
    val payload = remember(projectId) { buildJsonObject { projectId?.let { put("project_id", it) }; put("enabled", true); put("summary", true) } }
    val live by remember(payload, claude) { if (claude) nav.relay.live("skill.list", payload) else flowOf(RelayData.Live()) }.collectAsStateWithLifecycle(RelayData.Live())
    val skills = remember(live.result) {
        (live.result as? JsonObject)?.get("skills").let { it as? JsonArray }.orEmpty()
            .mapNotNull { (it as? JsonObject)?.let { o -> o.str("name").takeIf(String::isNotBlank)?.let { n -> n to o.str("description").lineSequence().first() } } }
    }
    val own = remember(asked, claude) { matchCommands(if (claude) CLAUDE_COMMANDS else CODEX_COMMANDS, asked) }
    val theirs = remember(asked, skills) { matchCommands(skills, asked) }
    if (own.isEmpty() && theirs.isEmpty()) return
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 10.dp).padding(top = 6.dp).heightIn(max = 280.dp)
            .clip(Radii.card).background(c.raised).border(1.dp, c.strong, Radii.card).verticalScroll(rememberScrollState()).padding(vertical = 6.dp),
    ) {
        for ((cmd, about) in own) CommandRow(cmd, about) { onPick(cmd) }
        if (theirs.isNotEmpty()) {
            T("Skills", Modifier.padding(start = 14.dp, top = 10.dp, bottom = 2.dp), Relay.type.caption, c.ink3)
            for ((cmd, about) in theirs) CommandRow(cmd, about) { onPick(cmd) }
        }
    }
}

@Composable
private fun CommandRow(cmd: String, about: String, onClick: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = TOUCH).clickable(onClickLabel = "Use /$cmd", role = Role.Button, onClick = onClick).padding(horizontal = 14.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        T("/$cmd", style = Relay.type.mono, color = c.ink, maxLines = 1)
        if (about.isNotBlank()) T(about, Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
    }
}
