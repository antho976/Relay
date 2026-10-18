package com.quietsoftware.relay.ui.threads

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.put
import kotlin.math.abs
import kotlin.math.roundToLong

// ---- The frame the three sources share ----

/** A query a page shows: the phone's cached answer first, then the PC's. */
@Composable
private fun rememberLive(op: String, payload: JsonObject = JsonObject(emptyMap())): RelayData.Live {
    val nav = LocalNav.current
    val live by remember(op, payload) { nav.relay.live(op, payload) }.collectAsStateWithLifecycle(RelayData.Live())
    return live
}

/** A source's page: a column of at most 720dp that scrolls, its title and context over the body. */
@Composable
private fun SourcePage(title: String, context: (@Composable () -> Unit)?, actions: @Composable RowScope.() -> Unit = {}, body: @Composable ColumnScope.() -> Unit) {
    val c = Relay.colors
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val side = gutter(maxWidth)
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
            Column(Modifier.column().fillMaxWidth().padding(horizontal = side).padding(top = 16.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        T(title, style = Relay.type.heading, color = c.ink)
                        context?.invoke()
                    }
                    actions()
                }
                body()
            }
        }
    }
}

private fun gutter(width: androidx.compose.ui.unit.Dp) = if (width < 600.dp) 16.dp else 24.dp

/** A reading's three states: its answer (with how old a saved one is), a refusal, or reading. */
@Composable
private fun Reading(live: RelayData.Live, missing: String, content: @Composable (JsonObject) -> Unit) {
    val c = Relay.colors
    val r = live.result as? JsonObject
    when {
        r != null -> {
            if (!live.fresh) StaleNote(live.at)
            content(r)
        }
        // The PC away is a state, not a failure: said quietly, as an empty page, where red read as an error.
        live.error?.code == "link.down" -> Empty("cloud", "Not on this phone yet", "The PC is out of reach. This shows here once it has been read with the PC in reach.")
        live.error != null -> T(said(live.error, missing), Modifier.padding(vertical = 12.dp), Relay.type.ui, c.heldText)
        else -> Quiet("Reading the PC…")
    }
}


/** A figure in a small card: caption, figure, a line under it. */
@Composable
private fun Stat(caption: String, figure: String, sub: String, modifier: Modifier, figureColor: Color = Relay.colors.ink, subColor: Color = Relay.colors.ink3) {
    val c = Relay.colors
    Column(modifier.clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(horizontal = 14.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        T(caption, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        T(figure, style = Relay.type.heading.copy(fontSize = 21.sp, lineHeight = 26.sp, fontFeatureSettings = "tnum"), color = figureColor, maxLines = 1)
        if (sub.isNotEmpty()) T(sub, style = Relay.type.caption, color = subColor, maxLines = 2)
    }
}

/** Figures two to a row. */
@Composable
private fun StatGrid(stats: List<@Composable (Modifier) -> Unit>) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        for (pair in stats.chunked(2)) Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            for (s in pair) s(Modifier.weight(1f).fillMaxHeight())
            if (pair.size == 1) Box(Modifier.weight(1f))
        }
    }
}

/** The large figure a page opens on: 26sp/600. */
@Composable
private fun Figure(text: String, color: Color = Relay.colors.ink) =
    T(text, style = Relay.type.heading.copy(fontSize = 26.sp, lineHeight = 32.sp, letterSpacing = (-0.02).em, fontFeatureSettings = "tnum"), color = color, maxLines = 1)

@Composable
private fun Eyebrow(text: String) = T(text, style = Relay.type.mono.copy(fontSize = 12.sp), color = Relay.colors.ink3, maxLines = 1)

// ---- Tally ----

/**
 * Tally's overview as the PC's panel draws it (threads_view.rs `panel_overview`), read only from
 * the PC's ledger: what is left with its pace, the budgets, the bills coming up and recent
 * entries; then Entries, Budgets and Invest.
 */
@Composable
fun TallyScreen(nav: Nav) {
    SpaceFrame(nav) {
        var tab by rememberSaveable { mutableStateOf("overview") }
        val c = Relay.colors
        val ledger by remember { nav.relay.cached("money.summary") }.collectAsStateWithLifecycle(null)
        SourcePage("Tally", { T("The PC's ledger, read on this phone", style = Relay.type.caption, color = c.ink3) }) {
            Segmented(listOf("overview" to "Overview", "entries" to "Entries", "budgets" to "Budgets", "invest" to "Invest"), tab, { tab = it }, Modifier.fillMaxWidth(), fill = true)
            val missing = "This engine does not read Tally yet. Update Relay's engine to see it here."
            when (tab) {
                "entries" -> Reading(rememberLive("money.tx.list", remember { buildJsonObject { put("limit", 100) } }), missing) { TallyEntries(it, Money.of(ledger?.result as? JsonObject)) }
                "budgets" -> Reading(rememberLive("money.summary"), missing) { s -> Budgets(s, Int.MAX_VALUE) }
                "invest" -> {
                    val lists = rememberLive("money.lists")
                    Reading(rememberLive("money.invest.summary"), "This engine does not read investments yet. Update Relay's engine to see them here.") { Invest(it, lists.result as? JsonObject) }
                }
                else -> Reading(rememberLive("money.summary"), missing) { TallyOverview(it) }
            }
            T(
                "Tally on this phone is its own app; this is the PC's copy of the same ledger.",
                Modifier.fillMaxWidth().padding(top = 8.dp),
                Relay.type.caption.copy(textAlign = TextAlign.Center),
                c.ink3,
            )
        }
    }
}

@Composable
private fun statusTone(status: String?): Color? = when (status) {
    "OVER_BUDGET" -> Relay.colors.heldText
    "OVER_PACE" -> Relay.colors.waiting
    else -> null
}

@Composable
private fun TallyOverview(s: JsonObject) {
    val c = Relay.colors
    if (s.b("empty") == true) {
        Quiet("Tally has nothing yet. Add an account, load the sample household or import a backup in Tally on the PC.")
        return
    }
    val money = Money.of(s)
    val period = s.o("period")
    val days = period?.l("days") ?: 0L
    val day = (days - (period?.l("days_left") ?: 0L) + 1).coerceIn(1L, maxOf(days, 1L))
    val pace = s.o("pace")
    val status = pace?.s("status")
    val budget = pace?.l("budget") ?: 0L
    val spent = s.l("spent") ?: 0L
    val income = s.l("income") ?: 0L
    Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
        Eyebrow("${periodName(period)} · day $day of $days")
        if (pace != null && budget > 0) {
            val left = pace.l("remaining") ?: 0L
            Figure(if (left < 0) "${money.whole(-left)} over" else "${money.whole(left)} left", if (left < 0) c.heldText else c.ink)
            val daily = pace.l("daily_allowance") ?: 0L
            T(if (daily > 0) "of ${money.whole(budget)} · ${money.whole(daily)} a day from here" else "of ${money.whole(budget)} planned this month", style = Relay.type.ui, color = c.ink3)
            PaceMeter(
                (pace.d("spent_fraction") ?: 0.0).toFloat(),
                statusTone(status) ?: c.ink,
                Modifier.padding(top = 6.dp),
                tick = pace.d("pace_fraction")?.toFloat(),
                height = 6.dp,
            )
            s.o("lines")?.s("pace")?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.caption, color = statusTone(status) ?: c.ink3) }
            T("Spent ${money.whole(spent)} · In ${money.whole(income)}", style = Relay.type.mono.copy(fontSize = 12.sp), color = c.ink2)
        } else {
            Figure("${money.whole(spent)} spent")
            T("In ${money.whole(income)} · no monthly budget yet: set one in Tally's Plan.", style = Relay.type.caption, color = c.ink3)
        }
    }
    Budgets(s, 5)
    val bills = s.objs("bills").take(3)
    if (bills.isNotEmpty()) Card("Coming up") {
        bills.forEachIndexed { i, bill ->
            val inflow = bill.s("type") == "INCOME"
            val amount = bill.l("amount") ?: 0L
            val soon = !inflow && (bill.l("days_until") ?: 99L) <= 3
            FigureRow(
                bill.str("name"), bill.str("due_line"),
                if (inflow) money.plus(amount) else "−${money.format(amount)}",
                figureColor = if (inflow) c.live else c.ink2,
                detailColor = if (soon) c.waiting else c.ink3,
                divider = i > 0,
                lead = { HueBadge("", c.ink3, 24.dp, glyph = if (inflow) "arrow-down" else "history") },
            )
        }
    }
    val recent = s.objs("recent").take(5)
    Card("Recent") {
        if (recent.isEmpty()) Quiet("Nothing logged this period yet.")
        recent.forEachIndexed { i, tx -> EntryRow(tx, money, dated = true, divider = i > 0) }
    }
}

/** The budgets furthest along first, each with its hue badge and a meter against the period's pace. */
@Composable
private fun Budgets(s: JsonObject, most: Int) {
    val c = Relay.colors
    val money = Money.of(s)
    val tick = s.o("pace")?.d("pace_fraction")?.toFloat()
    val budgets = s.objs("budgets").filter { (it.l("budget") ?: 0L) > 0 }
        .sortedByDescending { (it.l("spent") ?: 0L).toDouble() / maxOf(it.l("budget") ?: 1L, 1L) }
    Card("Budgets", aside = if (budgets.size > most) "$most of ${budgets.size}" else null) {
        if (budgets.isEmpty()) Quiet("No budgets yet. Set them in Tally's Plan.")
        budgets.take(most).forEachIndexed { i, b ->
            val spent = b.l("spent") ?: 0L
            val budget = b.l("budget") ?: 0L
            val left = budget - spent
            val tone = statusTone(b.s("status"))
            if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
            Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                HueBadge(b.str("name"), hue(b.l("color")), 24.dp)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        T(b.str("name"), Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                        T(if (left < 0) "${money.whole(-left)} over" else "${money.whole(left)} left", style = Relay.type.mono.copy(fontSize = 12.sp), color = tone ?: c.ink2)
                    }
                    PaceMeter(spent.toFloat() / maxOf(budget, 1L), tone ?: hue(b.l("color")), tick = tick)
                }
            }
        }
    }
}

/** One entry: its badge, what it was over its category and day, and the amount. */
@Composable
private fun EntryRow(tx: JsonObject, money: Money, dated: Boolean, divider: Boolean) {
    val c = Relay.colors
    val transfer = tx.s("type") == "TRANSFER"
    val category = tx.s("category") ?: if (transfer) "Transfer" else "Entry"
    val note = tx.s("note")?.trim()?.takeIf { it.isNotEmpty() }
    val detail = buildList {
        if (note != null) add(category)
        if (transfer) add("${tx.str("account")} → ${tx.str("to_account")}")
        if (dated) add(humanDate(tx.s("date")))
        if (isEmpty()) add(tx.str("account"))
    }.joinToString(" · ")
    FigureRow(
        note ?: category, detail, txAmount(tx, money),
        figureColor = if (tx.s("type") == "INCOME") c.live else c.ink2,
        divider = divider,
        lead = { if (transfer) HueBadge("", c.ink3, 24.dp, glyph = "refresh") else HueBadge(category, hue(tx.l("color")), 24.dp) },
    )
}

/** This period's entries, by day with each day's net. */
@Composable
private fun TallyEntries(page: JsonObject, money: Money) {
    val c = Relay.colors
    val rows = page.objs("transactions")
    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Eyebrow(periodName(page.o("period")))
        T("Out ${money.whole(page.l("spent") ?: 0L)} · In ${money.whole(page.l("income") ?: 0L)}", style = Relay.type.mono.copy(fontSize = 12.sp), color = c.ink2)
    }
    if (rows.isEmpty()) {
        Quiet("No entries this period.")
        return
    }
    for ((day, list) in rows.groupBy { it.str("date") }) {
        val net = list.sumOf {
            when (it.s("type")) {
                "INCOME" -> it.l("amount") ?: 0L
                "EXPENSE" -> -(it.l("amount") ?: 0L)
                else -> 0L
            }
        }
        Card(humanDate(day), aside = money.plus(net)) {
            list.forEachIndexed { i, tx -> EntryRow(tx, money, dated = false, divider = i > 0) }
        }
    }
}

/**
 * The Invest view (threads_view.rs `panel_invest`): the portfolio's value and gain, how it is
 * split, the accounts, the room left this year and the largest holdings. An account no file has
 * filled is worth its balance in [lists], as the PC counts it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Invest(p: JsonObject, lists: JsonObject?) {
    val c = Relay.colors
    if (p.b("empty") == true) {
        Card("Investments") {
            Quiet("See your TFSA, RRSP and FHSA together: what they are worth, what they have earned, and the room left this year. Bring them in from Tally on the PC, or ask a thread to help connect Wealthsimple.")
        }
        return
    }
    val money = Money.of(p)
    val balances = lists?.objs("accounts").orEmpty()
    fun byHand(a: JsonObject): Long? =
        if ((a.l("holdings") ?: 0L) == 0L && (a.l("book") ?: 0L) == 0L && (a.l("value") ?: 0L) == 0L && (a.l("cash") ?: 0L) == 0L)
            balances.firstOrNull { it.l("id") == a.l("id") }?.l("balance") else null
    val accounts = p.objs("accounts")
    val hand = accounts.sumOf { byHand(it) ?: 0L }
    val gain = p.l("gain") ?: 0L
    fun gainColor(n: Long) = when {
        n > 0 -> c.live
        n < 0 -> c.heldText
        else -> c.ink3
    }
    Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
        Eyebrow(p.s("as_of")?.takeIf { it.isNotBlank() }?.let { "As of ${humanDate(it.take(10))}" } ?: "No report imported yet")
        Figure(money.whole((p.l("value") ?: 0L) + hand))
        T("${if (gain > 0) "+" else ""}${money.whole(gain)} gain${p.l("gain_bps")?.let { " · ${bps(it)} on book value" }.orEmpty()}", style = Relay.type.caption, color = gainColor(gain))
        val parts = p.objs("allocation").filter { (it.l("value") ?: 0L) > 0 }
        if (parts.isNotEmpty()) {
            Canvas(Modifier.fillMaxWidth().padding(top = 6.dp).height(8.dp).clip(Radii.pill)) {
                var x = 0f
                for (a in parts) {
                    val w = size.width * ((a.l("share_bps") ?: 0L) / 10_000f)
                    drawRect(registrationHue(a.s("registration")), Offset(x, 0f), Size(w, size.height))
                    x += w
                }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                for (a in parts) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Dot(registrationHue(a.s("registration")), 7.dp)
                    T("${registrationName(a.s("registration"))} ${Math.round((a.l("share_bps") ?: 0L) / 100.0)}%", style = Relay.type.caption, color = c.ink2)
                }
            }
        }
    }
    if (accounts.isNotEmpty()) Card("Accounts") {
        accounts.forEachIndexed { i, a ->
            val detail = listOfNotNull(
                registrationName(a.s("registration")),
                a.s("institution")?.takeIf { it.isNotBlank() },
                (a.l("holdings") ?: 0L).takeIf { it > 0 }?.let { plural(it, "holding") },
                byHand(a)?.let { "value recorded by hand" },
            ).joinToString(" · ")
            InvestRow(a.str("name"), detail, money.whole(byHand(a) ?: a.l("value") ?: 0L), a.l("gain_bps"), divider = i > 0) {
                HueBadge(a.str("name"), registrationHue(a.s("registration")), 24.dp)
            }
        }
    }
    val room = p.objs("room")
    room.firstOrNull()?.l("year")?.let { year ->
        Card("Room left in $year") {
            room.forEachIndexed { i, r ->
                val over = r.l("over") ?: 0L
                val total = r.l("room")
                val text = when {
                    over > 0 -> "Over by ${money.whole(over)}"
                    total != null && r.l("left") != null -> "${money.whole(r.l("left")!!)} left of ${money.whole(total)}"
                    else -> "Room not set"
                }
                if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
                Column(Modifier.padding(vertical = 9.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row {
                        T(registrationName(r.s("registration")), Modifier.weight(1f), Relay.type.ui, c.ink)
                        T(text, style = Relay.type.mono.copy(fontSize = 12.sp), color = if (over > 0) c.heldText else c.ink2)
                    }
                    if (total != null && total > 0) PaceMeter((r.l("contributed") ?: 0L).toFloat() / total, if (over > 0) c.heldText else registrationHue(r.s("registration")))
                    else T("Add it on the Investments page on the PC, from CRA My Account.", style = Relay.type.caption, color = c.ink3)
                }
            }
        }
    }
    val holdings = p.objs("holdings")
    if (holdings.isNotEmpty()) Card("Top holdings", aside = if (holdings.size > 5) "5 of ${holdings.size}" else null) {
        holdings.take(5).forEachIndexed { i, h ->
            val symbol = h.str("symbol")
            val detail = listOf(h.str("account"), if (h.b("no_price") == true) "no price yet" else "${Math.round((h.l("weight_bps") ?: 0L) / 100.0)}% of the total").joinToString(" · ")
            InvestRow(h.s("name")?.takeIf { it.isNotBlank() } ?: symbol, detail, money.whole(h.l("value") ?: 0L), h.l("gain_bps"), divider = i > 0) {
                Box(Modifier.width(42.dp).height(24.dp).clip(Radii.keycap).background(c.wash), contentAlignment = Alignment.Center) {
                    T(symbol.take(5), style = Relay.type.mono.copy(fontSize = 10.5.sp), color = c.ink2, maxLines = 1)
                }
            }
        }
    }
    val income = p.l("income_12m") ?: 0L
    if (income > 0) Card("Income", aside = "12 months") {
        FigureRow("Dividends and interest", "", money.plus(income), figureColor = c.live, divider = false)
    }
    val issues = p.arr("issues").mapNotNull { (it as? JsonPrimitive)?.content }
    if (issues.isNotEmpty()) Card("To check") {
        for (issue in issues.take(3)) T(issue, Modifier.padding(vertical = 5.dp), Relay.type.caption, c.ink2)
    }
}

@Composable
private fun InvestRow(title: String, detail: String, value: String, gainBps: Long?, divider: Boolean, lead: @Composable () -> Unit) {
    val c = Relay.colors
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Row(Modifier.fillMaxWidth().heightIn(min = 44.dp).padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        lead()
        Column(Modifier.weight(1f)) {
            T(title, style = Relay.type.ui, color = c.ink, maxLines = 1)
            T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
        Column(horizontalAlignment = Alignment.End) {
            T(value, style = Relay.type.mono.copy(fontSize = 12.5.sp), color = c.ink)
            gainBps?.let { T(bps(it), style = Relay.type.mono.copy(fontSize = 12.sp), color = if (it > 0) c.live else if (it < 0) c.heldText else c.ink3) }
        }
    }
}

// ---- Arbiter ----

private val MODES = mapOf("rules" to "Rules run, AI proposes", "agent" to "AI trades within limits", "ask" to "Every order asks")

/**
 * Arbiter, the trading tool, read from the PC (arbiter_pages.rs, Overview): the connection, the
 * portfolio, each strategy with its mode, what waits for the person (Approve or Dismiss), open
 * orders, today's limits and fills; and Halt all, after a yes.
 */
@Composable
fun ArbiterScreen(nav: Nav) {
    SpaceFrame(nav) {
        val live = rememberLive("arbiter.summary")
        val s = live.result as? JsonObject
        var halting by remember { mutableStateOf(false) }
        var tab by rememberSaveable { mutableStateOf("overview") }
        val missing = "This engine has no Arbiter yet. Update Relay's engine to use it."
        SourcePage(
            "Arbiter",
            context = s?.o("connection")?.let { { Connection(it) } },
            actions = { Key("Halt all", { halting = true }, kind = KeyKind.Danger, enabled = s != null && s.b("halted") != true, compact = true) },
        ) {
            Segmented(listOf("overview" to "Overview", "orders" to "Orders"), tab, { tab = it }, Modifier.fillMaxWidth(), fill = true)
            if (tab == "orders") {
                val limit = remember { buildJsonObject { put("limit", 50) } }
                Reading(rememberLive("arbiter.order.list", limit), missing) { o ->
                    val orders = o.objs("orders")
                    Card("Orders") {
                        if (orders.isEmpty()) Quiet("No orders yet.")
                        orders.forEachIndexed { i, order -> OrderLine(order, divider = i > 0) }
                    }
                }
                Reading(rememberLive("arbiter.decision.list", limit), missing) { d ->
                    val decisions = d.objs("decisions")
                    Card("Decisions") {
                        if (decisions.isEmpty()) Quiet("Nothing decided yet.")
                        decisions.forEachIndexed { i, decision -> DecisionRow(decision, divider = i > 0) }
                    }
                }
            } else {
                Reading(live, missing) { ArbiterOverview(it, nav) }
            }
        }
        if (halting) ConfirmDialog(
            title = "Halt everything?",
            body = "This cancels every open order and stops every strategy, paper and live, until you restart. What they hold is kept: selling it is a separate step on each strategy.",
            action = "Halt all",
            onDismiss = { halting = false },
        ) {
            nav.shell.act("arbiter.halt", buildJsonObject { put("reason", "you pressed Halt all on the phone") }) { v ->
                val o = v as? JsonObject
                val cancelled = o?.l("cancelled") ?: 0L
                val failed = o?.arr("failed")?.mapNotNull { (it as? JsonPrimitive)?.content }.orEmpty()
                var words = when (cancelled) {
                    0L -> "Halted. There were no open orders."
                    1L -> "Halted. 1 open order cancelled."
                    else -> "Halted. $cancelled open orders cancelled."
                }
                if (failed.isNotEmpty()) words += " Not cancelled: ${failed.joinToString("; ")}"
                nav.shell.toast(words)
            }
        }
    }
}

@Composable
private fun Connection(conn: JsonObject) {
    val c = Relay.colors
    val exchange = conn.s("exchange")?.takeIf { it.isNotBlank() && it != "coinbase" } ?: "Coinbase"
    val (tone, words) = when (conn.s("state")) {
        "ok" -> c.live to listOfNotNull(exchange, agoOf(conn.s("checked_at")).takeIf { it.isNotEmpty() }?.let { "checked $it" }).joinToString(" · ")
        "error" -> c.held to "$exchange · ${conn.s("error") ?: "the last check failed"}"
        else -> null to "Paper only · no Coinbase key yet"
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
        if (tone != null) Dot(tone, 7.dp)
        T(words, style = Relay.type.caption, color = if (tone == c.held) c.heldText else c.ink3, maxLines = 2)
    }
}

@Composable
private fun ArbiterOverview(s: JsonObject, nav: Nav) {
    val c = Relay.colors
    val home = s.s("home")?.takeIf { it.isNotBlank() } ?: "CAD"
    if (s.b("halted") == true) {
        Column(
            Modifier.fillMaxWidth().clip(Radii.card).background(c.held.copy(alpha = .12f)).border(1.dp, c.held.copy(alpha = .45f), Radii.card).padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            T("Arbiter is halted: ${s.s("halt_reason") ?: "the kill switch is on"}. No strategy places an order until you restart.", style = Relay.type.ui, color = c.ink)
            Key("Restart", { nav.shell.act("arbiter.restart", JsonObject(emptyMap()), done = "Restarted. Strategies run again from their next bar.") }, compact = true)
        }
    }
    s.s("runner_error")?.takeIf { it.isNotBlank() }?.let { T("The last pass went wrong: $it", style = Relay.type.caption, color = c.heldText) }
    if (s.o("connection")?.s("state") == "error") {
        T("The last check with Coinbase failed. Live strategies place no orders until it passes; check again from Arbiter on the PC.", style = Relay.type.caption, color = c.heldText)
    }
    Portfolio(s, home)
    val strategies = s.objs("strategies")
    Card("Strategies") {
        if (strategies.isEmpty()) Quiet("No strategies yet. Start one from a template in Arbiter on the PC; a new strategy starts on paper.")
        strategies.forEachIndexed { i, r -> StrategyRow(r, divider = i > 0, nav) }
    }
    val pending = s.objs("pending")
    Card(if (pending.isEmpty()) "Waiting for you" else "Waiting for you · ${pending.size}", padding = androidx.compose.foundation.layout.PaddingValues(14.dp)) {
        if (pending.isEmpty()) Quiet("Nothing waits for your approval.")
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) { for (p in pending) ProposalCard(p) }
    }
    val open = s.objs("open_orders")
    if (open.isNotEmpty()) Card("Open orders") { open.forEachIndexed { i, o -> OrderLine(o, divider = i > 0) } }
    val limits = s.objs("limits")
    Card("Limits today") {
        if (limits.isEmpty()) Quiet("No limits across strategies yet. Set them in Arbiter's settings on the PC.")
        limits.forEachIndexed { i, l -> LimitRow(l, home, divider = i > 0) }
    }
    val fills = s.objs("fills")
    Card("Recent fills") {
        if (fills.isEmpty()) Quiet("No fills yet.")
        fills.take(8).forEachIndexed { i, f ->
            val (base, quote) = pair(f.str("product"))
            val buy = f.s("side") == "buy"
            val pnl = f.d("pnl")
            FigureRow(
                "${if (buy) "Bought" else "Sold"} ${trimmed(f.d("size") ?: 0.0, 8)} $base at ${priceText(f.d("price") ?: 0.0, quote)}",
                listOf(f.s("strategy") ?: "By hand", if (f.s("venue") == "live") "Live" else "Paper", agoOf(f.s("at"))).filter { it.isNotEmpty() }.joinToString(" · "),
                pnl?.let { coinSigned(it, quote) }.orEmpty(),
                figureColor = pnl?.let { gainColor(it) } ?: c.ink2,
                divider = i > 0,
            )
        }
    }
}

/** The money: live when a key reads it, else paper; today, since start, cash and fees; and its line. */
@Composable
private fun Portfolio(s: JsonObject, home: String) {
    val c = Relay.colors
    val live = s.o("live")
    val account = live ?: s.o("paper") ?: return
    Card("Portfolio") {
        Row(Modifier.padding(top = 2.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Figure(coin(account.d("value") ?: 0.0, home))
            if (live != null) StatePill("Live", c.live) else StatePill("Paper", c.waiting)
        }
        val figures = listOf(
            Triple("Today, after fees", account.d("pnl_today") ?: 0.0, true),
            Triple("Since start", account.d("pnl_total") ?: 0.0, true),
            Triple("Held as cash", account.d("cash") ?: 0.0, false),
            Triple("Fees this month", account.d("fees_month") ?: 0.0, false),
        )
        for (pair in figures.chunked(2)) Row(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
            for ((caption, value, signed) in pair) Column(Modifier.weight(1f)) {
                T(caption, style = Relay.type.caption, color = c.ink3)
                T(if (signed) coinSigned(value, home) else coin(value, home), style = Relay.type.mono.copy(fontSize = 13.sp), color = if (signed) gainColor(value) else c.ink)
            }
        }
        Sparkline(account["equity"] as? JsonArray)
        if (live != null) s.o("paper")?.let { paper ->
            T("Paper account ${coin(paper.d("value") ?: 0.0, home)} · today ${coinSigned(paper.d("pnl_today") ?: 0.0, home)}", Modifier.padding(top = 6.dp, bottom = 4.dp), Relay.type.caption, c.ink3)
        }
    }
}

/** Value over time from `[time, value]` pairs. */
@Composable
private fun Sparkline(points: JsonArray?) {
    val values = points.orEmpty().mapNotNull { p ->
        val pair = p as? JsonArray ?: return@mapNotNull null
        (pair.getOrNull(1) as? JsonPrimitive)?.let { it.doubleOrNull ?: it.content.toDoubleOrNull() }
    }
    if (values.size < 2) return
    val c = Relay.colors
    val up = values.last() >= values.first()
    val color = if (up) c.live else c.heldText
    Canvas(Modifier.fillMaxWidth().padding(vertical = 8.dp).height(44.dp)) {
        val lo = values.min()
        val hi = values.max()
        val span = (hi - lo).takeIf { it > 0 } ?: 1.0
        val path = Path()
        values.forEachIndexed { i, v ->
            val x = size.width * i / (values.size - 1)
            val y = (size.height - 2.dp.toPx()) * (1 - ((v - lo) / span)).toFloat() + 1.dp.toPx()
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(path, color, style = Stroke(1.75.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/** A strategy: its name and mode pill over who decides and what it is doing, its profit; it opens to its rule in words. */
@Composable
private fun StrategyRow(r: JsonObject, divider: Boolean, nav: Nav) {
    val c = Relay.colors
    var open by rememberSaveable(r.l("id")) { mutableStateOf(false) }
    val state = r.s("state")
    val currency = r.s("currency") ?: "CAD"
    val mode = MODES[r.s("mode")] ?: MODES.getValue("rules")
    val doing = when (state) {
        "halted" -> "Halted: ${r.s("halt_reason") ?: "a limit was reached"}"
        "stopped" -> "Stopped · $mode"
        else -> "$mode · ${r.str("doing")}"
    }
    val pnl = (r.d("pnl_realized") ?: 0.0) + (r.d("pnl_open") ?: 0.0)
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Column(Modifier.fillMaxWidth().clickable { open = !open }.padding(vertical = 9.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    T(r.str("name"), Modifier.weight(1f, fill = false), Relay.type.ui, c.ink, maxLines = 1, weight = FontWeight.Medium)
                    when {
                        state == "halted" -> StatePill("Halted", c.held)
                        r.s("venue") == "live" -> StatePill("Live", c.live)
                        else -> StatePill("Paper", c.waiting)
                    }
                }
                T(doing, style = Relay.type.caption, color = if (state == "halted") c.heldText else c.ink3, maxLines = if (open) 4 else 1)
            }
            Column(horizontalAlignment = Alignment.End) {
                T(coinSigned(pnl, currency), style = Relay.type.mono.copy(fontSize = 13.sp), color = gainColor(pnl))
                T("today ${coinSigned(r.d("pnl_today") ?: 0.0, currency)}", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
            }
        }
        AnimatedVisibility(open) {
            Column(Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                for (line in r.arr("sentences").mapNotNull { (it as? JsonPrimitive)?.content }) Row {
                    T("•", Modifier.padding(end = 8.dp), Relay.type.caption, c.ink3)
                    T(line, Modifier.weight(1f), Relay.type.caption, c.ink2)
                }
                T(listOfNotNull(r.s("product"), plural(r.l("trades") ?: 0L, "trade"), "version ${r.l("version") ?: 1}").joinToString(" · "), style = Relay.type.mono.copy(fontSize = 11.5.sp), color = c.ink3)
                if (state == "halted") Key("Restart", {
                    nav.shell.act("arbiter.restart", buildJsonObject { put("strategy_id", r.l("id") ?: 0L) }, done = "Restarted. It runs again from its next bar.")
                }, Modifier.padding(top = 4.dp), compact = true)
            }
        }
    }
}

/** One line of the decision log, its lamp saying what kind of decision it was. */
@Composable
private fun DecisionRow(d: JsonObject, divider: Boolean) {
    val c = Relay.colors
    val tone = when (d.s("kind")) {
        "refused", "halt", "error" -> c.held
        "approved", "restart" -> c.live
        "proposal" -> c.waiting
        else -> null
    }
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.padding(top = 6.dp)) { Dot(tone ?: c.ink3, 7.dp) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            T(d.str("text"), style = Relay.type.ui.copy(fontSize = 13.5.sp, lineHeight = 19.sp), color = if (tone == c.held) c.heldText else c.ink)
            T(agoOf(d.s("at")), style = Relay.type.caption, color = c.ink3)
        }
    }
}

/** A limit and how much of it today used: red when full, amber past two thirds. */
@Composable
private fun LimitRow(l: JsonObject, currency: String, divider: Boolean) {
    val c = Relay.colors
    val unit = l.s("unit")
    fun shown(x: Double) = when (unit) {
        "money" -> coin(x, currency)
        "percent" -> String.format(java.util.Locale.ROOT, "%.1f%%", x)
        else -> x.roundToLong().toString()
    }
    val used = l.d("used") ?: 0.0
    val limit = l.d("limit")
    val fraction = limit?.takeIf { it > 0 }?.let { used / it } ?: 0.0
    val tone = when {
        fraction >= 1 -> c.heldText
        fraction > .66 -> c.waiting
        else -> null
    }
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Column(Modifier.padding(vertical = 9.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Row {
            T(l.str("label"), Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
            T(if (limit != null) "${shown(used)} of ${shown(limit)}" else "${shown(used)} · no limit", style = Relay.type.mono.copy(fontSize = 12.sp), color = tone ?: c.ink2)
        }
        PaceMeter(fraction.toFloat(), tone ?: c.ink)
    }
}

// ---- Avex ----

/**
 * Avex, the gym app, as Relay read its last export (avex_pages.rs): this week, volume, the streak
 * and the last workout, then recent workouts, records, lifts and goals. Read only; the export is
 * imported on the PC.
 */
@Composable
fun AvexScreen(nav: Nav) {
    SpaceFrame(nav) {
        val c = Relay.colors
        var tab by rememberSaveable { mutableStateOf("overview") }
        val summary = rememberLive("gym.summary")
        val s = summary.result as? JsonObject
        val unit = s?.s("unit") ?: "lb"
        val missing = "This engine does not read Avex yet. Update Relay's engine to see your training here."
        val imported = s?.o("imported")
        SourcePage("Avex", context = {
            T(
                imported?.let { i -> listOfNotNull(i.s("user_name")?.takeIf { it.isNotBlank() }?.let { "$it's training" }, i.s("exported_at")?.let { "exported ${humanDate(it.take(10))}" }, agoOf(i.s("imported_at")).takeIf { it.isNotEmpty() }?.let { "imported $it" }).joinToString(" · ") }
                    ?: "Your training, read from Avex's export",
                style = Relay.type.caption, color = c.ink3,
            )
        }) {
            Segmented(listOf("overview" to "Overview", "workouts" to "Workouts", "lifts" to "Lifts"), tab, { tab = it }, Modifier.fillMaxWidth(), fill = true)
            when (tab) {
                "workouts" -> Reading(rememberLive("gym.sessions", remember { buildJsonObject { put("limit", 60) } }), missing) { l ->
                    val list = l.objs("sessions")
                    Card("Workouts", aside = l.l("total")?.let { if (it > list.size) "${list.size} of $it" else null }) {
                        if (list.isEmpty()) Quiet("No workouts in this export.")
                        list.forEachIndexed { i, w -> WorkoutRow(w, unit, divider = i > 0) }
                    }
                }
                "lifts" -> Reading(rememberLive("gym.lifts"), missing) { l ->
                    val lifts = l.objs("lifts")
                    Card("Lifts") {
                        if (lifts.isEmpty()) Quiet("No lifts with working sets yet.")
                        lifts.forEachIndexed { i, lift -> LiftRow(lift, l.s("unit") ?: unit, divider = i > 0) }
                    }
                }
                else -> Reading(summary, missing) { AvexOverview(it, unit) }
            }
        }
    }
}

private fun minutes(m: Long) = if (m >= 60) "${m / 60} h ${(m % 60).toString().padStart(2, '0')} min" else "$m min"

private fun volume(v: Double?, unit: String) = "${group((v ?: 0.0).roundToLong())} $unit"

/** `102.5`, `100`: a weight or a count without a trailing `.0`. */
private fun weight(x: Double) = if (abs(x - Math.round(x)) < .05) group(Math.round(x)) else String.format(java.util.Locale.ROOT, "%.1f", x)

/** `225 lb × 5`, `× 12` for a bodyweight set. */
private fun setWords(w: Double?, reps: Long, unit: String) = if (w != null) "${weight(w)} $unit × $reps" else "× $reps"

private fun word(key: String) = key.replace('_', ' ').lowercase().replaceFirstChar { it.uppercase() }

@Composable
private fun AvexOverview(s: JsonObject, unit: String) {
    val c = Relay.colors
    if (s.o("imported") == null) {
        Card("Bring in your training") {
            Quiet("Avex keeps everything on your phone and has no internet, so Relay reads the file it exports. In Avex: Settings, Export, Training history (JSON); send avex_export.json to the PC and import it in Avex's page there. Each import replaces the last.")
        }
        return
    }
    val week = s.o("this_week")
    val last = s.o("last_week")
    val target = s.o("imported")?.l("days_per_week") ?: 0L
    val sessions = week?.l("sessions") ?: 0L
    val vol = week?.d("volume") ?: 0.0
    val before = last?.d("volume") ?: 0.0
    val streak = s.l("streak_weeks") ?: 0L
    val since = when (val d = s.l("days_since_last")) {
        null -> "Never"
        0L -> "Today"
        1L -> "Yesterday"
        else -> "$d days ago"
    }
    StatGrid(listOf(
        { m -> Stat("This week", "$sessions${if (target > 0) " of $target" else ""}", "Last week ${last?.l("sessions") ?: 0}", m) },
        { m -> Stat("Volume", volume(vol, unit), if (vol <= 0) "No workouts yet this week" else if (before > 0) String.format(java.util.Locale.ROOT, "%+.0f%% on last week", (vol / before - 1) * 100) else "Nothing last week", m) },
        { m -> Stat("Streak", plural(maxOf(streak, 0L), "week"), "In a row with a workout", m) },
        { m -> Stat("Last workout", since, "${minutes(week?.l("active_minutes") ?: 0L)} training this week", m) },
    ))
    val weeks = s.objs("weeks")
    if (weeks.isNotEmpty()) Card("Volume by week") { WeekBars(weeks, unit) }
    val recent = s.objs("recent")
    Card("Recent workouts") {
        if (recent.isEmpty()) Quiet("No workouts in this export.")
        recent.forEachIndexed { i, w -> WorkoutRow(w, unit, divider = i > 0) }
    }
    val records = s.objs("records")
    if (records.isNotEmpty()) Card("Records") {
        records.forEachIndexed { i, r ->
            val detail = listOfNotNull(humanDate(r.s("day")), r.d("e1rm")?.let { "estimated max ${weight(it)} $unit" }).joinToString(" · ")
            FigureRow(r.str("lift"), detail, setWords(r.d("weight"), r.l("reps") ?: 0L, unit), figureColor = c.live, divider = i > 0)
        }
    }
    val lifts = s.objs("lifts")
    if (lifts.isNotEmpty()) Card("Lifts") { lifts.forEachIndexed { i, l -> LiftRow(l, unit, divider = i > 0) } }
    val goals = s.objs("goals")
    if (goals.isNotEmpty()) Card("Goals") {
        goals.forEachIndexed { i, g ->
            val target2 = when {
                g.str("target_key").isEmpty() && g.d("target_value") != null -> weight(g.d("target_value")!!)
                g.d("target_value") != null -> "${word(g.str("target_key"))} · ${weight(g.d("target_value")!!)}"
                else -> word(g.str("target_key"))
            }
            FigureRow("${word(g.str("kind"))}: $target2", g.s("note")?.takeIf { it.isNotBlank() } ?: "Set ${humanDate(g.s("created_day"))}", divider = i > 0)
        }
    }
}

/** Twelve weeks of volume, this one last and brightest; a tap names a week. */
@Composable
private fun WeekBars(weeks: List<JsonObject>, unit: String) {
    val c = Relay.colors
    var picked by remember(weeks) { mutableStateOf(weeks.lastIndex) }
    val values = weeks.map { it.d("volume") ?: 0.0 }
    val hi = values.maxOrNull()?.takeIf { it > 0 } ?: 1.0
    Column(Modifier.padding(top = 4.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(Modifier.fillMaxWidth().height(96.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom) {
            values.forEachIndexed { i, v ->
                Box(Modifier.weight(1f).fillMaxHeight().clickable { picked = i }, contentAlignment = Alignment.BottomCenter) {
                    Canvas(Modifier.fillMaxWidth().fillMaxHeight((v / hi).toFloat().coerceIn(.02f, 1f))) {
                        drawRoundRect(if (i == picked) c.ink else c.ink3.copy(alpha = .55f), cornerRadius = CornerRadius(3.dp.toPx()))
                    }
                }
            }
        }
        weeks.getOrNull(picked)?.let { w ->
            T("Week of ${humanDate(w.s("start"))} · ${volume(w.d("volume"), unit)} · ${plural(w.l("sessions") ?: 0L, "workout")}", style = Relay.type.caption, color = c.ink2)
        }
    }
}

/** A workout: its title over the day and lifts, its volume; it opens to its exercises. */
@Composable
private fun WorkoutRow(w: JsonObject, unit: String, divider: Boolean) {
    val c = Relay.colors
    var open by rememberSaveable(w.l("id")) { mutableStateOf(false) }
    val lifts = w.arr("lifts").mapNotNull { (it as? JsonPrimitive)?.content }
    val title = buildString {
        append(w.str("title"))
        if (w.b("untracked") == true) append(" · untracked")
        (w.l("prs") ?: 0L).takeIf { it > 0 }?.let { append(" · ${plural(it, "record")}") }
    }
    val detail = listOfNotNull(humanDate(w.s("day")), (w.l("minutes") ?: 0L).takeIf { it > 0 }?.let(::minutes), lifts.joinToString(", ").takeIf { it.isNotEmpty() }).joinToString(" · ")
    FigureRow(title, detail, volume(w.d("volume"), unit), divider = divider, figureColor = if (w.b("untracked") == true) c.ink3 else c.ink2, onClick = { open = !open })
    AnimatedVisibility(open) { w.l("id")?.let { Workout(it, unit) } }
}

/** One workout's exercises and sets (`gym.session.get`). */
@Composable
private fun Workout(id: Long, unit: String) {
    val c = Relay.colors
    val live = rememberLive("gym.session.get", remember(id) { buildJsonObject { put("id", id) } })
    Column(Modifier.padding(start = 4.dp, bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(7.dp)) {
        Reading(live, "This engine cannot read one workout yet.") { d ->
            d.s("journal")?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.caption, color = c.ink2) }
            for (e in d.objs("exercises")) {
                if (e.b("skipped") == true) continue
                val sets = e.objs("sets").joinToString("  ·  ") { s -> setWords(s.d("weight"), s.l("reps") ?: 0L, d.s("unit") ?: unit) }
                Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        T(e.str("name"), style = Relay.type.caption, color = c.ink, weight = FontWeight.Medium)
                        if (e.b("was_pr") == true) StatePill("Record", c.live)
                    }
                    if (sets.isNotEmpty()) T(sets, style = Relay.type.mono.copy(fontSize = 11.5.sp, lineHeight = 16.sp), color = c.ink3)
                }
            }
        }
    }
}

/** A lift: how often, its best set, and the last four weeks beside the four before. */
@Composable
private fun LiftRow(l: JsonObject, unit: String, divider: Boolean) {
    val c = Relay.colors
    val best = l.o("best")
    val detail = listOfNotNull(
        plural(l.l("sessions") ?: 0L, "workout"),
        "last ${humanDate(l.s("last_day"))}",
        best?.let { "best ${setWords(it.d("weight"), it.l("reps") ?: 0L, unit)}" },
    ).joinToString(" · ")
    val e = l.d("recent_e1rm")
    val change = l.d("change_pct")
    val figure = when {
        e != null && change != null -> "${weight(e)} $unit ${String.format(java.util.Locale.ROOT, "%+.1f%%", change)}"
        e != null -> "${weight(e)} $unit"
        else -> ""
    }
    FigureRow(l.str("name"), detail, figure, divider = divider, figureColor = when {
        change == null -> c.ink2
        change > 0 -> c.live
        change < 0 -> c.waiting
        else -> c.ink2
    })
}
