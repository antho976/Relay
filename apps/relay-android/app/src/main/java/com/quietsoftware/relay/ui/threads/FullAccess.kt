package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.reveal
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// Full access (docs/THREADS.md, "Full access"): a thread whose agent is the assistant, Claude Code
// with its own tools and all of Relay, asks the person before each thing it does that changes
// anything. Each ask waits on a card above the message box until the person answers it.

/** A thread's access (`data` or `full`) and how many asks wait in it (`thread.list`'s `asking`). */
internal class Standing(val access: String, val asking: Int)

/**
 * Each thread's [Standing], by id, from `thread.list`: the phone's copy of a thread does not keep
 * them. Read again on every `thread.*` event, so an ask made or answered moves it.
 */
@Composable
internal fun rememberStandings(): Map<String, Standing> {
    val nav = LocalNav.current
    val live by remember { nav.relay.live("thread.list") }.collectAsStateWithLifecycle(RelayData.Live())
    return remember(live.result) {
        (live.result as? JsonObject)?.objs("threads")?.mapNotNull { t ->
            t.l("id")?.let { id -> id.toString() to Standing(t.s("access") ?: "data", (t.l("asking") ?: 0L).toInt()) }
        }?.toMap().orEmpty()
    }
}

/** The threads whose agent waits for the person's answer, by id: their sidebar rows say so in amber. */
@Composable
internal fun waitingThreads(): Set<String> = rememberStandings().filterValues { it.asking > 0 }.keys

/** How much of a file's text a card shows, before and after an edit or at a write's start (threads_access.rs). */
private const val SHOWN_LINES = 6
private const val SHOWN_CHARS = 600

/** How many of a multiple edit's changes a card shows. */
private const val SHOWN_EDITS = 2

/** The key that allows a tool for the rest of the thread, saying what it allows (threads_access.rs `for_thread`). */
private fun allowInThread(tool: String, op: String?) = when {
    op != null -> "Allow $op in this thread"
    tool == "Bash" -> "Allow commands in this thread"
    else -> "Allow for this thread"
}

/**
 * The text an edit puts in place of the old, or a write puts in its file: (caption, text) each,
 * and how many more changes a multiple edit makes than those shown (threads_access.rs `texts`).
 */
private fun askTexts(tool: String, input: JsonObject?): Pair<List<Pair<String, String>>, Int> = when (tool) {
    "Edit" -> listOf("Before" to input?.str("old_string").orEmpty(), "After" to input?.str("new_string").orEmpty()) to 0
    "MultiEdit" -> {
        val edits = input?.objs("edits").orEmpty()
        edits.take(SHOWN_EDITS).flatMap { listOf("Before" to it.str("old_string"), "After" to it.str("new_string")) } to maxOf(edits.size - SHOWN_EDITS, 0)
    }
    "Write" -> listOf("" to input?.str("content").orEmpty()) to 0
    else -> emptyList<Pair<String, String>>() to 0
}

/** [text] as a card shows a file's: its first lines and characters, and how many lines were left out. */
private fun cutText(text: String): String {
    val lines = text.lines()
    var out = lines.take(SHOWN_LINES).joinToString("\n")
    if (out.length > SHOWN_CHARS) out = out.take(SHOWN_CHARS) + "…"
    if (lines.size > SHOWN_LINES) {
        val more = lines.size - SHOWN_LINES
        out += "\n… $more more ${if (more == 1) "line" else "lines"}"
    }
    return out
}

/**
 * What a full-access agent asks to do ([ask], a `thread.ask.list` row), above the message box
 * until it is answered (threads_access.rs `ask_card`): what, exactly what (a command, a path, an
 * address, an op's payload), why when Claude Code says, and for an edit or a write its text; any
 * hidden character spelled out, so what is read is what is allowed. [more] asks wait after this
 * one. The keys wake half a second after the card shows, as a held action's do: it arrives
 * unannounced, under a finger aimed at the message box. Once answered the card goes
 * ([onAnswered]): what came of it shows in the thread as the tool's answer.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ToolAskCard(ask: JsonObject, more: Int, online: Boolean, modifier: Modifier = Modifier, onAnswered: (Long) -> Unit) {
    val c = Relay.colors
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val id = ask.l("id") ?: return
    var armed by remember(id) { mutableStateOf(false) }
    var busy by remember(id) { mutableStateOf(false) }
    var problem by remember(id) { mutableStateOf<String?>(null) }
    LaunchedEffect(id) {
        delay(500)
        armed = true
    }
    val tool = ask.str("tool")
    val op = ask.s("op")?.takeIf { it.isNotBlank() }
    // Agent-supplied text as it is to be allowed, each hidden character spelled out; how many there were.
    val shown = remember(id) {
        val (texts, extra) = askTexts(tool, ask.o("input"))
        val parts = listOf(ask.str("title").ifBlank { "Use a tool" }, ask.str("detail").trim(), ask.str("reason").trim()) +
            texts.map { (_, text) -> if (text.isEmpty()) "Empty" else cutText(text.replace("\t", "    ")) }
        val read = parts.map { reveal(it) }
        Triple(read.map { it.first }, texts.map { it.first }, extra) to read.sumOf { it.second }
    }
    val (words, hidden) = shown
    val (said, captions, extra) = words

    fun answer(allow: Boolean, lasting: String?) {
        busy = true
        problem = null
        scope.launch {
            val r = nav.attempt("thread.ask.answer", buildJsonObject {
                put("id", id)
                put("allow", allow)
                lasting?.let { put("scope", it) }
            }) { e ->
                when (e.error.code) {
                    // Answered on the PC meanwhile, or the agent stopped first: the card goes all the same.
                    "thread.ask_answered", "thread.ask_not_found" -> {
                        onAnswered(id)
                        true
                    }
                    "link.down" -> false
                    else -> {
                        problem = e.error.message.ifBlank { e.error.code }
                        true
                    }
                }
            }
            if (r != null) onAnswered(id)
            busy = false
        }
    }

    // A proposal's asking card: the same amber edge and pill while it waits.
    Column(
        modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.waiting.copy(alpha = .45f), Radii.card)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T(said[0], Modifier.weight(1f), Relay.type.ui.copy(fontSize = 13.5.sp), c.ink, weight = FontWeight.SemiBold)
            StatePill("Needs your approval", c.waiting)
        }
        if (hidden > 0) T("Contains hidden characters, shown as <U+…>: what it does may read differently from this.", style = Relay.type.caption, color = c.heldText)
        if (more > 0) T("${if (more == 1) "1 more waits" else "$more more wait"} after this one.", style = Relay.type.caption, color = c.ink3)
        // The whole command or path, scrolled rather than cut, so nothing allowed is out of sight.
        if (said[1].isNotEmpty()) {
            Box(
                Modifier.fillMaxWidth().clip(Radii.keycap).background(c.screen).heightIn(max = 120.dp)
                    .verticalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 8.dp),
            ) { T(said[1], style = Relay.type.code.copy(fontSize = 12.5.sp, lineHeight = 18.sp), color = c.ink2) }
        }
        if (said[2].isNotEmpty()) T(said[2], style = Relay.type.caption, color = c.ink3)
        captions.forEachIndexed { i, caption ->
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                if (caption.isNotEmpty()) T(caption, style = Relay.type.caption, color = c.ink3)
                Box(
                    Modifier.fillMaxWidth().clip(Radii.keycap).background(c.screen).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 8.dp),
                ) { T(said[3 + i], style = Relay.type.code.copy(fontSize = 12.sp, lineHeight = 17.sp), color = c.ink2) }
            }
        }
        if (extra > 0) T("And $extra more ${if (extra == 1) "change" else "changes"} in this file", style = Relay.type.caption, color = c.ink3)
        if (!online) T("Answer once the PC is in reach.", style = Relay.type.caption, color = c.ink3)
        // Keys wrap rather than squeeze: a phone is narrow, and the middle one says what it allows.
        FlowRow(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val ready = armed && online && !busy
            Key("Deny", { answer(false, null) }, kind = KeyKind.Plain, enabled = ready, compact = true)
            Key(allowInThread(tool, op), { answer(true, "thread") }, kind = KeyKind.Plain, enabled = ready, compact = true)
            Key("Allow once", { answer(true, "once") }, kind = KeyKind.Primary, enabled = ready, compact = true)
        }
        problem?.let { T(it, style = Relay.type.caption, color = c.heldText) }
    }
}
