package com.quietsoftware.relay.ui.dev

import android.content.Intent
import android.os.SystemClock
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
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
import androidx.compose.ui.hapticfeedback.HapticFeedback
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
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.code.ADD_TEXT
import com.quietsoftware.relay.ui.code.ADD_WASH
import com.quietsoftware.relay.ui.code.DEL_TEXT
import com.quietsoftware.relay.ui.code.DEL_WASH
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
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
import com.quietsoftware.relay.ui.threads.dayOf
import com.quietsoftware.relay.ui.threads.humanDay
import com.quietsoftware.relay.ui.threads.said
import com.quietsoftware.relay.ui.threads.str
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
import androidx.compose.foundation.layout.statusBarsPadding
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.threads.BUBBLE
import com.quietsoftware.relay.ui.threads.BarButton
import com.quietsoftware.relay.ui.threads.BarKey
import com.quietsoftware.relay.ui.threads.BarLabel
import com.quietsoftware.relay.ui.threads.CLAY
import com.quietsoftware.relay.ui.threads.ReplyKeys
import com.quietsoftware.relay.ui.threads.TalkBar
import com.quietsoftware.relay.ui.threads.Working
import com.quietsoftware.relay.ui.threads.bubbleFill
import com.quietsoftware.relay.ui.threads.bubbleStyle
import com.quietsoftware.relay.ui.threads.rememberToday
import com.quietsoftware.relay.ui.threads.replyStyle

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
    // Kept a while after this screen closes: back here, the conversation shows as it was left.
    val chat = rememberChat(nav.relay, name)
    val messages by chat.messages.collectAsStateWithLifecycle()
    val sending by chat.sending.collectAsStateWithLifecycle()
    val status by chat.status.collectAsStateWithLifecycle()
    val found by chat.found.collectAsStateWithLifecycle()
    val error by chat.error.collectAsStateWithLifecycle()
    val life = rememberLifecycle(nav) { nav.leaveAgent(it) }
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboard.current
    val focus = remember { FocusRequester() }
    var menu by remember { mutableStateOf(false) }
    var mailing by remember { mutableStateOf(false) }
    var controls by remember { mutableStateOf(false) }
    // What is being written to this agent outlives the screen: away and back, it is still here.
    val draft = remember(name) { ChatDrafts.of(name) }
    // A value, not a string: a picked command puts the cursor after itself.
    var field by rememberSaveable(name, stateSaver = TextFieldValue.Saver) { mutableStateOf(draft.text) }
    SideEffect { draft.text = field }
    val text = field.text
    fun type(next: String) {
        field = TextFieldValue(next, TextRange(next.length))
    }
    // Pictures for the next message: each sent to the agent's checkout as soon as it is picked,
    // and arriving all the same when the person opens the terminal meanwhile.
    val attached = draft.attached
    // Dictation through the phone's own recognizer: what was said joins what is typed.
    val dictate = rememberDictation { said -> type(joinSaid(field.text, said)) }
    val pickPictures = rememberPicturePicker(attached) { picture -> chat.upload(picture) }

    val ready = session?.attachable == true && s.online
    val working = session?.working == true
    val held = session?.blocked == true
    // Held at a prompt, the turn is still open: its last call waits on the answer, it was not cut off.
    val today = rememberToday()
    val pieces = remember(messages, sending, working, held, today) { chatPieces(messages, sending, working || held, today) }
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
    val since = remember(messages, sending) { epoch((sending.lastOrNull() ?: messages.lastOrNull { it.isPersons })?.createdAt).takeIf { it > 0 } }

    KeepScreenOn(s.prefs.keepScreenOn)

    // When the last message went: the key turns to Stop under the finger that sent it.
    var sentAt by remember { mutableLongStateOf(0L) }
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
        val paths = pictures.mapNotNull { it.sent }
        if (typed.isEmpty() && paths.isEmpty()) {
            // Empty: Esc stops an agent mid-turn; otherwise (a menu up, a prompt) it is the Enter it waits for.
            val stop = working && !asking
            // A second tap on Send just after a message is that tap again, not a stop.
            if (stop && SystemClock.uptimeMillis() - sentAt < STOP_AFTER_SEND_MS) return
            chat.key(if (stop) Keys.ESC else Keys.ENTER)
            if (stop) nav.shell.toast("Asked ${session?.provider?.let(::providerName) ?: "the agent"} to stop")
            return
        }
        sentAt = SystemClock.uptimeMillis()
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
            val taskId = session?.taskId
            val task by remember(taskId) { if (taskId != null) nav.relay.task(taskId.toString()) else flowOf(null) }.collectAsStateWithLifecycle(null)
            val project = s.projects.firstOrNull { it.num == session?.projectId }?.takeIf { s.projects.size > 1 }
            AgentBar(session, name, task, project, onBack = nav::back, onDetails = { nav.session(name) }, onMenu = { menu = true }) {
                session?.let { ss -> IconKey("branch", { nav.git(ss.projectId, ss.worktree.ifBlank { null }, name) }, size = 18.dp, tint = c.ink2, label = "Review changes") }
            }
            ChatBanner(status, session, s.online, life)
            // The agent's plan, pinned: what it is on, and how far along, while there is any left.
            val plan = remember(messages) { latestPlan(messages) }
            if (plan != null && (working || plan.any { it.state != 'x' })) PlanStrip(name, plan)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                // Nothing to read yet: the agent's details, never its terminal.
                val openTerminal = @Composable { Key("Agent details", { nav.session(name) }, glyph = "sliders", compact = true) }
                when {
                    pieces.isNotEmpty() || asking -> Conversation(
                        pieces, doing, since, asking, screen, chat, s.online, session?.provider == "codex",
                        onEdit = { words ->
                            type(joinDraft(field.text, words))
                            focus.requestFocus()
                        },
                        onResend = if (ready) ::resend else null,
                    )
                    error != null && status != TerminalFeed.Status.Saved -> Empty(
                        "terminal", "Can't read the conversation",
                        said(error!!.error, "The PC's Relay is older than this app: update it to see agents as a conversation."),
                        action = openTerminal,
                    )
                    found == false -> Empty(
                        "terminal", "Nothing said yet",
                        "The conversation shows here once ${session?.provider?.let(::providerName) ?: "the agent"} starts.",
                        action = openTerminal,
                    )
                    status == TerminalFeed.Status.Connecting && s.online -> Empty("terminal", "Reading the conversation", action = { Spinner(16.dp) })
                    else -> Empty("terminal", "Nothing said yet", if (s.online) null else "The PC is away, and this phone has not read this agent before.", action = openTerminal)
                }
            }
            com.quietsoftware.relay.ui.shell.LiftToasts()
            if (ready) ChatActions(
                working = working && !asking,
                onStop = {
                    chat.key(Keys.ESC)
                    nav.shell.toast("Asked ${session?.provider?.let(::providerName) ?: "the agent"} to stop")
                },
                onChanges = { session?.let { ss -> nav.git(ss.projectId, ss.worktree.ifBlank { null }, name) } },
                onCommands = { controls = true },
            )
            val asked = commandAsked(text)
            if (asked != null && ready) Commands(nav, session?.provider, session?.projectId, asked) { type("/$it ") }
            // Claude's bar: the words, the keys for pictures and for the agent's own keys, its mode, and the round key.
            val empty = text.isBlank() && attached.isEmpty()
            val barKey = when {
                !empty -> BarKey.Send
                working && !asking -> BarKey.Stop
                asking || dictate == null -> BarKey.Enter
                else -> BarKey.Speak
            }
            Box(Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding().padding(start = 10.dp, end = 10.dp, top = 4.dp, bottom = 8.dp)) {
                TalkBar(
                    field = field,
                    onField = { field = it },
                    placeholder = when {
                        session == null -> "This agent is closed"
                        !s.online -> "Needs the PC to talk to the agent"
                        !ready -> "The agent is not running"
                        else -> "Reply to ${session?.name ?: "the agent"}…"
                    },
                    key = barKey,
                    keyEnabled = ready,
                    onKey = { if (barKey == BarKey.Speak) dictate?.invoke() else send() },
                    enabled = ready,
                    focus = focus,
                    lines = 6,
                    above = { if (attached.isNotEmpty()) AttachmentStrip(attached) { id -> attached.removeAll { it.id == id } } },
                    trailing = {
                        // The agent's mode where Claude's app names its model: a tap moves to the next, as Shift Tab does.
                        mode?.takeIf { ready }?.let { m -> BarLabel(m, "Mode: $m. Tap for the next mode", chevron = false, onClick = { chat.key(Keys.SHIFT_TAB) }) }
                    },
                ) {
                    BarButton("plus", "Attach a photo or a screenshot", ready && attached.size < MAX_PICTURES, onClick = pickPictures)
                    BarButton("sliders", "The agent's keys and commands", onClick = { controls = true })
                }
            }
        }
    }

    // The agent's commands sheet (design 14): stop it, free its memory, start it fresh, how much it
    // does alone, and its slash commands and skills in words. Every one is a button.
    if (controls) DevSheet({ controls = false }, session?.intent?.ifBlank { null } ?: name, session?.let { agentLine(it) }) { close ->
        val codex = session?.provider == "codex"
        Row(Modifier.fillMaxWidth().padding(horizontal = 6.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ActionTile("pause", "Stop", ready, Modifier.weight(1f)) { close { chat.key(Keys.ESC) } }
            ActionTile("save", "Free memory", ready && !working, Modifier.weight(1f)) { close { resend("/compact") } }
            ActionTile("refresh", "Start fresh", ready && !working, Modifier.weight(1f)) { close { resend(if (codex) "/new" else "/clear") } }
        }
        Eyebrow("How much it does alone", Modifier.padding(start = 6.dp, top = 14.dp, bottom = 6.dp))
        SheetRow(modeWords(mode), "shield", { close { chat.key(Keys.SHIFT_TAB) } }, detail = "Change", enabled = ready)
        Eyebrow("Ask it to…", Modifier.padding(start = 6.dp, top = 14.dp, bottom = 6.dp))
        SheetRow("Review its own changes", "focus", { close { resend("Review the changes you made so far and list anything that could break.") } }, enabled = ready)
        SheetRow("Run a command or skill", "code", {
            close {
                if (text.isBlank()) type("/")
                focus.requestFocus()
            }
        }, detail = "Pick from the list", enabled = ready)
        SheetRow("Send it a message", "mail", { close { mailing = true } }, enabled = session != null && session?.state != "closed")
    }

    LifecycleSheets(life)
    if (menu) {
        DevSheet({ menu = false }, name, session?.let(::kindWords)) { close ->
            session?.let { ss ->
                SheetRow("Changes", "commit", { close { nav.git(ss.projectId, ss.worktree.ifBlank { null }, name) } })
                if (ss.state != "closed" && !ss.pending) SheetRow("Mail the agent", "mail", { close { mailing = true } })
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

/**
 * The conversation's top bar, drawn as a thread's: back, the agent's badge, what it is doing over
 * who it is in plain words (its name, its project, its provider, its state, its branch when that
 * is not its own), its keys and ⋯. A tap on the title opens its details.
 */
@Composable
private fun AgentBar(session: Session?, name: String, task: Task?, project: Project?, onBack: () -> Unit, onDetails: () -> Unit, onMenu: () -> Unit, extra: @Composable () -> Unit = {}) {
    val c = Relay.colors
    val held = session?.blocked == true
    Row(
        Modifier.fillMaxWidth().background(c.wall).statusBarsPadding().heightIn(min = 56.dp).padding(start = 4.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconKey("arrow-left", onBack, size = 18.dp, tint = c.ink2)
        Row(
            Modifier.weight(1f).clip(Radii.icon).clickable(onClickLabel = "Session details", onClick = onDetails).padding(horizontal = 2.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (session != null) AgentBadge(session, 32.dp, ring = c.wall)
            Column(Modifier.weight(1f)) {
                val title = session?.let { agentTitle(it, task) } ?: name
                T(title, style = Relay.type.title, color = c.ink, maxLines = 1)
                if (session != null) {
                    val line = listOfNotNull(name.takeIf { it != title }, project?.name, stateWord(session), providerName(session.provider)).joinToString(" · ")
                    T(line, style = Relay.type.caption, color = if (held) c.waiting else c.ink3, maxLines = 1)
                }
            }
        }
        extra()
        IconKey("more", onMenu, size = 18.dp, tint = c.ink2)
    }
}

/** "Claude Code · Working · relay/other-branch": an agent in words, under its name. */
internal fun agentLine(session: Session): String {
    val branch = session.branch.takeIf { it.isNotBlank() && it != "relay/${session.name}" }
    return listOfNotNull(providerName(session.provider), stateWord(session), branch).joinToString(" · ")
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
    /**
     * A line that is neither side's words, by [kind]: `compact` (the context was compacted, with
     * what it kept in [detail] once its summary follows), `relay` (what Relay typed in), `task`
     * (a background task finished), `interrupt`.
     */
    data class Note(override val key: String, val text: String, val kind: String = "", val detail: String? = null) : ChatPiece
    data class Error(override val key: String, val text: String) : ChatPiece
    /** The end of a finished turn: a quiet Copy key for the agent's words in it. */
    data class Copy(override val key: String, val text: String) : ChatPiece
}

/** [working]: the agent's turn is still open (running, or held at a prompt), so its last unanswered calls are waiting, not cut off. */
internal fun chatPieces(messages: List<ThreadMessage>, sending: List<ThreadMessage>, working: Boolean, today: LocalDate): List<ChatPiece> {
    val answers = HashMap<String, Pair<Boolean, String>>()
    for (m in messages) if (m.role == "tool") {
        val b = m.body as? JsonObject ?: continue
        answers[b.str("tool_use_id")] = ((b["is_error"] as? JsonPrimitive)?.booleanOrNull == true) to b.str("text")
    }
    val lastUser = messages.indexOfLast { it.isPersons }
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
        if (m.isPersons) {
            flush()
            endTurn()
        }
        day(m)
        when (if (m.role == "user" && !m.isPersons) "note" else m.role) {
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
                val b = m.body as? JsonObject
                // A summary logged as the person's turn, as a PC from before notes said otherwise sent it.
                val legacy = m.role == "user"
                val kind = if (legacy) "summary" else b?.str("kind").orEmpty()
                val detail = (if (legacy) m.text else b?.str("detail"))?.trim()?.ifEmpty { null }
                val before = out.lastOrNull() as? ChatPiece.Note
                // What a compaction kept opens under the line that says it happened.
                if (kind == "summary" && before?.kind == "compact" && before.detail == null) out[out.lastIndex] = before.copy(detail = detail)
                else if (kind == "summary") out += ChatPiece.Note("m:${m.id}", "Context compacted", "compact", detail)
                else out += ChatPiece.Note("m:${m.id}", m.text, kind, detail)
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

/**
 * Words the person wrote: not the summary Claude Code hands the model after a compaction, which
 * an older PC sent as a user turn.
 */
private val ThreadMessage.isPersons: Boolean
    get() = role == "user" && !text.trimStart().startsWith("This session is being continued from a previous conversation")

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
        "user" -> if (m.isPersons) "> ${m.text}" else "— Context compacted —"
        "assistant" -> m.text.takeIf { it.isNotBlank() }
        "error" -> "! ${m.text}"
        "note" -> m.text.takeIf { it.isNotBlank() && (m.body as? JsonObject)?.str("kind") != "summary" }?.let { "— $it —" }
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
    codex: Boolean,
    onEdit: (String) -> Unit,
    onResend: ((String) -> Unit)?,
) {
    val c = Relay.colors
    val state = rememberLazyListState()
    val scope = rememberCoroutineScope()
    // Newest at the bottom, and kept in view as it grows while the person reads the end.
    val shown = remember(pieces) { pieces.asReversed() }
    // What the person just sent always comes into view: it is the last piece until the log has it.
    LaunchedEffect(pieces.size, doing, asking) {
        if (state.firstVisibleItemIndex <= 1 || (pieces.lastOrNull() as? ChatPiece.User)?.sending == true) state.animateScrollToItem(0)
    }
    // Scrolled up to read: a key back to the newest, counting what arrived meanwhile.
    val away by remember { derivedStateOf { state.firstVisibleItemIndex > 1 } }
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
            if (doing != null) item(key = "doing") { WorkingRow(doing, since, codex, cell.padding(top = 14.dp)) }
            items(shown, key = { it.key }, contentType = { it::class }) { p ->
                if (p is ChatPiece.User) UserBubble(
                    p.text,
                    p.sending,
                    online,
                    cell.padding(top = 22.dp),
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
        is ChatPiece.User -> UserBubble(p.text, p.sending, online, cell.padding(top = 22.dp))
        is ChatPiece.Reply -> Markdown(p.text, cell.padding(top = 12.dp), replyStyle())
        is ChatPiece.Thought -> Box(cell.padding(top = 8.dp)) { ThoughtView(p) }
        is ChatPiece.Tool -> Box(cell.padding(top = 2.dp)) {
            when {
                isEdit(p.call.name) -> EditView(p.call)
                isAsk(p.call.name) -> ToolLine(p.call, startOpen = true)
                else -> ToolLine(p.call)
            }
        }
        is ChatPiece.Steps -> Box(cell.padding(top = 2.dp)) { StepsView(p) }
        is ChatPiece.Note -> NoteView(p, cell.padding(top = 14.dp))
        is ChatPiece.Error -> Row(cell.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph("close", 13.dp, c.heldText, Modifier.padding(top = 4.dp))
            SelectionContainer { T(p.text, style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp), color = c.heldText) }
        }
        is ChatPiece.Copy -> ReplyKeys(p.text, cell)
    }
}

/**
 * Where the conversation stands when it is not live, in the conversation's look: a quiet pill
 * under the header, with the key that brings a stopped agent back. The terminal keeps its band.
 */
@Composable
private fun ChatBanner(status: TerminalFeed.Status, session: Session?, online: Boolean, life: Lifecycle) {
    val c = Relay.colors
    val (line, dot) = bannerLine(status, session, online, ::stateWord) ?: return
    val revive = session?.takeIf { status == TerminalFeed.Status.Stopped && !it.pending }?.let { reviveAct(it.state) }
    Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 2.dp), horizontalArrangement = Arrangement.Center) {
        Row(
            Modifier.heightIn(min = 34.dp).clip(STRIP).background(c.wash).padding(start = 14.dp, end = if (revive != null) 4.dp else 14.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (dot != null) Dot(dot, 6.dp) else session?.let { LampDot(it.lamp, 6.dp) }
            T(line, Modifier.weight(1f, fill = false), Relay.type.caption, c.ink2, maxLines = 2)
            if (revive != null && session != null) {
                Key(
                    if (revive == Act.Start) "Start session" else revive.label,
                    { life.run(revive, session) },
                    Modifier.alpha(if (online) 1f else .5f),
                    kind = KeyKind.Primary,
                    glyph = revive.glyph,
                    enabled = life.busy == null,
                    compact = true,
                )
            }
        }
    }
}

/** The strips under the header: rounded, as the bar is. */
private val STRIP = RoundedCornerShape(18.dp)

/**
 * The agent's plan under the header, as a card in the conversation's look: a ring of how far
 * along it is, the step under way and the count, opening to every step, done ones ticked and quiet.
 */
@Composable
private fun PlanStrip(name: String, plan: List<PlanStep>) {
    val c = Relay.colors
    var open by rememberSaveable(name) { mutableStateOf(false) }
    val done = plan.count { it.state == 'x' }
    val now = plan.firstOrNull { it.state == '~' } ?: plan.firstOrNull { it.state != 'x' }
    Column(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 6.dp, bottom = 2.dp).clip(STRIP).background(c.wash)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = TOUCH).clickable(onClickLabel = if (open) "Fold the plan" else "Show the whole plan") { open = !open }
                .padding(start = 14.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            PlanRing(done, plan.size)
            T(if (done == plan.size) "Plan done" else now?.text.orEmpty(), Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
            T("$done of ${plan.size}", style = Relay.type.caption, color = c.ink3, maxLines = 1)
            Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.ink3)
        }
        AnimatedVisibility(open) {
            Column(
                Modifier.fillMaxWidth().heightIn(max = 260.dp).verticalScroll(rememberScrollState()).padding(start = 14.dp, end = 14.dp, bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                for (step in plan) {
                    Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Box(Modifier.padding(top = 2.dp).size(16.dp), contentAlignment = Alignment.Center) {
                            when (step.state) {
                                'x' -> Glyph("check", 13.dp, c.live)
                                '~' -> Spinner(11.dp, c.ink2)
                                else -> Box(Modifier.size(11.dp).border(1.dp, c.inkDim, CircleShape))
                            }
                        }
                        T(
                            step.text,
                            Modifier.weight(1f),
                            Relay.type.ui.copy(textDecoration = if (step.state == 'x') TextDecoration.LineThrough else null),
                            if (step.state == 'x') c.ink3 else c.ink2,
                        )
                    }
                }
            }
        }
    }
}

/** How far along a plan is, as a ring: green once every step is done. */
@Composable
private fun PlanRing(done: Int, total: Int) {
    val c = Relay.colors
    val track = c.track
    val fill = if (done == total) c.live else c.ink2
    val part = if (total == 0) 0f else done.toFloat() / total
    Box(
        Modifier.size(18.dp).drawBehind {
            val w = 2.dp.toPx()
            val box = Size(size.width - w, size.height - w)
            val at = androidx.compose.ui.geometry.Offset(w / 2, w / 2)
            drawArc(track, 0f, 360f, false, at, box, style = Stroke(w))
            if (part > 0f) drawArc(fill, -90f, 360f * part, false, at, box, style = Stroke(w, cap = StrokeCap.Round))
        },
    )
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
    val touch = Modifier.messageTouch(open, LocalHapticFeedback.current, onOpen)
    Column(modifier, horizontalAlignment = Alignment.End) {
        if (pictures.isNotEmpty()) Row(Modifier.padding(bottom = 6.dp).clip(Radii.pill).then(if (words.isEmpty()) touch else Modifier), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for (p in pictures) AttachedPill(p)
        }
        if (words.isNotEmpty()) Box(Modifier.fillMaxWidth(.86f), contentAlignment = Alignment.CenterEnd) {
            Box(
                Modifier.clip(BUBBLE).background(bubbleFill).then(if (open) Modifier.border(1.dp, c.lineFocus, BUBBLE) else Modifier).then(touch)
                    .padding(horizontal = 16.dp, vertical = 11.dp),
            ) {
                T(words, style = bubbleStyle(), color = c.ink)
            }
        }
        if (sending) Row(Modifier.padding(top = 5.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Dot(c.waiting, 5.dp)
            T(if (online) "Sent to the terminal" else "Waiting for the PC", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
        }
        MessageKeys(open, words, onClose = { onOpen?.invoke() }, onEdit = onEdit, onResend = onResend?.let { again -> { again(text) } })
    }
}

/** A tap or a long press on a message of the person's: [onOpen] shows its keys, or a tap hides them. */
@OptIn(ExperimentalFoundationApi::class)
internal fun Modifier.messageTouch(open: Boolean, haptic: HapticFeedback, onOpen: (() -> Unit)?): Modifier {
    if (onOpen == null) return this
    return combinedClickable(
        onClickLabel = if (open) "Hide this message's keys" else "Show this message's keys",
        onLongClick = {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            if (!open) onOpen()
        },
        onClick = onOpen,
    )
}

/** The keys under a message of the person's, while it is [open]: Copy and Edit for its [words], and Send again. */
@Composable
internal fun MessageKeys(open: Boolean, words: String, onClose: () -> Unit, onEdit: ((String) -> Unit)?, onResend: (() -> Unit)?) {
    val nav = LocalNav.current
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    AnimatedVisibility(open, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (words.isNotEmpty()) {
                Chip("Copy", "Copy this message", open, glyph = "copy") {
                    scope.launch {
                        clipboard.copy(words)
                        nav.shell.toast("Copied")
                    }
                    onClose()
                }
                onEdit?.let { edit -> Chip("Edit", "Edit it in the message box", open, glyph = "edit") { edit(words) } }
            }
            // Off while the keys fold away, so a quick second tap never sends it twice.
            onResend?.let { again -> Chip("Send again", "Send this message again", open, glyph = "refresh", onClick = again) }
        }
    }
}

/** The working row: Claude's spark (Codex's mark for Codex), the step running now, and how long the turn has run, as Claude Code counts it. */
@Composable
private fun WorkingRow(text: String, since: Long?, codex: Boolean, modifier: Modifier) {
    val c = Relay.colors
    val now by produceState(System.currentTimeMillis(), since) {
        while (since != null) {
            delay(1_000)
            value = System.currentTimeMillis()
        }
    }
    val took = since?.let { took(now - it) }
    Working(text, modifier, glyph = if (codex) "codex" else "claude", tint = if (codex) c.ink2 else CLAY) {
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

/** A note's glyph: what kind of line it is, at a glance. */
private fun noteGlyph(kind: String): String = when (kind) {
    "compact" -> "collapse"
    "relay" -> "board"
    "task" -> "terminal"
    "interrupt" -> "stop"
    else -> "minus"
}

/**
 * A line that is neither side's words, drawn as a divider across the conversation so it never
 * reads as either: its glyph and words between two rules. One with more to it (what a compaction
 * kept, the instructions Relay typed again) opens under the line.
 */
@Composable
private fun NoteView(p: ChatPiece.Note, modifier: Modifier) {
    val c = Relay.colors
    var open by rememberSaveable(p.key) { mutableStateOf(false) }
    val detail = p.detail
    val tint = if (p.kind == "compact") c.ink2 else c.ink3
    Column(modifier) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = if (detail != null) TOUCH else 24.dp).clip(Radii.icon)
                .clickable(enabled = detail != null, onClickLabel = if (open) "Fold it" else if (p.kind == "compact") "Show what it kept" else "Show more") { open = !open },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Box(Modifier.weight(1f).height(1.dp).background(c.edge))
            Glyph(noteGlyph(p.kind), 13.dp, tint)
            T(p.text, Modifier.widthIn(max = 280.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), tint, weight = if (p.kind == "compact") FontWeight.Medium else null)
            if (detail != null) Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
            Box(Modifier.weight(1f).height(1.dp).background(c.edge))
        }
        if (detail != null) AnimatedVisibility(open) {
            Column(Modifier.padding(top = 6.dp)) {
                if (p.kind == "compact") T("The agent carries on from this summary; what came before it is no longer in its context.", Modifier.padding(bottom = 6.dp), Relay.type.caption, c.ink3)
                Box(
                    Modifier.fillMaxWidth().heightIn(max = 360.dp).clip(Radii.keycap).border(1.dp, c.edge, Radii.keycap)
                        .verticalScroll(rememberScrollState()).padding(12.dp),
                ) { Markdown(detail, style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp), color = c.ink2) }
            }
        }
    }
}

/**
 * Reasoning, folded as Claude's app folds it: one quiet line, "Thought process" and how it began,
 * opening to all of it in italics beside a rule.
 */
@Composable
private fun ThoughtView(p: ChatPiece.Thought) {
    val c = Relay.colors
    var open by rememberSaveable(p.key) { mutableStateOf(false) }
    val gist = remember(p.text) { p.text.lineSequence().firstOrNull { it.isNotBlank() }.orEmpty().trim() }
    val style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp, fontStyle = FontStyle.Italic)
    val rule = c.edge
    Column {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 34.dp).clip(Radii.icon)
                .clickable(onClickLabel = if (open) "Fold the reasoning" else "Show the reasoning") { open = !open }.padding(end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            T("Thought process", style = Relay.type.ui, color = c.ink3, maxLines = 1)
            T(if (open) "" else gist, Modifier.weight(1f), Relay.type.caption, c.inkDim, maxLines = 1)
            Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            Box(
                Modifier.fillMaxWidth().padding(top = 2.dp, bottom = 6.dp)
                    .drawBehind { drawRect(rule, size = Size(2.dp.toPx(), size.height)) }
                    .padding(start = 12.dp, end = 6.dp),
            ) { Markdown(p.text, style = style, color = c.ink3) }
        }
    }
}

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
        modifier.clip(Radii.card).background(c.slab).border(1.dp, c.accent.copy(alpha = .55f), Radii.card).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph("help", 15.dp, c.accentText)
            Eyebrow("Waiting for your answer", color = c.accentText)
        }
        if (text == null) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Spinner(11.dp)
            T("Reading the question…", style = Relay.type.caption, color = c.ink3)
        } else if (question.isNotEmpty()) T(askWords(question), style = Relay.type.body, color = c.ink, weight = FontWeight.Medium)
        if (options.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                options.forEachIndexed { i, (n, label) ->
                    val first = i == 0
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = TOUCH).alpha(if (online) 1f else .5f).clip(Radii.key)
                            .background(if (first) c.accentSoft else c.wash)
                            .border(1.dp, if (first) c.accent else Color.Transparent, Radii.key)
                            .clickable(enabled = online, role = Role.Button, onClickLabel = label) { chat.key(n) }.padding(horizontal = 14.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        T(label.trimStart('❯', '›', '>', ' '), Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 3, weight = FontWeight.Medium)
                    }
                }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Key("Cancel", { chat.key(Keys.ESC) }, kind = KeyKind.Quiet, enabled = online, compact = true)
            }
        } else {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Yes", { chat.key("y") }, Modifier.weight(1f), kind = KeyKind.Primary, enabled = online)
                Key("No", { chat.key("n") }, Modifier.weight(1f), enabled = online)
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                Key("Continue", { chat.key(Keys.ENTER) }, kind = KeyKind.Quiet, enabled = online, compact = true)
                Key("Cancel", { chat.key(Keys.ESC) }, kind = KeyKind.Quiet, enabled = online, compact = true)
            }
        }
    }
}

/** The question's lines as one sentence: the screen wraps it, the card does not need to. */
internal fun askWords(lines: List<String>): String = lines.joinToString(" ") { it.trim() }.replace(Regex("\\s+"), " ")

/** The agent's mode in words: Claude Code's footer says it the way a terminal person reads it. */
internal fun modeWords(mode: String?): String {
    val m = mode?.lowercase().orEmpty()
    return when {
        m.isEmpty() -> "Asks before edits"
        "plan" in m -> "Plans first"
        "accept" in m || "auto" in m -> "Edits on its own"
        "bypass" in m || "yolo" in m -> "Does everything on its own"
        else -> mode!!
    }
}

/** The row over the message box (design 14): Stop while it works, its changes, its commands. */
@Composable
private fun ChatActions(working: Boolean, onStop: () -> Unit, onChanges: () -> Unit, onCommands: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 14.dp, end = 14.dp, top = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (working) ActionChip("pause", "Stop", onStop)
        ActionChip("branch", "Review changes", onChanges)
        ActionChip("zap", "Commands", onCommands)
    }
}

@Composable
private fun ActionChip(glyph: String, label: String, onClick: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.heightIn(min = 36.dp).clip(Radii.pill).background(c.pane).border(1.dp, c.strong, Radii.pill).clickable(role = Role.Button, onClick = onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Glyph(glyph, 14.dp, c.ink)
        T(label, style = Relay.type.caption, color = c.ink, weight = FontWeight.Medium)
    }
}

@Composable
private fun ActionTile(glyph: String, label: String, enabled: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = Relay.colors
    Column(
        modifier.alpha(if (enabled) 1f else .4f).clip(Radii.key).background(c.wash).clickable(enabled = enabled, role = Role.Button, onClick = onClick).padding(vertical = 14.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Glyph(glyph, 19.dp, c.ink)
        T(label, style = Relay.type.caption, color = c.ink, weight = FontWeight.Medium, maxLines = 1)
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
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ScreenPanel(nav: Nav, name: String, screen: State<String?>, onClose: () -> Unit) {
    val c = Relay.colors
    val feed = rememberFeed(nav.relay, name)
    val reading = screen.value == null
    // With the keyboard up the panel is shorter, so the conversation keeps some room over the message box.
    val tall = if (WindowInsets.isImeVisible) SCREEN_PANEL_IME_H else SCREEN_PANEL_H
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
            Box(Modifier.size(10.dp), contentAlignment = Alignment.Center) { if (reading) Spinner(10.dp) else Dot(c.live, 5.dp) }
            T(if (reading) "Reading the agent's screen…" else "The agent's screen", Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
            IconKey("close", onClose, size = 14.dp, tint = c.ink3, label = "Hide the agent's screen")
        }
        BoxWithConstraints(Modifier.fillMaxWidth().height(tall).clipToBounds()) {
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
                    modifier = Modifier.width(with(density) { maxOf(widthPx, needPx).toDp() }).wrapContentHeight(Alignment.Top, unbounded = true).height(tall + extra),
                    onCopy = { picked ->
                        scope.launch {
                            clipboard.copy(picked)
                            nav.shell.toast("Copied")
                        }
                    },
                    onLink = { url -> runCatching { uris.openUri(url) }.onFailure { nav.shell.toast("No app here opens $url") } },
                    hiddenBelow = extra,
                )
            }
        }
    }
}

private val SCREEN_PANEL_H = 240.dp
private val SCREEN_PANEL_IME_H = 120.dp
private const val SCREEN_SP = 10.5f
private const val SCREEN_MIN_SP = 8f

// ---- Keys and composer ----

private class ChatKey(val label: String, val spoken: String, val data: String)

/** Arrows as a normal-mode cursor sends them: with no screen to ask, both CLIs read these. */
private const val ARROW_UP = "\u001b[A"
private const val ARROW_DOWN = "\u001b[B"
private const val ARROW_RIGHT = "\u001b[C"
private const val ARROW_LEFT = "\u001b[D"

/** The keys an agent CLI still asks for in a conversation: Escape, a menu's arrows and ticks. The mode switch is the bar's. */
private val CHAT_KEYS = listOf(
    ChatKey("Esc", "Escape", Keys.ESC),
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
                Modifier.height(40.dp).widthIn(min = 44.dp).alpha(if (enabled) 1f else .4f).clip(Radii.keycap).background(c.slab).border(1.dp, c.edge, Radii.keycap)
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
    // A terminal sends spaces as typed: only no text at all is its Enter.
    blankIsEmpty: Boolean = true,
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
                val empty = (if (blankIsEmpty) text.isBlank() else text.isEmpty()) && !pictures
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

/**
 * A key of the agent's under the message box, drawn as a thread's chips are: a pill in a
 * full-size target. Off, a tap does nothing, or says why when it has [whyOff].
 */
@Composable
internal fun Chip(label: String, spoken: String, enabled: Boolean, glyph: String? = null, trailing: String? = null, icon: (@Composable () -> Unit)? = null, whyOff: (() -> Unit)? = null, onClick: () -> Unit) {
    val c = Relay.colors
    val haptic = LocalHapticFeedback.current
    Box(
        Modifier.heightIn(min = TOUCH).clickable(enabled = enabled || whyOff != null, onClickLabel = spoken, role = Role.Button) {
            if (enabled) {
                haptic.performHapticFeedback(HapticFeedbackType.KeyboardTap)
                onClick()
            } else {
                whyOff?.invoke()
            }
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

/** A message brought back to edit: in place of an empty box, or after what is being written, never over it. */
internal fun joinDraft(draft: String, words: String): String = draft.trimEnd().let { if (it.isEmpty()) words else "$it\n\n$words" }

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

/** How long after a message the empty box's key will not stop the agent. */
private const val STOP_AFTER_SEND_MS = 1_000L

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
