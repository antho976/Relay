package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.Duration
import java.time.Instant

/** What a refused act says in a toast: plainly when the PC is away. */
internal fun refusal(e: BusException): String =
    if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code }

/** An act that needs the PC now, its refusal handed to [failed] (a toast unless it says otherwise). */
internal suspend fun Nav.attempt(op: String, payload: JsonObject, failed: (BusException) -> Boolean = { false }): JsonElement? =
    try {
        relay.call(op, payload)
    } catch (e: BusException) {
        if (!failed(e)) shell.toast(refusal(e))
        null
    }

/** The card frame: slab, an edge, 14dp corners; dimmed once what it shows was undone. */
@Composable
private fun CardFrame(content: @Composable RowScope.() -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(start = 12.dp, end = 6.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) { content() }
}

/**
 * Undo on a card for what the agent added: [ops].first removes it, then the card dims, says so,
 * and offers [ops].second to bring it back. Something already removed reads as removed.
 */
@Composable
private fun UndoKey(id: Long, ops: Pair<String, String>, undone: Boolean, onUndone: (Boolean) -> Unit) {
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    Key(
        if (undone) "Restore" else "Undo",
        onClick = {
            busy = true
            scope.launch {
                val restoring = undone
                val payload = buildJsonObject { put("id", id) }
                val done = nav.attempt(if (restoring) ops.second else ops.first, payload) { e ->
                    if (!restoring && e.error.code == "money.not_found") { onUndone(true); true } else false
                }
                if (done != null) onUndone(!restoring)
                busy = false
            }
        },
        kind = KeyKind.Quiet,
        enabled = !busy,
        compact = true,
    )
}

internal fun txAmount(tx: JsonObject, money: Money): String {
    val amount = money.format(tx.minor("amount", money.digits) ?: 0L)
    return when (tx.s("type")) {
        "EXPENSE" -> "−$amount"
        "INCOME" -> "+$amount"
        else -> amount
    }
}

/** What the agent did to an entry ([op]), with Undo when it added it (threads_view.rs `entry_card`). */
@Composable
internal fun EntryCard(tx: JsonObject, op: String, money: Money) {
    val c = Relay.colors
    val id = tx.l("id")
    var undone by rememberSaveable(id) { mutableStateOf(false) }
    val transfer = tx.s("type") == "TRANSFER"
    val kind = when (tx.s("type")) {
        "INCOME" -> "income"
        "TRANSFER" -> "transfer"
        else -> "expense"
    }
    val what = if (transfer) "${tx.str("account")} → ${tx.str("to_account")}"
    else tx.s("note")?.takeIf { it.isNotBlank() } ?: tx.s("category") ?: kind
    val verb = when (op) {
        "money.tx.add" -> "Added"
        "money.tx.restore" -> "Restored"
        else -> "Changed"
    }
    val detail = listOfNotNull(
        tx.s("category")?.takeIf { !transfer },
        tx.s("date")?.let(::humanDate),
        tx.s("account")?.takeIf { !transfer },
    ).joinToString(" · ")
    CardFrame {
        Row(Modifier.weight(1f).alpha(if (undone) .6f else 1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (transfer) HueBadge("", c.ink3, 30.dp, glyph = "refresh") else HueBadge(tx.s("category") ?: what, hue(tx.l("color")), 30.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                T(if (undone) "Removed: $what" else "$verb $kind: $what", style = Relay.type.ui.copy(fontSize = 13.5.sp), color = c.ink, maxLines = 1, weight = FontWeight.SemiBold)
                if (detail.isNotEmpty()) T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
            T(txAmount(tx, money), style = Relay.type.mono.copy(fontSize = 13.sp, fontFeatureSettings = "tnum"), color = if (kind == "income") c.live else c.ink)
        }
        if (op == "money.tx.add" && id != null) UndoKey(id, "money.tx.delete" to "money.tx.restore", undone) { undone = it }
    }
}

private fun activityWord(kind: String) = when (kind) {
    "DEPOSIT" -> "a deposit"
    "TRANSFER_IN" -> "a transfer in"
    "BUY" -> "a buy"
    "REINVEST" -> "a reinvested dividend"
    "SPLIT" -> "a split"
    "DIVIDEND" -> "a dividend"
    "INTEREST" -> "interest"
    "CREDIT" -> "a credit"
    "NOTIONAL_DISTRIBUTION" -> "a notional distribution"
    "RETURN_OF_CAPITAL" -> "a return of capital"
    "SELL" -> "a sale"
    "TRANSFER_OUT" -> "a transfer out"
    "FEE" -> "a fee"
    "TAX" -> "tax withheld"
    "FX" -> "a currency exchange"
    "WITHDRAWAL" -> "a withdrawal"
    else -> "an activity"
}

/** Units held, at 1e-8 a unit, as written: "10", "0.5", "41.2". */
internal fun units(quantity: Long): String {
    val whole = quantity / 100_000_000
    val part = kotlin.math.abs(quantity % 100_000_000)
    return if (part == 0L) whole.toString() else "$whole.${part.toString().padStart(8, '0').trimEnd('0')}"
}

/** An investment activity the agent recorded (`money.invest.add`), with Undo. */
@Composable
internal fun ActivityCard(a: JsonObject) {
    val c = Relay.colors
    val id = a.l("id")
    var undone by rememberSaveable(id) { mutableStateOf(false) }
    val kind = a.str("type")
    val sign = when (kind) {
        "DEPOSIT", "TRANSFER_IN", "DIVIDEND", "INTEREST", "CREDIT", "RETURN_OF_CAPITAL", "SELL" -> "+"
        "WITHDRAWAL", "TRANSFER_OUT", "FEE", "TAX", "BUY" -> "−"
        else -> ""
    }
    val account = a.str("account")
    val quantity = a.minor("quantity", 8) ?: 0L
    val symbol = a.s("symbol")?.takeIf { it.isNotBlank() }
    val subject = when {
        symbol != null && quantity > 0 -> "${units(quantity)} $symbol"
        symbol != null -> symbol
        else -> account
    }
    val money = Money(a.s("currency") ?: "CAD", Money.digitsOf(a.s("currency") ?: "CAD"))
    val detail = listOfNotNull(
        account.takeIf { it.isNotEmpty() && symbol != null },
        a.s("date")?.let(::humanDate),
        (a.minor("fee", money.digits) ?: 0L).takeIf { it > 0 }?.let { "${money.format(it)} fee" },
        a.s("note")?.trim()?.takeIf { it.isNotEmpty() },
    ).joinToString(" · ")
    val amount = money.format(a.minor("amount", money.digits) ?: 0L)
    val toCurrency = a.s("to_currency")
    val toMoney = toCurrency?.let { Money(it, Money.digitsOf(it)) }
    val toAmount = toMoney?.let { a.minor("to_amount", it.digits) }
    val shown = if (kind == "FX" && toMoney != null && toAmount != null) {
        "$amount → ${toMoney.format(toAmount)}"
    } else "$sign$amount"
    CardFrame {
        Row(Modifier.weight(1f).alpha(if (undone) .6f else 1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            HueBadge(symbol ?: account, c.ink2, 30.dp, glyph = if (sign == "+") "arrow-down" else if (sign == "−") "arrow-up" else "refresh")
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                T(if (undone) "Removed: ${activityWord(kind)}, $subject" else "Recorded ${activityWord(kind)}: $subject", style = Relay.type.ui.copy(fontSize = 13.5.sp), color = c.ink, maxLines = 1, weight = FontWeight.SemiBold)
                if (detail.isNotEmpty()) T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 1)
            }
            T(shown, style = Relay.type.mono.copy(fontSize = 13.sp, fontFeatureSettings = "tnum"), color = if (kind == "DIVIDEND" || kind == "INTEREST") c.live else c.ink)
        }
        if (id != null) UndoKey(id, "money.invest.delete" to "money.invest.restore", undone) { undone = it }
    }
}

// ---- Arbiter ----

/** `ETH-CAD` as ("ETH", "CAD"). */
internal fun pair(product: String): Pair<String, String> = product.substringBefore('-') to product.substringAfter('-', "")

/** "$1,234.56" in a currency Tally knows; "0.0123 BTC" in any other. */
internal fun coin(value: Double, currency: String): String =
    if (Money.known(currency)) Money(currency, Money.digitsOf(currency)).major(value)
    else {
        val places = if (kotlin.math.abs(value) >= 1000) 2 else if (kotlin.math.abs(value) >= 1) 4 else 8
        "${if (value < 0) "−" else ""}${trimmed(kotlin.math.abs(value), places)} $currency"
    }

/** [coin] with a plus on a gain. */
internal fun coinSigned(value: Double, currency: String): String {
    val shown = coin(value, currency)
    return if (value > 0 && shown.any { it.isDigit() && it != '0' }) "+$shown" else shown
}

/** Green for a gain, red for a loss, ink for nothing. */
@Composable
internal fun gainColor(v: Double): Color = when {
    v >= .005 -> Relay.colors.live
    v <= -.005 -> Relay.colors.heldText
    else -> Relay.colors.ink2
}

/** A price in its quote currency, with the decimals a price under 1 needs. */
internal fun priceText(p: Double, quote: String) = if (p >= 1) coin(p, quote) else "${trimmed(p, 6)} $quote"

internal fun orderAmount(o: JsonObject): String {
    val (base, quote) = pair(o.str("product"))
    val q = o.d("quote_size")
    val b = o.d("base_size")
    return when {
        q != null -> "${coin(q, quote)} of $base"
        b != null -> "${trimmed(b, 8)} $base"
        else -> "all the $base"
    }
}

/** "Bought 0.0123 ETH at $4,123.00", or "Buy $50.00 of ETH" while it is not filled. */
internal fun orderTitle(o: JsonObject): String {
    val (base, quote) = pair(o.str("product"))
    val buy = o.s("side") == "buy"
    val filled = o.d("filled_base") ?: 0.0
    if (o.s("status") == "filled" && filled > 0) {
        val at = o.d("average_price")?.let { " at ${priceText(it, quote)}" }.orEmpty()
        return "${if (buy) "Bought" else "Sold"} ${trimmed(filled, 8)} $base$at"
    }
    return "${if (buy) "Buy" else "Sell"} ${orderAmount(o)}"
}

internal fun statusWord(status: String) = when (status) {
    "pending" -> "Sending"
    "open" -> "Open"
    "filled" -> "Filled"
    "cancelled" -> "Cancelled"
    "expired" -> "Expired"
    "failed" -> "Failed"
    else -> "Unknown"
}

/** An order as one line: what it does, who asked and where, its status. */
@Composable
internal fun OrderLine(o: JsonObject, divider: Boolean = true) {
    val c = Relay.colors
    val source = when (o.s("source")) {
        "rule" -> "its rule"
        "agent" -> "an agent"
        else -> "you"
    }
    val detail = listOfNotNull(
        o.s("strategy") ?: "By hand",
        "by $source",
        if (o.s("venue") == "live") "Live" else "Paper",
        agoOf(o.s("created_at")).takeIf { it.isNotEmpty() },
        o.s("error")?.takeIf { it.isNotBlank() },
    ).joinToString(" · ")
    val status = o.str("status")
    FigureRow(orderTitle(o), detail, divider = divider, lead = {
        HueBadge("", if (o.s("side") == "buy") c.live else c.ink2, 24.dp, glyph = if (o.s("side") == "buy") "arrow-down" else "arrow-up")
    })
    if (status.isNotEmpty()) Row(Modifier.padding(start = 34.dp, bottom = 6.dp)) { StatePill(statusWord(status), if (status == "failed") c.held else null) }
}

/** An agent's tool call the engine refused, in the engine's words. */
@Composable
internal fun RefusalLine(message: String) =
    T("Refused: $message", Modifier.padding(start = 21.dp), Relay.type.caption.copy(fontSize = 13.sp, lineHeight = 19.sp), Relay.colors.heldText)

/** A refused tool call's text, as Relay's MCP server words it, down to the engine's sentence. */
internal fun refusalWords(raw: String): String {
    val said = runCatching { (Wire.json.parseToJsonElement(raw) as? JsonObject)?.s("error") }.getOrNull() ?: raw.trim()
    val head = said.substringBefore(": ", "")
    return if (head.isNotEmpty() && head.contains('.') && head.split(' ').size <= 2) said.substringAfter(": ") else said
}

private fun proposalDetail(p: JsonObject): String {
    val strategy = p.s("strategy") ?: "A strategy"
    return when (p.s("kind")) {
        "order" -> "$strategy · ${if (p.s("venue") == "live") "live, real money" else "on paper"} · ${if (p.b("limit") == true) "limit order" else "market order"}"
        "new_strategy" -> "New strategy · ${(p["draft"] as? JsonObject)?.s("product").orEmpty()} · starts stopped, on paper"
        else -> strategy
    }
}

private fun proposalFootnote(p: JsonObject) = when (p.s("kind")) {
    "order" -> "Approving passes it through the limits again, then places it."
    "new_strategy" -> "Approving saves it exactly as shown. It starts stopped, on paper."
    else -> "Approving runs the new version exactly as shown."
}

private fun JsonObject.b(key: String): Boolean? = (this[key] as? JsonPrimitive)?.content?.toBooleanStrictOrNull()

@Composable
private fun ProposalPill(status: String) {
    val c = Relay.colors
    when (status) {
        "pending" -> StatePill("Needs your approval", c.waiting)
        "approved" -> StatePill("Approved", c.live)
        "failed" -> StatePill("Failed", c.held)
        "expired" -> StatePill("Expired", null)
        else -> StatePill("Dismissed", null)
    }
}

/**
 * A proposal as a card: what it is, why, its changes, and Approve and Dismiss while it waits.
 * [now] is the proposal as the PC last listed it, so a card from an old message shows its answer.
 */
@Composable
internal fun ProposalCard(p: JsonObject, now: JsonObject? = null) {
    val nav = LocalNav.current
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    var answered by remember(p.l("id")) { mutableStateOf<JsonObject?>(null) }
    var busy by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<String?>(null) }
    val shown = answered ?: now ?: p
    val status = shown.str("status")
    val pending = status == "pending"
    Column(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, if (pending) c.waiting.copy(alpha = .45f) else c.edge, Radii.card).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T(shown.str("title"), Modifier.weight(1f), Relay.type.ui.copy(fontSize = 13.5.sp), c.ink, weight = FontWeight.SemiBold)
            ProposalPill(status)
        }
        T(proposalDetail(shown), style = Relay.type.caption, color = c.ink3)
        shown.s("why")?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.caption.copy(fontSize = 13.sp, lineHeight = 19.sp), color = c.ink2) }
        val changes = shown.arrStrings("changes")
        if (changes.isNotEmpty()) Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
            for (change in changes) Row {
                T("•", Modifier.padding(end = 8.dp), Relay.type.caption, c.ink3)
                T(change, Modifier.weight(1f), Relay.type.caption, c.ink2)
            }
        }
        if (pending) {
            Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                fun resolve(approve: Boolean) {
                    busy = true
                    problem = null
                    scope.launch {
                        val payload = buildJsonObject {
                            put("id", shown.l("id") ?: 0L)
                            put("approve", approve)
                            shown.s("draft_hash")?.let { put("draft_hash", it) }
                        }
                        val r = nav.attempt("arbiter.proposal.resolve", payload) { e ->
                            if (e.error.code != "link.down") { problem = e.error.message.ifBlank { e.error.code }; true } else false
                        }
                        (r as? JsonObject)?.let { answered = it }
                        busy = false
                    }
                }
                Key(if (shown.s("kind") == "order") "Approve order" else "Approve", { resolve(true) }, kind = KeyKind.Primary, enabled = !busy, compact = true)
                Key("Dismiss", { resolve(false) }, kind = KeyKind.Quiet, enabled = !busy, compact = true)
                minutesUntil(shown.s("expires_at"))?.takeIf { it >= 0 }?.let { left ->
                    T(if (left < 1) "expires in under a minute" else "expires in $left min", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                }
            }
            T(proposalFootnote(shown), style = Relay.type.caption, color = c.ink3)
            problem?.let { T(it, style = Relay.type.caption, color = c.heldText) }
        }
    }
}

private fun JsonObject.arrStrings(key: String): List<String> =
    (this[key] as? kotlinx.serialization.json.JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.content } ?: emptyList()

private fun minutesUntil(ts: String?): Long? =
    ts?.let { runCatching { Duration.between(Instant.now(), Instant.parse(it)).toMinutes() }.getOrNull() }

/** Palette's registration hues for investments (money_invest.rs `registration_hue`). */
internal fun registrationHue(r: String?): Color = Palette.HUES[when (r) {
    "TFSA" -> 2
    "RRSP" -> 4
    "FHSA" -> 9
    "NON_REGISTERED" -> 10
    "RESP" -> 3
    else -> 8
}]

internal fun registrationName(r: String?) = when (r) {
    "NON_REGISTERED" -> "Non-registered"
    "TFSA", "RRSP", "FHSA", "RESP", "LIRA", "RRIF" -> r
    "OTHER" -> "Other"
    else -> "Not set"
}
