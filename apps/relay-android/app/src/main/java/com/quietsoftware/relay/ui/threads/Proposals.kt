package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// Asking first in a thread (docs/THREADS.md, phase 4; threads_ask.rs): what the agent proposed
// (`thread.propose`) as a card in the conversation, with the act named on its key and "Not now".
//
// Everything on the card is the PC's reading of the ledger (`thread.proposal.*`): the rows are the
// real entries, budgets and accounts, drawn as Tally draws them, so a proposal to delete "a
// duplicate coffee" whose id is the rent shows the rent. The agent's words appear only as its
// quoted reason. The card's state is the proposal's, read again whenever the thread opens, so an
// answer stays answered. A row that changed or went since the agent asked says so, and the key
// waits until the person looks again ("Review again"); approving then binds to that reading (`seen`).

/** What the person read again after a proposal changed: that reading's `seen`, by proposal. */
private val REVIEWED_SEEN = mutableStateMapOf<Long, String>()

/** A proposal refused because it moved since it was read: changed, gone, or answered elsewhere. */
private val MOVED = setOf("thread.proposal_changed", "thread.proposal_gone", "thread.proposal_answered")

/** What an entry edit leaves alone when only its category moves (threads_ask.rs `act`). */
private val MOVE_KEEPS = listOf("type", "amount", "date", "account_id", "to_account_id", "note")

/**
 * [RelayData.live], also read again when an event moves one of the domains in [also]: a
 * proposal's rows are the ledger's, so its card reads them again on `money.changed` as well as on
 * `thread.proposal`.
 */
internal fun RelayData.liveAlso(op: String, payload: JsonObject, also: Set<String>): Flow<RelayData.Live> = channelFlow {
    val domains = also + op.substringBefore('.')
    var last = RelayData.Live()
    hub.cached(op, payload)?.let { last = RelayData.Live(it.result, it.at, fresh = false); send(last) }
    suspend fun read() {
        last = try {
            val a = hub.query(op, payload)
            RelayData.Live(a.result, a.at, fresh = a.fresh)
        } catch (e: BusException) {
            last.copy(error = e.error)
        }
        send(last)
    }
    read()
    launch { hub.syncer.stale.filter { moved -> "*" in moved || moved.any { it in domains } }.collect { read() } }
    launch { hub.state.filter { it is LinkState.Online }.distinctUntilChanged().collect { if (!last.fresh) read() } }
    awaitClose()
}.flowOn(Dispatchers.Default)

/** "1 entry", "2 entries". */
private fun counted(n: Int, one: String, many: String) = if (n == 1) "1 $one" else "$n $many"

/** The op a proposal runs, and how many it holds. */
private fun actOf(p: JsonObject): Pair<String, Int> {
    val (op, list) = p.o("action")?.entries?.firstOrNull() ?: return Pair("", 0)
    return op to ((list as? JsonArray)?.size ?: 0)
}

/** The act in words, now and done ("Delete 2 entries", "Deleted 2 entries"), and whether it removes something. */
private class AskAct(val now: String, val done: String, val destructive: Boolean)

private fun actWords(p: JsonObject): AskAct {
    val (op, n) = actOf(p)
    val items = p.objs("items")
    val entries = counted(n, "entry", "entries")
    return when (op) {
        "money.tx.delete" -> AskAct("Delete $entries", "Deleted $entries", true)
        "money.invest.delete" -> counted(n, "activity", "activities").let { AskAct("Delete $it", "Deleted $it", true) }
        "money.tx.update" -> {
            // Only the category moves: a move, as the person would call it.
            val moves = items.all { i ->
                val was = i.o("entry")
                val now = i.o("entry_after")
                now == null || MOVE_KEEPS.all { k -> was?.get(k) == now[k] }
            }
            if (moves) AskAct("Move $entries", "Moved $entries", false) else AskAct("Change $entries", "Changed $entries", false)
        }
        "money.budget.set" -> counted(n, "budget", "budgets").let { budgets ->
            if (items.all { it["budget_after"] == null || it["budget_after"] is JsonNull }) AskAct("Remove $budgets", "Removed $budgets", true)
            else AskAct("Set $budgets", "Set $budgets", false)
        }
        "money.account.add" -> counted(n, "account", "accounts").let { AskAct("Add $it", "Added $it", false) }
        "money.account.update" -> counted(n, "account", "accounts").let { accounts ->
            if (items.all { it.o("account_after")?.b("archived") == true && it.o("account")?.b("archived") == false }) AskAct("Archive $accounts", "Archived $accounts", true)
            else AskAct("Change $accounts", "Changed $accounts", false)
        }
        else -> AskAct("Approve", "Done", false)
    }
}

@Composable
private fun AskedPill(status: String) {
    val c = Relay.colors
    when (status) {
        "pending" -> StatePill("Needs your approval", c.waiting)
        "running" -> StatePill("Applying…", c.waiting)
        "approved" -> StatePill("Done", c.live)
        "failed" -> StatePill("Failed", c.held)
        "undone" -> StatePill("Undone", null)
        else -> StatePill("Not done", null)
    }
}

/**
 * What the agent asked first, proposal [id] (threads_ask.rs `fill`): [p] is the PC's reading of
 * it from the thread's proposals, absent until [read] has it. [reread] reads them again, after an
 * answer the PC refused because the proposal moved, so the card says what moved.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AskedCard(id: Long, p: JsonObject?, read: RelayData.Live, reread: () -> Unit) {
    val c = Relay.colors
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    // The PC's answer to a key here, until its next reading of the thread's proposals.
    var answered by remember(p) { mutableStateOf<JsonObject?>(null) }
    var busy by remember(id) { mutableStateOf(false) }
    var problem by remember(id) { mutableStateOf<String?>(null) }
    val shown = answered ?: p
    val frame = Modifier.fillMaxWidth().clip(Radii.card).background(c.slab)
        .border(1.dp, if (shown?.s("status") == "pending") c.waiting.copy(alpha = .45f) else c.edge, Radii.card)
        .padding(horizontal = 14.dp, vertical = 12.dp)
    if (shown == null) {
        val error = read.error
        val words = when {
            error != null -> said(error, "Relay on the PC is older than this app: open this thread there to see what it asked, and answer it.")
            read.result != null && read.fresh -> "Relay on the PC no longer has what was asked here."
            else -> "Reading the ledger…"
        }
        Column(frame) { T(words, style = Relay.type.caption, color = c.ink3) }
        return
    }
    val status = shown.str("status")
    val pending = status == "pending"
    val act = actWords(shown)
    val op = actOf(shown).first
    val currency = shown.s("currency")?.takeIf { it.isNotBlank() } ?: "CAD"
    val money = Money(currency, Money.digitsOf(currency))
    val items = shown.objs("items")
    val seen = shown.str("seen")
    val gone = items.any { it.s("state") == "gone" }
    // Something moved since Claude asked: the key waits for a fresh look, unless this is one.
    val stale = shown.b("stale") == true && REVIEWED_SEEN[id] != seen
    val dim = if (status == "undone" || status == "declined") .6f else 1f

    // The card redraws from what the PC says came of it. A refusal says why; one because the
    // proposal moved reads the thread's proposals again.
    fun answer(what: String, payload: JsonObject) {
        busy = true
        problem = null
        scope.launch {
            val r = nav.attempt(what, payload) { e ->
                if (e.error.code != "link.down") {
                    problem = e.error.message.ifBlank { e.error.code }
                    if (e.error.code in MOVED) reread()
                    true
                } else false
            }
            (r as? JsonObject)?.let { answered = it }
            busy = false
        }
    }

    fun review() {
        busy = true
        problem = null
        scope.launch {
            val fresh = nav.attempt("thread.proposal.get", buildJsonObject { put("id", id) }) { e ->
                if (e.error.code != "link.down") { problem = e.error.message.ifBlank { e.error.code }; true } else false
            } as? JsonObject
            if (fresh != null) {
                // What the person looks at now is what they approve.
                REVIEWED_SEEN[id] = fresh.str("seen")
                answered = fresh
            }
            busy = false
        }
    }

    Column(frame, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(Modifier.alpha(dim), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T(if (status == "approved") act.done else act.now, Modifier.weight(1f), Relay.type.ui.copy(fontSize = 13.5.sp), c.ink, weight = FontWeight.SemiBold)
            AskedPill(status)
        }
        Column(Modifier.alpha(dim)) {
            items.forEachIndexed { i, item ->
                AskedRow(op, item, money, divider = i > 0)
                // Only a card that waits says what moved since the agent asked: an answered one
                // shows the ledger as it was when it was answered.
                if (pending) when (item.s("state")) {
                    "changed" -> T("Changed since Claude asked", Modifier.padding(start = 38.dp, bottom = 6.dp), Relay.type.caption, c.waiting)
                    "gone" -> T("Already gone", Modifier.padding(start = 38.dp, bottom = 6.dp), Relay.type.caption, c.heldText)
                    else -> Unit
                }
            }
        }
        shown.s("why")?.trim()?.takeIf { it.isNotEmpty() }?.let {
            T("“$it” — Claude", Modifier.alpha(dim), Relay.type.caption.copy(fontSize = 13.sp, lineHeight = 19.sp), c.ink2)
        }
        when (status) {
            "pending" -> {
                // Keys wrap rather than squeeze: with Review again there are three, and a phone is narrow.
                FlowRow(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key(
                        act.now,
                        { answer("thread.proposal.resolve", buildJsonObject { put("id", id); put("approve", true); put("seen", seen) }) },
                        kind = if (act.destructive) KeyKind.Danger else KeyKind.Primary,
                        enabled = !busy && !gone && !stale,
                    )
                    Key("Not now", { answer("thread.proposal.resolve", buildJsonObject { put("id", id); put("approve", false) }) }, kind = KeyKind.Quiet, enabled = !busy)
                    if (stale && !gone) Key("Review again", { review() }, kind = KeyKind.Quiet, glyph = "refresh", enabled = !busy)
                }
                when {
                    gone -> T("Some of this is no longer in the ledger: choose Not now, and ask again.", style = Relay.type.caption, color = c.ink3)
                    stale -> T("Something changed since Claude asked: review it again first.", style = Relay.type.caption, color = c.ink3)
                    else -> Unit
                }
            }
            "approved" -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                T(shown.s("outcome")?.ifBlank { null } ?: "Done.", Modifier.weight(1f), Relay.type.caption, c.ink3)
                if (shown.b("undoable") == true) Key("Undo", { answer("thread.proposal.undo", buildJsonObject { put("id", id) }) }, kind = KeyKind.Quiet, enabled = !busy)
            }
            "failed" -> T("Could not be done, so nothing changed: ${shown.s("outcome")?.ifBlank { null } ?: "no reason given"}", style = Relay.type.caption, color = c.heldText)
            "undone" -> T("Put back as it was.", style = Relay.type.caption, color = c.ink3)
            "running" -> T("Applying it now.", style = Relay.type.caption, color = c.ink3)
            else -> T("Not done: nothing changed.", style = Relay.type.caption, color = c.ink3)
        }
        problem?.let { T(it, style = Relay.type.caption, color = c.heldText) }
    }
}

private fun kindWord(kind: String) = when (kind) {
    "INCOME" -> "Income"
    "TRANSFER" -> "Transfer"
    else -> "Expense"
}

/** One thing a proposal acts on, as Tally draws it (threads_ask.rs `row`). */
@Composable
private fun AskedRow(op: String, item: JsonObject, money: Money, divider: Boolean) {
    val c = Relay.colors
    when (op) {
        "money.tx.delete", "money.tx.update" -> AskedEntry(item, money, divider)
        "money.invest.delete" -> AskedActivity(item.o("activity"), divider)
        "money.budget.set" -> {
            val after = item.l("budget_after")
            val was = item.l("budget")?.let(money::format) ?: "none"
            val detail = if (after != null) "$was → ${money.format(after)} a month" else "$was → removed"
            FigureRow(item.s("category") ?: "Monthly budget", detail, after?.let(money::format).orEmpty(), divider = divider, lead = { HueBadge("", c.ink3, 28.dp, glyph = "graph") })
        }
        "money.account.add", "money.account.update" -> AskedAccount(item, money, divider)
        else -> FigureRow("Something Relay cannot draw", "", divider = divider, lead = { HueBadge("", c.ink3, 28.dp, glyph = "more") })
    }
}

/** An entry: what it is, and for an edit what changes, before → after, in place of what it leaves alone. */
@Composable
private fun AskedEntry(item: JsonObject, money: Money, divider: Boolean) {
    val c = Relay.colors
    val tx = item.o("entry")
    if (tx == null) {
        FigureRow("An entry no longer in the ledger", "", divider = divider, lead = { HueBadge("", c.ink3, 28.dp, glyph = "trash") })
        return
    }
    val kind = tx.s("type") ?: "EXPENSE"
    val transfer = kind == "TRANSFER"
    val category = tx.s("category") ?: kindWord(kind)
    val note = tx.str("note").trim()
    val after = item.o("entry_after")
    val detail = ArrayList<String>()
    var figure = txAmount(tx, money)
    if (after != null) {
        val categoryAfter = after.s("category") ?: kindWord(after.s("type") ?: kind)
        detail += if (categoryAfter != category) "$category → $categoryAfter" else category
        detail += if (after.s("account") != tx.s("account")) "${tx.str("account")} → ${after.str("account")}" else tx.str("account")
        detail += if (after.s("date") != tx.s("date")) "${humanDate(tx.s("date"))} → ${humanDate(after.s("date"))}" else humanDate(tx.s("date"))
        if (after.s("note") != tx.s("note")) detail += "note “${after.str("note")}”"
        if (after.minor("amount", money.digits) != tx.minor("amount", money.digits) || after.s("type") != tx.s("type")) figure = "$figure → ${txAmount(after, money)}"
    } else {
        if (note.isNotEmpty()) detail += category
        detail += if (transfer) "${tx.str("account")} → ${tx.str("to_account")}" else tx.str("account")
        detail += humanDate(tx.s("date"))
    }
    FigureRow(
        note.ifEmpty { category },
        detail.filter { it.isNotEmpty() }.joinToString(" · "),
        figure,
        divider = divider,
        figureColor = if (kind == "INCOME") c.live else c.ink2,
        lead = { if (transfer) HueBadge("", c.ink3, 28.dp, glyph = "refresh") else HueBadge(category, hue(tx.l("color")), 28.dp) },
    )
}

@Composable
private fun AskedActivity(a: JsonObject?, divider: Boolean) {
    val c = Relay.colors
    if (a == null) {
        FigureRow("An activity no longer in the ledger", "", divider = divider, lead = { HueBadge("", c.ink3, 28.dp, glyph = "trash") })
        return
    }
    val currency = a.s("currency") ?: "CAD"
    val money = Money(currency, Money.digitsOf(currency))
    val kind = a.str("type").replace('_', ' ').lowercase()
    val symbol = a.s("symbol")?.takeIf { it.isNotBlank() }
    val detail = listOf(a.str("account"), humanDate(a.s("date"))).filter { it.isNotEmpty() }.joinToString(" · ")
    FigureRow(symbol?.let { "$it, $kind" } ?: kind, detail, money.format(a.minor("amount", money.digits) ?: 0L), divider = divider, lead = { HueBadge(symbol ?: a.str("account"), c.ink2, 28.dp) })
}

/** An account to add (what it will be), or one to rename or archive (what changes). */
@Composable
private fun AskedAccount(item: JsonObject, money: Money, divider: Boolean) {
    val c = Relay.colors
    val was = item.o("account")
    val now = item.o("account_after")
    if (was == null) {
        val detail = listOfNotNull(
            now?.str("type")?.lowercase()?.takeIf { it.isNotEmpty() },
            now?.s("registration"),
            now?.s("institution")?.takeIf { it.isNotBlank() }?.let { "at $it" },
            "opening ${money.format(now?.l("balance") ?: 0L)}",
        ).joinToString(" · ")
        FigureRow(now?.s("name") ?: "A new account", detail, "New", divider = divider, lead = { HueBadge(now?.s("name").orEmpty(), c.ink2, 28.dp) })
        return
    }
    val name = was.str("name")
    val renamed = now?.s("name")?.takeIf { it != name }
    val detail = listOfNotNull(
        was.str("type").lowercase().takeIf { it.isNotEmpty() },
        now?.takeIf { it["archived"] != was["archived"] }?.let { if (it.b("archived") == true) "archive it" else "bring it back" },
        now?.takeIf { it["registration"] != was["registration"] }?.let { "registration ${it.s("registration") ?: "none"}" },
        now?.takeIf { it["institution"] != was["institution"] }?.let { "held at ${it.str("institution")}" },
    ).joinToString(" · ")
    FigureRow(renamed?.let { "$name → $it" } ?: name, detail, money.format(was.l("balance") ?: 0L), divider = divider, lead = { HueBadge(name, c.ink2, 28.dp) })
}
