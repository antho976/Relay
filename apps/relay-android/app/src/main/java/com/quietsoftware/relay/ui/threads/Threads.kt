package com.quietsoftware.relay.ui.threads

import android.content.ClipData
import android.os.SystemClock
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Thread
import com.quietsoftware.relay.core.model.ThreadMessage
import com.quietsoftware.relay.core.sync.Fetch
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.dev.Attachment
import com.quietsoftware.relay.ui.dev.MessageKeys
import com.quietsoftware.relay.ui.dev.joinDraft
import com.quietsoftware.relay.ui.dev.messageTouch
import com.quietsoftware.relay.ui.dev.rememberPicturePicker
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LocalFence
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.LocalDate
import java.time.LocalTime

/** Questions a new thread offers when the ledger suggests fewer of its own (threads_view.rs `SUGGESTIONS`). */
private val SUGGESTIONS = listOf(
    "Where did my money go this week?" to "Spending by category",
    "Am I on pace this month?" to "Each budget against the day of the month",
    "What bills are coming up?" to "Due dates and whether you can cover them",
    "Any duplicates or odd entries this month?" to "Entries that look doubled or out of place",
)

/** How often a streaming reply redraws: about 30 a second. */
private const val FRAME = 33L

/** A stop this soon after a message is the same tap again, landing on the key it turned into (the agent chat's). */
private const val STOP_AFTER_SEND_MS = 1_000L

/** The key [Drafts] keeps a new thread's words under. */
private const val NEW = "new"

private fun gutter(width: Dp) = if (width < 600.dp) 16.dp else 24.dp

/**
 * Each thread's unsent words and pictures while the app runs, by thread id, a new thread's under
 * [NEW] (threads_view.rs `drafts`): Back, a new thread or another thread keep them for when that
 * one shows again. Sending empties them; deleting the thread drops them.
 *
 * Used from the main thread only.
 */
private object Drafts {
    private val held = HashMap<String, String>()
    private val boxes = HashMap<String, SnapshotStateList<Attachment<JsonObject>>>()

    operator fun get(id: String): String = held[id].orEmpty()

    operator fun set(id: String, text: String) {
        if (text.isBlank()) held.remove(id) else held[id] = text
    }

    /** The words of [to], taking those written under [from] first: a thread made here has the PC's id now. */
    fun follow(from: String, to: String): String {
        if (from != to) held.remove(from)?.let { if (to !in held) held[to] = it }
        return get(to)
    }

    /**
     * The pictures in [to]'s message box, those picked under [from] moved to it first, as
     * [follow] moves the words. Each is the image `thread.upload` answered once it is on the PC.
     */
    fun pictures(from: String, to: String): SnapshotStateList<Attachment<JsonObject>> {
        if (from != to) boxes.remove(from)?.let { if (to !in boxes) boxes[to] = it }
        return boxes.getOrPut(to) { mutableStateListOf() }
    }

    /** A deleted thread's words and pictures go with it. */
    fun drop(id: String) {
        held.remove(id)
        boxes.remove(id)
    }
}

/** A `thread.send`: the words, and the pictures by the ids the PC gave them; text may be empty with pictures. */
private fun message(thread: JsonElement, text: String, images: List<JsonElement>) = buildJsonObject {
    put("id", thread)
    put("text", text)
    if (images.isNotEmpty()) put("images", JsonArray(images))
}

/**
 * Whether a message box's pictures may go now, and what it says when not: one still on its way,
 * one the PC did not take, or a model that cannot read them (one of the PC's own, [local]).
 */
private fun picturesWait(pictures: List<Attachment<JsonObject>>, model: String?, local: LocalModels): String? = when {
    pictures.any { it.sending } -> "A picture is still on its way to the PC"
    pictures.any { it.failed } -> "A picture did not reach the PC: take it out, or pick it again"
    pictures.isNotEmpty() && !local.readsPictures(model) -> if (local.has(model)) LOCAL_CANNOT_READ else "$PICTURES_NEED_CLAUDE: take them out, or pick one"
    else -> null
}

/** The ids of the pictures on the PC, for a message's `images`. */
private fun imageIds(pictures: List<Attachment<JsonObject>>): List<JsonElement> = pictures.mapNotNull { it.sent?.get("id") }

/**
 * Why the PC would not take a message, in words: its own sentence, and plainly for pictures a
 * model on the PC cannot read (`thread.images_unsupported`).
 */
private fun refusal(e: BusError?): String = when (e?.code) {
    "thread.images_unsupported" -> "pictures need a Claude model, and this thread's is on the PC. Pick a Claude model, or send it without them"
    else -> e?.message?.ifBlank { null } ?: "the PC refused it"
}

/**
 * Why a thread cannot answer, from `provider.list` (threads_view.rs `check_claude`), and whether
 * it keeps the message from going: without Claude Code nothing answers; a signed-out reading is
 * only as fresh as the last refresh on the PC, so it warns and still lets the person try. Null
 * when Claude Code is ready, or the list was never read.
 */
private fun claudeNote(providers: JsonElement?): Pair<String, Boolean>? {
    val list = (providers as? JsonObject)?.arr("providers") ?: return null
    val claude = list.firstOrNull { (it as? JsonObject)?.s("provider") == "claude" } as? JsonObject
    return when {
        claude == null || claude.b("installed") != true -> "Threads need Claude Code on this PC. Install it, then refresh it in Settings." to true
        claude.s("version") != null && claude.s("signed_in_as") == null -> "Claude Code is not signed in on this PC. Sign in, then refresh it in Settings." to false
        else -> null
    }
}

/**
 * A quiet note by the message box. The dot sits beside the words, read left to right: centred
 * over two lines, it was left alone at the edge.
 */
@Composable
private fun NoteRow(text: String, modifier: Modifier) {
    val c = Relay.colors
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        Dot(c.waiting, 6.dp, Modifier.padding(top = 6.dp))
        T(text, Modifier.weight(1f), style = Relay.type.caption, color = c.ink3)
    }
}

// ---- A new thread ----

/**
 * The Threads space's home (threads_view.rs, the new-thread view): a greeting, the month in a
 * line, the message box and four questions to start. Sending makes the thread, through the
 * outbox, so it starts with the PC away too; its agent answers once the PC has it.
 */
@Composable
fun ThreadsHome(nav: Nav) {
    SpaceFrame(nav) { NewThread(nav) }
}

@Composable
private fun NewThread(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val summary by remember { nav.relay.live("money.summary") }.collectAsStateWithLifecycle(RelayData.Live())
    val invest by remember { nav.relay.live("money.invest.summary") }.collectAsStateWithLifecycle(RelayData.Live())
    val providers by remember { nav.relay.live("provider.list") }.collectAsStateWithLifecycle(RelayData.Live())
    val claude = remember(providers.result) { claudeNote(providers.result) }
    val blocked = claude?.second == true
    // What was written here waits for the next new thread, after Back or another thread too.
    var draft by rememberSaveable { mutableStateOf(Drafts[NEW]) }
    SideEffect { Drafts[NEW] = draft }
    // Its pictures too, each on the PC as soon as it is picked.
    val app = LocalContext.current.applicationContext
    val attached = remember { Drafts.pictures(NEW, NEW) }
    val pickPictures = rememberPicturePicker(attached) { jpeg -> ThreadPictures.upload(app, nav, jpeg) }
    // A new thread starts with the last choice: the newest thread's model and effort.
    val last = s.threads.firstOrNull()
    var model by rememberSaveable { mutableStateOf<String?>(null) }
    var effort by rememberSaveable { mutableStateOf<String?>(null) }
    val m = model ?: last?.model.orEmpty()
    val e = effort ?: last?.effort.orEmpty()
    val local = rememberLocalModels()
    val hello = remember(s.prefs.name) { greeting(s.prefs.name) }
    // The line and the questions follow the source showing: Avex's are read from its export;
    // Arbiter's new thread keeps Tally's (docs/THREADS.md, "One source switch").
    val source = ThreadSource.key
    val avex = source == "avex"
    val gym by remember(avex) { if (avex) nav.relay.live("gym.summary") else flowOf(RelayData.Live()) }.collectAsStateWithLifecycle(RelayData.Live())
    val month = remember(avex, summary.result, gym.result, c) {
        if (avex) (gym.result as? JsonObject)?.let { AnnotatedString(avexLine(it)) } else (summary.result as? JsonObject)?.let { monthLine(it, c) }
    }
    val asks = remember(avex, summary.result, invest.result, gym.result) {
        if (avex) avexSuggestions(gym.result as? JsonObject) else suggestions(summary.result as? JsonObject, invest.result as? JsonObject)
    }

    // The message box sends its pictures with its words; a starter question sends only its own.
    fun start(text: String, withPictures: Boolean = false) {
        val t = text.trim()
        if (blocked) return
        val pictures = if (withPictures) attached.toList() else emptyList()
        picturesWait(pictures, m, local)?.let { return nav.shell.toast(it) }
        val images = imageIds(pictures)
        if (t.isEmpty() && images.isEmpty()) return
        val payload = buildJsonObject {
            put("text", t)
            if (images.isNotEmpty()) put("images", JsonArray(images))
            if (m.isNotEmpty()) put("model", m)
            if (e.isNotEmpty()) put("effort", e)
        }
        // A starter question leaves what was typed in the box: only the words sent go.
        if (t == draft.trim()) draft = ""
        if (withPictures) attached.clear()
        nav.shell.change("thread.create", payload, "New thread") { change ->
            when (change) {
                is Hub.Change.Queued -> nav.thread(Optimistic.tempId(change.entry.id))
                is Hub.Change.Now -> (change.result as? JsonObject)?.l("id")?.let { nav.thread(it.toString()) }
            }
        }
    }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val one = maxWidth < 600.dp
        val side = gutter(maxWidth)
        val tall = maxHeight
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
            Column(
                Modifier.heightIn(min = tall).column().fillMaxWidth().padding(horizontal = side, vertical = 24.dp),
                verticalArrangement = Arrangement.Center,
            ) {
                // What the new thread is about: its line, its questions and what the agent may do follow.
                Box(Modifier.fillMaxWidth().padding(bottom = 18.dp), contentAlignment = Alignment.Center) {
                    Segmented(THREAD_SOURCES, source, { ThreadSource.key = it })
                }
                T(hello, Modifier.fillMaxWidth(), Relay.type.heading.copy(fontSize = 24.sp, lineHeight = 30.sp, letterSpacing = (-0.02).em, textAlign = TextAlign.Center), c.ink)
                month?.let {
                    androidx.compose.foundation.text.BasicText(
                        it,
                        Modifier.fillMaxWidth().padding(top = 8.dp),
                        style = Relay.type.ui.copy(color = c.ink3, textAlign = TextAlign.Center, lineHeight = 21.sp),
                    )
                }
                Box(Modifier.height(22.dp))
                val dictate = com.quietsoftware.relay.ui.dev.rememberDictation { said -> draft = com.quietsoftware.relay.ui.dev.joinSaid(draft, said) }
                Composer(
                    value = draft,
                    onValue = { draft = it },
                    onDictate = dictate,
                    model = m,
                    effort = e,
                    onModel = {
                        model = it
                        nav.pickModel(it, local)
                    },
                    onEffort = { effort = it },
                    working = false,
                    onSend = { start(draft, withPictures = true) },
                    onStop = {},
                    canSend = !blocked,
                    pictures = attached,
                    onPick = pickPictures,
                    onUnpick = { id -> attached.removeAll { it.id == id } },
                    online = s.online,
                    local = local,
                    limits = sourceLimits(source),
                )
                if (!s.online) NoteRow("The PC is out of reach. A new thread waits on this phone and starts when it is back.", Modifier.padding(top = 12.dp, start = 4.dp, end = 4.dp))
                claude?.let { (why, _) -> NoteRow(why, Modifier.padding(top = 12.dp, start = 4.dp, end = 4.dp)) }
                Box(Modifier.height(20.dp))
                val perRow = if (one) 1 else 2
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    for (chunk in asks.chunked(perRow)) Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        for ((ask, about) in chunk) Starter(ask, about, Modifier.weight(1f).fillMaxHeight(), enabled = !blocked) { start(ask) }
                        if (chunk.size < perRow) Box(Modifier.weight(1f))
                    }
                }
            }
        }
    }
}

@Composable
private fun Starter(ask: String, about: String, modifier: Modifier, enabled: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    Column(
        modifier.alpha(if (enabled) 1f else .4f).clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        T(ask, style = Relay.type.ui, color = c.ink, weight = FontWeight.Medium)
        T(about, style = Relay.type.caption, color = c.ink3)
    }
}

private fun greeting(name: String): String {
    val part = when (LocalTime.now().hour) {
        in 5..11 -> "Good morning"
        in 12..17 -> "Good afternoon"
        in 18..22 -> "Good evening"
        else -> "Good night"
    }
    val first = name.trim().substringBefore(' ')
    return if (first.isEmpty()) part else "$part, $first"
}

/** "$412 left in October · on pace", from Tally's summary; nothing for an empty ledger. */
private fun monthLine(s: JsonObject, c: Palette): AnnotatedString? {
    if (s.b("empty") == true) return null
    val money = Money.of(s)
    val pace = s.o("pace")
    val name = periodName(s.o("period")).let { if (it.contains(" to ")) "this period" else "in $it" }
    val budget = pace?.l("budget") ?: 0L
    return buildAnnotatedString {
        if (pace != null && budget > 0) {
            val left = pace.l("remaining") ?: 0L
            append(if (left < 0) "${money.whole(-left)} over $name" else "${money.whole(left)} left $name")
            val (word, tone) = when (pace.s("status")) {
                "UNDER_PACE" -> "under pace" to c.ink3
                "ON_PACE" -> "on pace" to c.ink3
                "OVER_PACE" -> "ahead of pace" to c.waiting
                "OVER_BUDGET" -> "over budget" to c.heldText
                else -> null to c.ink3
            }
            if (word != null) {
                append(" · ")
                withStyle(SpanStyle(color = tone)) { append(word) }
            }
        } else {
            append("${money.whole(s.l("spent") ?: 0L)} spent $name")
        }
    }
}

/**
 * The questions a new thread offers, from what the ledger shows (threads_view.rs `suggestions`):
 * the budget furthest ahead of its pace, a bill due within three days, the investments, then the
 * usual four. At most four.
 */
private fun suggestions(summary: JsonObject?, invest: JsonObject?): List<Pair<String, String>> {
    val out = ArrayList<Pair<String, String>>()
    if (summary != null) {
        if (summary.b("empty") == true) {
            out += "How do I start with Tally?" to "Accounts, a budget and your first entries"
        } else {
            val money = Money.of(summary)
            fun used(b: JsonObject) = (b.l("spent") ?: 0L).toDouble() / maxOf(b.l("budget") ?: 1L, 1L)
            summary.objs("budgets").filter { it.s("status") in setOf("OVER_PACE", "OVER_BUDGET") }.maxByOrNull(::used)?.let { b ->
                val name = b.s("name") ?: "this budget"
                val how = if (b.s("status") == "OVER_BUDGET") "over budget" else "ahead of pace"
                val period = summary.o("period")
                val days = period?.l("days") ?: 0L
                val day = (days - (period?.l("days_left") ?: 0L) + 1).coerceIn(1L, maxOf(days, 1L))
                out += "Why is $name $how?" to "${money.whole(b.l("spent") ?: 0L)} of ${money.whole(b.l("budget") ?: 0L)} on day $day"
            }
            summary.objs("bills").firstOrNull { it.s("type") != "INCOME" && (it.l("days_until") ?: 99L) in 0L..3L }?.let { b ->
                val name = b.s("name") ?: "this bill"
                val whenText = when (b.l("days_until")) {
                    0L -> "today"
                    1L -> "tomorrow"
                    else -> "on ${humanDate(b.s("next_date"))}"
                }
                out += "Can I cover $name $whenText?" to "${money.format(b.l("amount") ?: 0L)} · ${b.str("due_line")}"
            }
        }
    }
    when {
        invest == null -> Unit
        // The desktop offers "Connect my Wealthsimple accounts" here: it ends in choosing the
        // files to import, which the phone cannot do.
        invest.b("empty") == true -> Unit
        else -> out += "How are my investments doing?" to "Value, gain and the room left this year"
    }
    for (s in SUGGESTIONS) if (out.size < 4) out += s
    return out.take(4)
}

// ---- A thread ----

/** Where a tool call stands: called, answered, refused, or cut off by a stop with no answer. */
private enum class Step { Running, Done, Failed, Cut }

private sealed interface CardData {
    data class Entry(val tx: JsonObject, val op: String) : CardData
    data class Activity(val a: JsonObject) : CardData
    data class Proposal(val p: JsonObject) : CardData
    data class Order(val o: JsonObject) : CardData
    data class Refused(val message: String) : CardData
    /** What the agent asked first (`thread.propose`): only the proposal's id; the PC says the rest. */
    data class Asked(val id: Long) : CardData
}

/** The thread's proposals as the PC last read them, for the cards of what the agent asked first. */
private class AskedReading(val byId: Map<Long, JsonObject>, val live: RelayData.Live, val reread: () -> Unit) {
    companion object {
        val None = AskedReading(emptyMap(), RelayData.Live()) {}
    }
}

private data class ToolCall(val key: String, val name: String, val op: String, val writes: Boolean, val step: Step, val answer: String?, val card: CardData?)

private sealed interface Piece {
    val key: String

    data class Day(override val key: String, val label: String) : Piece
    data class User(override val key: String, val text: String, val queued: Boolean, val pictures: List<Picture> = emptyList()) : Piece
    data class Reply(override val key: String, val text: String) : Piece
    data class Tool(override val key: String, val call: ToolCall) : Piece
    /** Three or more reads in a row, folded under the first: "Read the budget and 2 more". */
    data class Reads(override val key: String, val calls: List<ToolCall>) : Piece
    data class Error(override val key: String, val text: String) : Piece
    /** The end of a finished turn: a quiet Copy key for its words. */
    data class Copy(override val key: String, val text: String) : Piece
}

/** The op an agent's tool call ran: `mcp__relay__money_tx_add` is `money.tx.add`. */
private fun toolOp(name: String): String = name.removePrefix("mcp__relay__").takeIf { it != name }?.replace('_', '.').orEmpty()

private val WRITES = setOf("money.tx.add", "money.tx.update", "money.tx.restore", "money.invest.add", "arbiter.propose", "arbiter.order.place", "arbiter.halt")

/** What a tool call did, in words (relay_client::thread_view::tool_caption, arbiter_pages.rs). */
private fun toolCaption(name: String, op: String): String = when (op) {
    "money.summary" -> "Read the budget"
    "money.lists" -> "Read accounts and categories"
    "money.tx.list" -> "Read entries"
    "money.series" -> "Added up spending"
    "money.tx.add" -> "Added an entry"
    "money.tx.update" -> "Changed an entry"
    "money.tx.restore" -> "Restored an entry"
    "money.invest.summary" -> "Read your investments"
    "money.invest.list" -> "Listed investment activity"
    "money.invest.add" -> "Recorded an investment activity"
    "thread.propose" -> "Asked you first"
    "gym.summary" -> "Read your training"
    "gym.sessions" -> "Listed workouts"
    "gym.session.get" -> "Read a workout"
    "gym.lifts" -> "Read your lifts"
    "gym.lift.get" -> "Read a lift's history"
    "gym.cardio" -> "Read cardio"
    "gym.series" -> "Added up training"
    "arbiter.summary" -> "Read Arbiter"
    "arbiter.strategy.get" -> "Read a strategy"
    "arbiter.order.list" -> "Read orders"
    "arbiter.decision.list" -> "Read the decision log"
    "arbiter.products" -> "Looked up products"
    "arbiter.series" -> "Read prices"
    "arbiter.backtest" -> "Ran a backtest"
    "arbiter.proposal.list" -> "Read proposals"
    "arbiter.settings.get" -> "Read Arbiter's settings"
    "arbiter.propose" -> "Proposed a change"
    "arbiter.order.place" -> "Asked for an order"
    "arbiter.halt" -> "Halted Arbiter"
    "bus.schema" -> "Checked how a tool works"
    "" -> "Used $name"
    else -> "Used $op"
}

/** What a tool's line says while it runs. */
private fun doing(op: String): String = when (op) {
    "money.summary" -> "Reading the budget…"
    "money.lists" -> "Reading accounts and categories…"
    "money.tx.list" -> "Reading entries…"
    "money.series" -> "Adding up spending…"
    "money.tx.add" -> "Adding an entry…"
    "money.tx.update" -> "Changing an entry…"
    "money.tx.restore" -> "Restoring an entry…"
    "money.invest.summary" -> "Reading your investments…"
    "money.invest.list" -> "Reading investment activity…"
    "money.invest.add" -> "Recording investment activity…"
    "thread.propose" -> "Asking you first…"
    "bus.schema" -> "Checking how a tool works…"
    // Avex's and Arbiter's, as the desktop says them (threads_view.rs `doing`).
    "gym.series" -> "Adding up training…"
    "arbiter.backtest" -> "Running a backtest…"
    "arbiter.propose" -> "Proposing a change…"
    "arbiter.order.place" -> "Asking for an order…"
    "arbiter.halt" -> "Halting Arbiter…"
    else -> if (op.startsWith("gym.")) "Reading your training…" else if (op.startsWith("arbiter.")) "Reading Arbiter…" else "Working…"
}

/**
 * A tool's answer as a card: an entry or an activity the agent wrote, Arbiter's proposal or order,
 * or what the agent asked first.
 */
private fun cardOf(op: String, answer: String?): CardData? {
    val v = answer?.let { runCatching { Wire.json.parseToJsonElement(it) as? JsonObject }.getOrNull() } ?: return null
    // Its answer's id is a proposal's, never an entry's. The card takes only that, and reads the
    // rest from the PC (threads_ask.rs), so it shows the ledger, not the agent's account of it.
    if (op == "thread.propose") return v.l("id")?.let { CardData.Asked(it) }
    if (op.startsWith("arbiter.")) return when {
        op == "arbiter.propose" && v.l("id") != null -> CardData.Proposal(v)
        op == "arbiter.order.place" -> when (v.s("outcome")) {
            "proposed" -> v.o("proposal")?.let { CardData.Proposal(it) }
            "placed" -> v.o("order")?.let { CardData.Order(it) }
            "refused" -> CardData.Refused(v.o("refusal")?.s("message") ?: "The limits refused it.")
            else -> null
        }
        v.o("proposal") != null -> CardData.Proposal(v.o("proposal")!!)
        else -> null
    }
    // Only the ledger's writes answer with an entry; a command's output that reads as JSON is not one.
    if (op !in WRITES || v.l("id") == null) return null
    return if (op == "money.invest.add") CardData.Activity(v) else CardData.Entry(v, op)
}

/**
 * A thread's messages as the pieces the conversation draws: day names, the person's bubbles, the
 * agent's words, tool lines (reads in a row folded), cards, errors, and a Copy key per finished
 * turn. A tool call with no answer turns while the agent works and reads as cut off after.
 */
private fun pieces(messages: List<ThreadMessage>, working: Boolean, today: LocalDate): List<Piece> {
    val answers = HashMap<String, Pair<Boolean, String>>()
    for (m in messages) if (m.role == "tool") {
        val b = m.body as? JsonObject ?: continue
        val id = b.s("tool_use_id") ?: continue
        answers[id] = (b.b("is_error") == true) to b.str("text")
    }
    val lastUser = messages.indexOfLast { it.role == "user" }
    val out = ArrayList<Piece>()
    val reads = ArrayList<ToolCall>()
    val words = StringBuilder()
    var turnKey: String? = null
    var lastDay: LocalDate? = null
    fun flushReads() {
        if (reads.size >= 3) out += Piece.Reads("g:" + reads.first().key, reads.toList())
        else reads.forEach { out += Piece.Tool(it.key, it) }
        reads.clear()
    }
    fun endTurn(finished: Boolean) {
        flushReads()
        if (finished && words.isNotEmpty() && turnKey != null) out += Piece.Copy("c:$turnKey", words.toString())
        words.setLength(0)
        turnKey = null
    }
    messages.forEachIndexed { index, m ->
        if (m.role == "tool") return@forEachIndexed
        val day = dayOf(m.createdAt)
        if (day != null && day != lastDay) {
            // A thread that began today names no day: its replies are as fresh as the ledger.
            if (!(lastDay == null && day == today)) {
                endTurn(true)
                out += Piece.Day("d:${m.id}", humanDay(day, today))
            }
            lastDay = day
        }
        when (m.role) {
            "user" -> {
                endTurn(true)
                out += Piece.User("m:${m.id}", m.text, Optimistic.isTemp(m.id), picturesOf(m.body as? JsonObject))
            }
            "assistant" -> ((m.body as? JsonObject)?.arr("blocks") ?: emptyList()).forEachIndexed { i, blk ->
                val o = blk as? JsonObject ?: return@forEachIndexed
                when (o.s("type")) {
                    "text" -> {
                        val t = o.str("text")
                        if (t.isNotBlank()) {
                            flushReads()
                            out += Piece.Reply("m:${m.id}:$i", t)
                            if (words.isNotEmpty()) words.append("\n\n")
                            words.append(t.trim())
                            turnKey = m.id
                        }
                    }
                    "tool_use" -> {
                        val name = o.str("name")
                        val op = toolOp(name)
                        // Asking first stands alone, as a change does: it is what the agent did instead of one.
                        val writes = op in WRITES || op == "thread.propose"
                        val id = o.s("id")
                        val answer = id?.let { answers[it] }
                        val step = when {
                            id == null -> Step.Done
                            answer != null -> if (answer.first) Step.Failed else Step.Done
                            working && index > lastUser -> Step.Running
                            else -> Step.Cut
                        }
                        val card = if (step == Step.Done) cardOf(op, answer?.second) else null
                        val call = ToolCall("t:${id ?: "${m.id}:$i"}", name, op, writes, step, answer?.second, card)
                        if (writes || card != null) {
                            flushReads()
                            out += Piece.Tool(call.key, call)
                        } else {
                            reads += call
                        }
                    }
                }
            }
            "error" -> {
                flushReads()
                out += Piece.Error("m:${m.id}", m.text.ifBlank { "Something went wrong" })
            }
        }
    }
    endTurn(!working)
    return out
}

/** The text an assistant message's deltas wrote: its text blocks, end to end. */
private fun written(m: ThreadMessage): String =
    ((m.body as? JsonObject)?.arr("blocks") ?: emptyList()).mapNotNull { (it as? JsonObject)?.takeIf { o -> o.s("type") == "text" }?.str("text") }.joinToString("")

private class Found(val thread: Thread?)

/**
 * A thread (threads_view.rs): the conversation in one centred column, the agent's reply streaming
 * in as it is written, and the message box under it. A thread the phone made offline (`tmp:`)
 * shows its first message waiting for the PC, and follows the PC's id once the PC has it.
 */
@Composable
fun ThreadScreen(id: String, nav: Nav) {
    val real by remember(id) { nav.relay.resolved(id) }.collectAsStateWithLifecycle(id)
    Page {
        CompositionLocalProvider(LocalFence provides ChartFence) {
            Conversation(real, id, nav)
        }
    }
}

@Composable
private fun Conversation(tid: String, opened: String, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val found by remember(tid) { nav.relay.thread(tid).map { Found(it) } }.collectAsStateWithLifecycle(null)
    val loaded by remember(tid) { nav.relay.messages(tid) }.collectAsStateWithLifecycle<List<ThreadMessage>?>(null)
    val messages = loaded.orEmpty()
    val ledger by remember { nav.relay.cached("money.summary") }.collectAsStateWithLifecycle(null)
    val money = remember(ledger) { Money.of(ledger?.result as? JsonObject) }
    val thread = found?.thread
    val temp = Optimistic.isTemp(tid)
    val entryId = if (temp) tid.removePrefix(Optimistic.TEMP) else null
    val num = thread?.num ?: tid.toLongOrNull()
    val working = thread?.working == true
    val create = entryId?.let { e -> s.outbox.firstOrNull { it.id == e } }
    val createFailed = create?.parked == true
    val scope = rememberCoroutineScope()

    // Messages of a thread the replica has not read in full: read once, into the replica.
    var fetched by remember(tid) { mutableStateOf(false) }
    LaunchedEffect(num, s.online) {
        if (num != null && s.online && !fetched) {
            fetched = true
            nav.relay.hub.syncer.schedule(setOf(Fetch.Thread(num)))
        }
    }

    // The reply as it is written: deltas gather in a buffer, the screen redraws about 30 times a second.
    val buffer = remember(tid) { StringBuilder() }
    var streamed by remember(tid) { mutableStateOf("") }
    var stopping by remember(tid) { mutableStateOf(false) }
    var stopped by remember(tid) { mutableStateOf<String?>(null) }
    // Whether a reply is streaming: read here instead of the text, so only the streaming item redraws.
    val hasStream by remember(tid) { derivedStateOf { streamed.isNotEmpty() } }
    LaunchedEffect(num) {
        val n = num ?: return@LaunchedEffect
        var last = 0L
        var later: Job? = null
        nav.relay.deltas.collect { (thread, chunk) ->
            if (thread != n) return@collect
            buffer.append(chunk)
            val now = SystemClock.uptimeMillis()
            if (now - last >= FRAME) {
                last = now
                streamed = buffer.toString()
            } else if (later?.isActive != true) {
                later = launch {
                    delay(FRAME - (now - last))
                    last = SystemClock.uptimeMillis()
                    streamed = buffer.toString()
                }
            }
        }
    }
    // A stored reply takes the place of the words it streamed.
    var seen by remember(tid) { mutableStateOf<Set<String>?>(null) }
    LaunchedEffect(loaded) {
        val assistants = loaded?.filter { it.role == "assistant" } ?: return@LaunchedEffect
        val known = seen
        seen = assistants.mapTo(HashSet()) { it.id }
        if (known == null) return@LaunchedEffect
        val fresh = assistants.filter { it.id !in known }
        for (m in fresh) {
            val w = written(m).trim()
            if (w.isEmpty()) continue
            val at = buffer.indexOf(w)
            if (at >= 0) buffer.delete(0, at + w.length) else buffer.setLength(0)
        }
        if (fresh.isNotEmpty()) streamed = buffer.toString()
    }
    LaunchedEffect(working) {
        if (working) {
            stopped = null
            return@LaunchedEffect
        }
        // The turn ended: what a stop cut short stays, said so, as does a stop before any words;
        // the rest gives way to the stored reply.
        val lastUser = messages.indexOfLast { it.role == "user" }
        val replied = messages.drop(lastUser + 1).any { it.role == "assistant" && written(it).isNotBlank() }
        val cut = stopping && (buffer.isNotBlank() || !replied)
        stopping = false
        if (cut) stopped = buffer.toString() else delay(600)
        buffer.setLength(0)
        streamed = ""
    }

    val today = remember { LocalDate.now() }
    val rows = remember(messages, working) { pieces(messages, working, today) }
    // The first message of a thread made here: from its create while it waits, and still shown
    // after the PC names the thread until the PC's copy of the message arrives.
    var first by rememberSaveable(opened) { mutableStateOf<String?>(null) }
    val queuedFirst = if (temp) create?.payload?.s("text") ?: thread?.preview else null
    LaunchedEffect(queuedFirst) { if (queuedFirst != null) first = queuedFirst }
    val firstText = (queuedFirst ?: first)?.takeIf { temp || rows.none { r -> r is Piece.User } }
    // Its pictures, by the ids the create names, kept the same way.
    var firstPictures by remember(opened) { mutableStateOf<List<Picture>>(emptyList()) }
    val queuedPictures = if (temp) create?.payload?.let { picturesOf(it) } else null
    LaunchedEffect(queuedPictures) { if (queuedPictures != null) firstPictures = queuedPictures }
    // A message the PC refused stays in the outbox: its bubble says why, in place of "Sending…".
    val refusedSends = remember(s.outbox) { s.outbox.filter { it.op == "thread.send" && it.parked }.associate { "m:" + Optimistic.tempId(it.id) to refusal(it.error) } }
    val anyProposal = rows.any { it is Piece.Tool && it.call.card is CardData.Proposal }
    val proposals by remember(anyProposal) {
        if (anyProposal) nav.relay.live("arbiter.proposal.list", buildJsonObject { put("limit", 200) }) else flowOf(RelayData.Live())
    }.collectAsStateWithLifecycle(RelayData.Live())
    val current = remember(proposals.result) {
        (proposals.result as? JsonObject)?.objs("proposals")?.mapNotNull { p -> p.l("id")?.let { it to p } }?.toMap().orEmpty()
    }
    // What the agent asked first, as the PC reads it: on opening, on `thread.proposal`, and when
    // the ledger its rows show moves. A card whose answer the PC refused reads them again.
    val anyAsked = rows.any { it is Piece.Tool && it.call.card is CardData.Asked }
    var rereads by remember(tid) { mutableIntStateOf(0) }
    val askedLive by remember(anyAsked, num, rereads) {
        if (anyAsked && num != null) nav.relay.liveAlso("thread.proposal.list", buildJsonObject { put("thread_id", num) }, setOf("money"))
        else flowOf(RelayData.Live())
    }.collectAsStateWithLifecycle(RelayData.Live())
    val asked = remember(askedLive) {
        val byId = (askedLive.result as? JsonObject)?.objs("proposals")?.mapNotNull { p -> p.l("id")?.let { it to p } }?.toMap().orEmpty()
        AskedReading(byId, askedLive) { rereads++ }
    }
    val local = rememberLocalModels()
    // A tool line turning says what the agent does; the working row shows only while none does
    // (threads_view.rs `set_working`).
    val running = rows.any { p -> p is Piece.Tool && p.call.step == Step.Running || p is Piece.Reads && p.calls.any { x -> x.step == Step.Running } }
    val doingText = if (thread?.live == true || hasStream) "Working…" else "Starting Claude…"
    val showWorking = working && !running

    val providers by remember { nav.relay.live("provider.list") }.collectAsStateWithLifecycle(RelayData.Live())
    val claude = remember(providers.result) { claudeNote(providers.result) }
    val blocked = claude?.second == true

    // What is written here outlives the screen, and follows a thread made on this phone to its PC id.
    var draft by rememberSaveable(tid) { mutableStateOf(Drafts.follow(opened, tid)) }
    SideEffect { Drafts[tid] = draft }
    // Its pictures as well, each on the PC as soon as it is picked; this phone keeps a copy.
    val app = LocalContext.current.applicationContext
    val attached = remember(tid) { Drafts.pictures(opened, tid) }
    val pickPictures = rememberPicturePicker(attached) { jpeg -> ThreadPictures.upload(app, nav, jpeg) }
    // When the last message went: the send key turns to Stop under the finger that sent it.
    var sentAt by remember { mutableLongStateOf(0L) }
    val focus = remember { FocusRequester() }
    // The one message of the person's showing its keys, if any; Back puts them away first.
    var keysOn by remember(tid) { mutableStateOf<String?>(null) }
    BackHandler(enabled = keysOn != null) { keysOn = null }
    var menu by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    val list = rememberLazyListState()
    // Not keyed on [tid]: the scroll effects below are keyed on [list] and keep the state they
    // first captured, so a new state when a phone-made thread gets its PC id would go unread.
    var follow by remember { mutableStateOf(true) }
    val dragged by list.interactionSource.collectIsDraggedAsState()
    // A drag away from the newest stops following; one that ends at the bottom follows again.
    LaunchedEffect(dragged) { if (dragged) follow = false else if (!list.canScrollForward) follow = true }
    LaunchedEffect(list) { androidx.compose.runtime.snapshotFlow { list.canScrollForward }.collect { if (!it) follow = true } }

    val target: JsonElement? = num?.let { JsonPrimitive(it) } ?: entryId?.let { Refs.ref(it) }
    // The words and the pictures go together; a picture still going holds the message, as in the
    // agent chat. Pictures already on the PC may wait in the outbox with it.
    fun send() {
        val text = draft.trim()
        val t = target ?: return
        if (working || blocked) return
        picturesWait(attached, thread?.model, local)?.let { return nav.shell.toast(it) }
        val images = imageIds(attached)
        if (text.isEmpty() && images.isEmpty()) return
        nav.shell.change("thread.send", message(t, text, images), "Message")
        sentAt = SystemClock.uptimeMillis()
        draft = ""
        attached.clear()
        stopped = null
        follow = true
    }
    // Again as a new message, with the pictures it had: through the outbox, so it waits for the
    // PC like any other.
    fun resend(text: String, pictures: List<Picture>) {
        val t = target ?: return
        if (working) {
            nav.shell.toast("Claude is still answering: stop it first, or wait")
            return
        }
        if (blocked) {
            nav.shell.toast(claude?.first.orEmpty())
            return
        }
        if (pictures.isNotEmpty() && !local.readsPictures(thread?.model)) {
            nav.shell.toast("$PICTURES_NEED_CLAUDE: pick one to send these again")
            return
        }
        keysOn = null
        nav.shell.change("thread.send", message(t, text, pictures.map { it.id }), "Message")
        stopped = null
        follow = true
    }
    val keys = BubbleKeys(
        keysOn,
        toggle = { k -> keysOn = if (keysOn == k) null else k },
        edit = { words ->
            keysOn = null
            draft = joinDraft(draft, words)
            focus.requestFocus()
        },
        resend = if (target != null && !createFailed) ::resend else null,
    )
    fun stop() {
        val n = num ?: return
        // A second tap on Send just after a message is that tap again, not a stop.
        if (SystemClock.uptimeMillis() - sentAt < STOP_AFTER_SEND_MS) return
        stopping = true
        nav.shell.act("thread.stop", buildJsonObject { put("id", n) })
    }
    fun choose(model: String?, effort: String?) {
        val t = target ?: return
        nav.shell.change("thread.set", buildJsonObject {
            put("id", t)
            put("model", model ?: thread?.model.orEmpty())
            put("effort", effort ?: thread?.effort.orEmpty())
        }, "Model and effort")
        model?.let { nav.pickModel(it, local) }
    }
    // Try again (threads_view.rs `retry_key`): under the newest failed or stopped reply, until the
    // next message, sending the person's newest message once more.
    val lastUser = rows.lastOrNull { it is Piece.User } as? Piece.User
    val lastWords = lastUser?.text ?: firstText
    val lastPictures = lastUser?.pictures ?: firstPictures
    val retryAt = if (stopped != null) "stopped" else rows.lastOrNull { it is Piece.Error || it is Piece.User }?.takeIf { it is Piece.Error }?.key
    val retry: (() -> Unit)? = lastWords?.takeIf { target != null && !createFailed && (it.isNotBlank() || lastPictures.isNotEmpty()) }?.let { words -> { resend(words, lastPictures) } }

    // Nothing to show yet: no message, nothing streaming or waiting. Reading one the PC has
    // messages for says so for about ten seconds, then that it could not be read.
    val blank = loaded != null && found != null && rows.isEmpty() && firstText == null && !temp && !working && stopped == null && !hasStream
    val reading = blank && s.online && !thread?.preview.isNullOrBlank()
    var tries by remember(tid) { mutableIntStateOf(0) }
    var slow by remember(tid) { mutableStateOf(false) }
    LaunchedEffect(reading, tries) {
        slow = false
        if (reading) {
            delay(10_000)
            slow = true
        }
    }

    val subtitle = when {
        createFailed -> "Not started"
        temp -> "Waiting for the PC"
        !s.online -> "Offline · what this phone saved"
        else -> null
    }
    val extra = if (firstText != null) 1 else 0
    val tail = listOfNotNull(
        stopped?.let { "stopped" },
        "stream".takeIf { hasStream },
        "working".takeIf { showWorking },
        "waiting".takeIf { temp },
    )
    val count = extra + rows.size + tail.size
    // Follow the newest while the person has not scrolled away: on every new item and as the last one grows.
    LaunchedEffect(list) {
        androidx.compose.runtime.snapshotFlow {
            val info = list.layoutInfo
            Triple(info.totalItemsCount, info.visibleItemsInfo.lastOrNull()?.index, info.visibleItemsInfo.lastOrNull()?.size)
        }.collect { (total, _, _) ->
            if (!follow || total == 0) return@collect
            try {
                list.toEnd(total)
            } catch (e: CancellationException) {
                // A scroll of the person's own took over; this goes on unless it was cancelled itself.
                currentCoroutineContext().ensureActive()
            }
        }
    }
    val jump by remember { derivedStateOf { !follow && list.canScrollForward } }

    Column(Modifier.fillMaxSize()) {
        PageBar(thread?.title?.ifBlank { null } ?: if (temp) "New thread" else "Thread", nav::back, subtitle) {
            Box {
                IconKey("more", { menu = true }, tint = c.ink2, enabled = thread != null || create != null, label = "Thread actions")
                Choices(
                    open = menu,
                    onDismiss = { menu = false },
                    options = listOf("rename" to "Rename", "delete" to "Delete thread"),
                    selected = null,
                    onPick = { if (it == "rename") renaming = true else deleting = true },
                    above = false,
                    anchor = TOUCH,
                    glyphs = mapOf("rename" to "edit", "delete" to "trash"),
                    danger = setOf("delete"),
                )
            }
        }
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val side = gutter(maxWidth)
            val cell = Modifier.column().fillMaxWidth().padding(horizontal = side)
            when {
                found != null && thread == null && !temp -> Empty("threads", "This thread is not here", "It may have been deleted on the PC.") {
                    Key("New thread", { nav.newThread() }, glyph = "plus")
                }
                found != null && thread == null && create == null -> Empty("threads", "This thread was not started", "Its first message was dropped before the PC had it.") {
                    Key("New thread", { nav.newThread() }, glyph = "plus")
                }
                // Only once the phone's copy was read: before, every thread looked empty for a moment.
                blank -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    when {
                        !s.online -> Empty("threads", "Not on this phone yet", "Its messages arrive when the PC is back.")
                        thread?.preview.isNullOrBlank() -> Empty("threads", "No messages yet", "Ask something below.")
                        slow -> Empty("threads", "Could not read this conversation", "The PC did not send its messages.") {
                            Key("Try again", {
                                tries++
                                num?.let { scope.launch { nav.relay.hub.syncer.schedule(setOf(Fetch.Thread(it))) } }
                            }, glyph = "refresh")
                        }
                        else -> T("Reading the conversation…", style = Relay.type.caption, color = c.ink3)
                    }
                }
                else -> LazyColumn(
                    state = list,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(top = 8.dp, bottom = 20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    if (firstText != null) item("first") { UserBubble(firstText, queued = false, online = s.online, modifier = cell.padding(top = 12.dp), pictures = firstPictures) }
                    items(rows, key = { it.key }, contentType = { it::class }) { p -> PieceView(p, cell, s.online, money, current, keys, retry.takeIf { p.key == retryAt }, !working, refusedSends[p.key], asked) }
                    stopped?.let { text ->
                        item("stopped") {
                            Column(cell.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                if (text.isNotBlank()) Markdown(text)
                                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    T("Stopped", style = Relay.type.caption, color = c.ink3)
                                    retry?.let { RetryKey(!working, it) }
                                }
                            }
                        }
                    }
                    if (hasStream) item("stream") { Markdown(streamed, cell.padding(top = 12.dp)) }
                    if (showWorking) item("working") { WorkingRow(doingText, cell.padding(top = 12.dp)) }
                    if (temp) item("waiting") { Waiting(create, s.online, cell.padding(top = 14.dp)) { nav.outbox() } }
                }
            }
            if (jump) {
                Box(
                    Modifier.align(Alignment.BottomCenter).padding(bottom = 10.dp).size(TOUCH).clip(CircleShape)
                        .clickable(role = Role.Button) {
                            follow = true
                            scope.launch { list.toEnd(count) }
                        }
                        .semantics { contentDescription = "Go to the newest" },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(Modifier.size(36.dp).shadow(10.dp, CircleShape).clip(CircleShape).background(c.slab).border(1.dp, c.strong, CircleShape), contentAlignment = Alignment.Center) {
                        Glyph("arrow-down", 16.dp, c.ink2)
                    }
                }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth().navigationBarsPadding().imePadding().padding(top = 4.dp, bottom = 8.dp), contentAlignment = Alignment.TopCenter) {
            val dictate = com.quietsoftware.relay.ui.dev.rememberDictation { said -> draft = com.quietsoftware.relay.ui.dev.joinSaid(draft, said) }
            Column(Modifier.column().fillMaxWidth().padding(horizontal = gutter(maxWidth))) {
                claude?.let { (why, _) -> NoteRow(why, Modifier.padding(start = 4.dp, end = 4.dp, bottom = 8.dp)) }
                Composer(
                    value = draft,
                    onValue = { draft = it },
                    onDictate = dictate,
                    model = thread?.model.orEmpty(),
                    effort = thread?.effort.orEmpty(),
                    onModel = { choose(it, null) },
                    onEffort = { choose(null, it) },
                    working = working,
                    onSend = ::send,
                    onStop = ::stop,
                    canSend = target != null && !createFailed && !blocked,
                    canPick = !working && target != null,
                    focus = focus,
                    hint = sourceHint(ThreadSource.key),
                    pictures = attached,
                    onPick = pickPictures,
                    onUnpick = { id -> attached.removeAll { it.id == id } },
                    online = s.online,
                    local = local,
                    limits = sourceLimits(ThreadSource.key),
                )
            }
        }
    }

    if (renaming) RenameDialog(thread?.title.orEmpty(), { renaming = false }) { title ->
        target?.let { nav.shell.change("thread.rename", buildJsonObject { put("id", it); put("title", title) }, "Rename thread") }
    }
    if (deleting) ConfirmDialog(
        title = "Delete this thread?",
        body = if (temp) "Its messages have not reached the PC; they are dropped from this phone." else "Its messages go too, on the PC as well. There is no undo.",
        action = "Delete",
        onDismiss = { deleting = false },
    ) {
        // Its unsent words and pictures go with it.
        draft = ""
        Drafts.drop(tid)
        if (temp && entryId != null) {
            val drop = s.outbox.filter { it.id == entryId || entryId in Refs.of(it.payload) }.map(OutboxEntry::id)
            scope.launch { drop.forEach { nav.relay.discard(it) } }
        } else if (num != null) {
            nav.shell.change("thread.delete", buildJsonObject { put("id", num) }, "Delete thread")
        }
        nav.back()
    }
}

/** What a message of the person's offers under it, and which one shows its keys. */
private class BubbleKeys(val opened: String?, val toggle: ((String) -> Unit)?, val edit: ((String) -> Unit)?, val resend: ((String, List<Picture>) -> Unit)?) {
    companion object {
        val None = BubbleKeys(null, null, null, null)
    }
}

/** Keep the newest piece in view: the last item, then to its very end. */
private suspend fun LazyListState.toEnd(count: Int) {
    if (count <= 0) return
    val lastShown = layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1
    if (lastShown < count - 2) scrollToItem(count - 1)
    scrollBy(100_000f)
}

/** Try again, after a reply that failed or was stopped; off while the agent works. */
@Composable
private fun RetryKey(enabled: Boolean, onClick: () -> Unit) = Key("Try again", onClick, kind = KeyKind.Quiet, glyph = "refresh", enabled = enabled)

@Composable
private fun PieceView(
    p: Piece,
    cell: Modifier,
    online: Boolean,
    money: Money,
    proposals: Map<Long, JsonObject>,
    keys: BubbleKeys = BubbleKeys.None,
    retry: (() -> Unit)? = null,
    idle: Boolean = true,
    refused: String? = null,
    asked: AskedReading = AskedReading.None,
) {
    val c = Relay.colors
    when (p) {
        is Piece.Day -> T(p.label, cell.padding(top = 18.dp, bottom = 2.dp), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
        is Piece.User -> UserBubble(
            p.text,
            p.queued,
            online,
            cell.padding(top = 18.dp),
            open = keys.opened == p.key,
            onOpen = keys.toggle?.let { t -> { t(p.key) } },
            onEdit = keys.edit,
            onResend = keys.resend?.takeIf { !p.queued }?.let { again -> { again(p.text, p.pictures) } },
            pictures = p.pictures,
            refused = refused,
        )
        is Piece.Reply -> Markdown(p.text, cell.padding(top = 12.dp))
        is Piece.Tool -> Box(cell.padding(top = if (p.call.card != null) 10.dp else 2.dp)) { ToolView(p.call, money, proposals, asked) }
        is Piece.Reads -> Box(cell.padding(top = 2.dp)) { ReadsView(p, money, proposals, asked) }
        is Piece.Error -> Column(cell.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Glyph("close", 13.dp, c.heldText, Modifier.padding(top = 4.dp))
                SelectionContainer { T(p.text, style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 20.sp), color = c.heldText) }
            }
            retry?.let { RetryKey(idle, it) }
        }
        is Piece.Copy -> Box(cell) { CopyKey(p.text) }
    }
}

/**
 * The person's message: its pictures, then a bubble on the right, at most about six sevenths of
 * the column. A tap or a long press on the words shows their keys under them (the agent chat's):
 * Copy, Edit and Send again; a tap on a picture opens it. One the PC [refused] says why under it.
 */
@Composable
private fun UserBubble(
    text: String,
    queued: Boolean,
    online: Boolean,
    modifier: Modifier,
    open: Boolean = false,
    onOpen: (() -> Unit)? = null,
    onEdit: ((String) -> Unit)? = null,
    onResend: (() -> Unit)? = null,
    pictures: List<Picture> = emptyList(),
    refused: String? = null,
) {
    val c = Relay.colors
    Column(modifier, horizontalAlignment = Alignment.End) {
        if (pictures.isNotEmpty()) Box(Modifier.fillMaxWidth(.86f), contentAlignment = Alignment.CenterEnd) {
            SentPictures(pictures, Modifier.padding(bottom = if (text.isNotBlank()) 6.dp else 0.dp))
        }
        if (text.isNotBlank() || pictures.isEmpty()) Box(Modifier.fillMaxWidth(.86f), contentAlignment = Alignment.CenterEnd) {
            Box(
                Modifier.clip(Radii.card).background(c.wash).then(if (open) Modifier.border(1.dp, c.strong, Radii.card) else Modifier)
                    .messageTouch(open, LocalHapticFeedback.current, onOpen).padding(horizontal = 14.dp, vertical = 10.dp),
            ) {
                T(text, style = Relay.type.body, color = c.ink)
            }
        }
        MessageKeys(open, text, onClose = { onOpen?.invoke() }, onEdit = onEdit, onResend = onResend)
        if (refused != null) Row(Modifier.padding(top = 5.dp, end = 4.dp).fillMaxWidth(.86f), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End)) {
            Dot(c.held, 5.dp, Modifier.padding(top = 6.dp))
            T("Not sent: $refused", Modifier.weight(1f, fill = false), Relay.type.caption.copy(fontSize = 11.5.sp, textAlign = TextAlign.End), c.heldText)
        } else if (queued) Row(Modifier.padding(top = 5.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Dot(c.waiting, 5.dp)
            T(if (online) "Sending…" else "Waiting for the PC", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
        }
    }
}

/** The working row: the live lamp and what the agent is doing. */
@Composable
private fun WorkingRow(text: String, modifier: Modifier) {
    Row(modifier.heightIn(min = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(9.dp)) {
        WorkingLamp()
        T(text, style = Relay.type.caption, color = Relay.colors.ink3)
    }
}

/** A thread made on this phone, until the PC has it; or why the PC would not start it. */
@Composable
private fun Waiting(create: OutboxEntry?, online: Boolean, modifier: Modifier, onOutbox: () -> Unit) {
    val c = Relay.colors
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dot(if (create?.parked == true) c.held else c.waiting, 6.dp)
            T(
                when {
                    create?.parked == true -> "Not started: ${refusal(create.error)}"
                    online -> "Sending to the PC…"
                    else -> "Waiting for the PC · its agent answers once the PC is back"
                },
                Modifier.weight(1f), Relay.type.caption, if (create?.parked == true) c.heldText else c.ink3,
            )
        }
        if (create?.parked == true) Key("Open the outbox", onOutbox, glyph = "outbox")
    }
}

@Composable
private fun StepMark(step: Step, writes: Boolean) {
    val c = Relay.colors
    Box(Modifier.size(14.dp), contentAlignment = Alignment.Center) {
        when (step) {
            Step.Running -> Spinner(11.dp)
            Step.Done -> Glyph(if (writes) "check" else "search", 13.dp, c.inkDim)
            Step.Failed -> Glyph("close", 13.dp, c.heldText)
            Step.Cut -> Glyph("minus", 13.dp, c.inkDim)
        }
    }
}

/** A tool call: a quiet line saying what it did, its refusal under it, or the card its answer became. */
@Composable
private fun ToolView(t: ToolCall, money: Money, proposals: Map<Long, JsonObject>, asked: AskedReading) {
    val c = Relay.colors
    when (val card = t.card) {
        is CardData.Entry -> EntryCard(card.tx, card.op, money)
        is CardData.Activity -> ActivityCard(card.a)
        is CardData.Proposal -> ProposalCard(card.p, card.p.l("id")?.let { proposals[it] })
        is CardData.Order -> Column(Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(horizontal = 14.dp)) { OrderLine(card.o, divider = false) }
        is CardData.Refused -> RefusalLine(card.message)
        is CardData.Asked -> AskedCard(card.id, asked.byId[card.id], asked.live, asked.reread)
        null -> Column {
            Row(Modifier.heightIn(min = 26.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                StepMark(t.step, t.writes)
                // While it runs the line says what the agent is doing, and is the only working line
                // (threads_view.rs `draw_tool`): the working row under it said the same again.
                val now = doing(t.op)
                T(if (t.step == Step.Running && now != "Working…") now else toolCaption(t.name, t.op), style = Relay.type.caption, color = if (t.step == Step.Failed) c.heldText else c.ink3, maxLines = 1)
            }
            if (t.step == Step.Failed) {
                // The engine's own sentence, never the JSON Relay's MCP server wraps it in.
                val text = refusalWords(t.answer.orEmpty())
                if (t.op == "arbiter.order.place" || t.op == "arbiter.propose") RefusalLine(text)
                else T(text.ifEmpty { "It was refused." }, Modifier.padding(start = 21.dp, bottom = 2.dp), Relay.type.caption, c.ink3, maxLines = 2)
            }
        }
    }
}

/** Reads in a row: from three, one line naming the first, opening to show them all. */
@Composable
private fun ReadsView(g: Piece.Reads, money: Money, proposals: Map<Long, JsonObject>, asked: AskedReading) {
    val c = Relay.colors
    var open by rememberSaveable(g.key) { mutableStateOf(false) }
    val step = when {
        g.calls.any { it.step == Step.Running } -> Step.Running
        g.calls.any { it.step == Step.Failed } -> Step.Failed
        else -> Step.Done
    }
    Column {
        Row(
            Modifier.heightIn(min = TOUCH).clip(Radii.icon).clickable(role = Role.Button) { open = !open }
                .semantics { stateDescription = if (open) "Expanded" else "Collapsed" }.padding(end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(7.dp),
        ) {
            StepMark(step, false)
            T("${toolCaption(g.calls.first().name, g.calls.first().op)} and ${g.calls.size - 1} more", style = Relay.type.caption, color = if (step == Step.Failed) c.heldText else c.ink3, maxLines = 1)
            Glyph(if (open) "chevron-up" else "chevron-down", 12.dp, c.inkDim)
        }
        AnimatedVisibility(open) {
            Column(Modifier.padding(start = 2.dp, bottom = 4.dp)) { for (t in g.calls) ToolView(t, money, proposals, asked) }
        }
    }
}

/** A finished turn's Copy key. */
@Composable
private fun CopyKey(text: String) {
    val c = Relay.colors
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var copied by remember { mutableStateOf(false) }
    Box(
        Modifier.size(TOUCH).clip(Radii.icon).clickable(role = Role.Button) {
            scope.launch {
                clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("reply", text)))
                copied = true
                delay(1400)
                copied = false
            }
        }.semantics { contentDescription = if (copied) "Copied" else "Copy reply" },
        contentAlignment = Alignment.CenterStart,
    ) {
        Glyph(if (copied) "check" else "copy", 15.dp, c.inkDim)
    }
}

@Composable
private fun DialogCard(content: @Composable () -> Unit) {
    val c = Relay.colors
    Column(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(18.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) { content() }
}

@Composable
private fun RenameDialog(current: String, onDismiss: () -> Unit, onRename: (String) -> Unit) {
    val c = Relay.colors
    // The name opens selected, so typing replaces it (threads_view.rs).
    var name by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue(current, TextRange(0, current.length))) }
    val focus = remember { FocusRequester() }
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    LaunchedEffect(Unit) { focus.requestFocus() }
    Dialog(onDismissRequest = onDismiss) {
        DialogCard {
            T("Rename thread", style = Relay.type.title, color = c.ink)
            val typed = name.text.trim()
            fun done() {
                if (typed.isEmpty()) return
                // Done on the name as it was closes, as Cancel does: there is nothing to rename.
                if (typed != current) onRename(typed)
                onDismiss()
            }
            // Field's look; Field takes only the text, and this needs its selection.
            BasicTextField(
                value = name,
                onValueChange = { name = it },
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
                textStyle = Relay.type.ui.copy(color = c.ink),
                singleLine = true,
                cursorBrush = SolidColor(c.ink),
                interactionSource = source,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { done() }),
                decorationBox = { inner ->
                    Box(
                        Modifier.heightIn(min = 42.dp).clip(Radii.key).background(c.raised)
                            .border(1.dp, if (focused) c.lineFocus else c.strong, Radii.key).padding(horizontal = 12.dp, vertical = 9.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        if (name.text.isEmpty()) T("Thread name", style = Relay.type.ui, color = c.ink3, maxLines = 1)
                        inner()
                    }
                },
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                Key("Cancel", onDismiss, kind = KeyKind.Quiet)
                Key("Rename", { done() }, kind = KeyKind.Primary, enabled = typed.isNotEmpty() && typed != current)
            }
        }
    }
}

@Composable
internal fun ConfirmDialog(title: String, body: String, action: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    val c = Relay.colors
    Dialog(onDismissRequest = onDismiss) {
        DialogCard {
            T(title, style = Relay.type.title, color = c.ink)
            T(body, style = Relay.type.ui.copy(lineHeight = 20.sp), color = c.ink2)
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                Key("Cancel", onDismiss, kind = KeyKind.Quiet)
                Key(action, { onConfirm(); onDismiss() }, kind = KeyKind.Danger)
            }
        }
    }
}
