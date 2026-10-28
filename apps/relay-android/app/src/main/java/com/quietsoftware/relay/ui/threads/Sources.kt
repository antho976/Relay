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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
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
import com.quietsoftware.relay.ui.home.AskBar
import com.quietsoftware.relay.ui.kit.PillTabs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.tasks.BoardSheet
import com.quietsoftware.relay.ui.tasks.MenuRow
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOf
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToLong

// ---- The frame the three sources share ----

/** A query a page shows: the phone's cached answer first, then the PC's. */
@Composable
internal fun rememberLive(op: String, payload: JsonObject = JsonObject(emptyMap())): RelayData.Live {
    val nav = LocalNav.current
    val live by remember(op, payload) { nav.relay.live(op, payload) }.collectAsStateWithLifecycle(RelayData.Live())
    return live
}

/** A source's page: a column of at most 720dp that scrolls, its title and context over the body. */
@Composable
private fun SourcePage(
    title: String,
    context: (@Composable () -> Unit)?,
    actions: @Composable RowScope.() -> Unit = {},
    ask: Pair<String, String>? = null,
    body: @Composable ColumnScope.() -> Unit,
) {
    val c = Relay.colors
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val side = gutter(maxWidth)
        Column(Modifier.fillMaxSize()) {
            Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
                Column(Modifier.column().fillMaxWidth().padding(horizontal = side).padding(top = 8.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                            T(title, style = Relay.type.heading.copy(fontSize = 24.sp, lineHeight = 30.sp), color = c.ink)
                            context?.invoke()
                        }
                        actions()
                    }
                    body()
                }
            }
            // The space's own question, asked from its page: it starts a thread about it.
            val nav = LocalNav.current
            ask?.let { (space, hint) -> AskBar(nav, space, hint, Modifier.padding(start = 12.dp, end = 12.dp, top = 6.dp, bottom = 10.dp)) }
        }
    }
}

private fun gutter(width: androidx.compose.ui.unit.Dp) = if (width < 600.dp) 16.dp else 24.dp

/**
 * A page opened from a source's row (a lift, a strategy), as the app's detail pages are: its bar
 * with Back, its title and a line under it, and the same column of at most 720dp that scrolls.
 */
@Composable
internal fun DetailPage(nav: Nav, title: String, subtitle: String?, body: @Composable ColumnScope.() -> Unit) {
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(title, nav::back, subtitle)
            BoxWithConstraints(Modifier.fillMaxWidth().weight(1f)) {
                val side = gutter(maxWidth)
                Column(Modifier.fillMaxSize().navigationBarsPadding().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
                    Column(Modifier.column().fillMaxWidth().padding(horizontal = side).padding(top = 8.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(12.dp), content = body)
                }
            }
        }
    }
}

/** A reading's three states: its answer (with how old a saved one is), a refusal, or reading. */
@Composable
internal fun Reading(live: RelayData.Live, missing: String, content: @Composable (JsonObject) -> Unit) {
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
internal fun Stat(caption: String, figure: String, sub: String, modifier: Modifier, figureColor: Color = Relay.colors.ink, subColor: Color = Relay.colors.ink3) {
    val c = Relay.colors
    Column(modifier.clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(horizontal = 14.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        T(caption, style = Relay.type.caption, color = c.ink3, maxLines = 1)
        T(figure, style = Relay.type.heading.copy(fontSize = 21.sp, lineHeight = 26.sp, fontFeatureSettings = "tnum"), color = figureColor, maxLines = 1)
        if (sub.isNotEmpty()) T(sub, style = Relay.type.caption, color = subColor, maxLines = 2)
    }
}

/** Figures two to a row. */
@Composable
internal fun StatGrid(stats: List<@Composable (Modifier) -> Unit>) {
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
 * entries; then Entries, Plan and Invest. A bill, an entry, an account or a holding opens to the
 * question it asks, and each page has its one key to ask (docs/THREADS.md, "Tally's pages,
 * distilled").
 */
@Composable
fun TallyScreen(nav: Nav) {
    SpaceFrame(nav) {
        var tab by rememberSaveable { mutableStateOf("overview") }
        val c = Relay.colors
        val ledger by remember { nav.relay.cached("money.summary") }.collectAsStateWithLifecycle(null)
        // Entries keeps its period, search and filters while another tab shows.
        val entries = remember { EntriesFilter() }
        SourcePage("Tally", { T("The PC's ledger, read on this phone", style = Relay.type.caption, color = c.ink3) }, ask = "tally" to "Ask about your money…") {
            PillTabs(listOf("overview" to "Overview", "entries" to "Entries", "plan" to "Plan", "invest" to "Invest"), tab, { tab = it })
            val missing = "This engine does not read Tally yet. Update Relay's engine to see it here."
            when (tab) {
                "entries" -> TallyEntries(entries, Money.of(ledger?.result as? JsonObject), missing)
                "plan" -> {
                    val lists = rememberLive("money.lists")
                    Reading(rememberLive("money.summary"), missing) { s -> TallyPlan(s, lists.result as? JsonObject) }
                }
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

/** How many of the period's entries, newest first, Entries reads at a time (threads_view.rs `ENTRIES_SHOWN`). */
private const val ENTRIES_SHOWN = 60

/** How many budgets Overview shows before "Show all" (threads_view.rs `BUDGETS_SHOWN`). */
private const val BUDGETS_SHOWN = 5

/**
 * What Entries reads (money_pages.rs `refresh`): its period, 0 this one and −1 the one before,
 * its search, its account and category, and how many entries.
 */
private class EntriesFilter {
    var offset by mutableStateOf(0L)
    var query by mutableStateOf("")
    var account by mutableStateOf<Long?>(null)
    var category by mutableStateOf<Long?>(null)
    var limit by mutableStateOf(ENTRIES_SHOWN)

    val payload: JsonObject get() = buildJsonObject {
        put("limit", limit)
        if (offset != 0L) put("period_offset", offset)
        if (query.isNotEmpty()) put("query", query)
        account?.let { put("account_id", it) }
        category?.let { put("category_id", it) }
    }

    /** Another period, search or filter reads from its newest again. */
    fun reset() {
        limit = ENTRIES_SHOWN
    }
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
    Budgets(s)
    val bills = s.objs("bills").take(3)
    if (bills.isNotEmpty()) Card("Coming up") {
        bills.forEachIndexed { i, bill ->
            val inflow = bill.s("type") == "INCOME"
            val amount = bill.l("amount") ?: 0L
            val soon = !inflow && (bill.l("days_until") ?: 99L) <= 3
            // A bill to pay asks whether it can be covered; money coming in asks nothing.
            Askable(if (inflow) null else coverQuestion(bill)) { onClick ->
                FigureRow(
                    bill.str("name"), bill.str("due_line"),
                    if (inflow) money.plus(amount) else "−${money.format(amount)}",
                    figureColor = if (inflow) c.live else c.ink2,
                    detailColor = if (soon) c.waiting else c.ink3,
                    divider = i > 0,
                    onClick = onClick,
                    lead = { HueBadge("", c.ink3, 24.dp, glyph = if (inflow) "arrow-down" else "history") },
                )
            }
        }
    }
    val recent = s.objs("recent").take(5)
    Card("Recent") {
        if (recent.isEmpty()) Quiet("Nothing logged this period yet.")
        recent.forEachIndexed { i, tx -> EntryRow(tx, money, dated = true, divider = i > 0) }
    }
}

/**
 * The budgets furthest along first, each with its hue badge and a meter against the period's
 * pace; past [BUDGETS_SHOWN], "Show all 8" shows the rest in place and "Show fewer" folds them
 * again, as the desktop's panel. Its key asks the question the month raises.
 */
@Composable
private fun Budgets(s: JsonObject) {
    val c = Relay.colors
    val money = Money.of(s)
    val tick = s.o("pace")?.d("pace_fraction")?.toFloat()
    val budgets = s.objs("budgets").filter { (it.l("budget") ?: 0L) > 0 }
        .sortedByDescending { (it.l("spent") ?: 0L).toDouble() / maxOf(it.l("budget") ?: 1L, 1L) }
    var all by rememberSaveable { mutableStateOf(false) }
    AskCard("Budgets", if (budgets.isEmpty()) null else budgetsQuestion(s.o("pace")?.s("status"))) {
        if (budgets.isEmpty()) Quiet("No budgets yet. Set them in Tally's Plan.")
        budgets.take(if (all) budgets.size else BUDGETS_SHOWN).forEachIndexed { i, b ->
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
        if (budgets.size > BUDGETS_SHOWN) Key(if (all) "Show fewer" else "Show all ${budgets.size}", { all = !all }, Modifier.padding(bottom = 4.dp), kind = KeyKind.Quiet)
    }
}

/** One entry: its badge, what it was over its category and day, and the amount; it opens to the question about it. */
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
    Askable(entryQuestion(tx, money)) { onClick ->
        FigureRow(
            note ?: category, detail, txAmount(tx, money),
            figureColor = if (tx.s("type") == "INCOME") c.live else c.ink2,
            divider = divider,
            onClick = onClick,
            lead = { if (transfer) HueBadge("", c.ink3, 24.dp, glyph = "refresh") else HueBadge(category, hue(tx.l("color")), 24.dp) },
        )
    }
}

/**
 * Entries (money_pages.rs `install`, `transactions`): a period's entries by day, each day with its
 * net, back a period and forward again, a search (notes, categories and accounts) and an account
 * and a category to show alone. It reads the newest [ENTRIES_SHOWN] and more on asking; its
 * figures count those read.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TallyEntries(f: EntriesFilter, money: Money, missing: String) {
    val c = Relay.colors
    val live = rememberLive("money.tx.list", f.payload)
    val lists = rememberLive("money.lists").result as? JsonObject
    var typed by rememberSaveable { mutableStateOf(f.query) }
    var picking by remember { mutableStateOf<String?>(null) }
    // A search reads once the typing pauses, not at every letter.
    LaunchedEffect(typed) {
        if (typed.trim() == f.query) return@LaunchedEffect
        delay(400)
        f.query = typed.trim()
        f.reset()
    }
    fun named(key: String, id: Long?): String? = id?.let { lists?.objs(key)?.firstOrNull { o -> o.l("id") == it }?.s("name") }
    fun pick(key: String, id: Long?) {
        if (key == "accounts") f.account = id else f.category = id
        f.reset()
    }
    val accountName = named("accounts", f.account)
    val categoryName = named("categories", f.category)
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconKey("chevron-left", { f.offset -= 1; f.reset() }, label = "Previous period")
        T(
            (live.result as? JsonObject)?.let { periodName(it.o("period")) } ?: if (f.offset == 0L) "This period" else "Reading…",
            Modifier.weight(1f), Relay.type.uiMedium.copy(textAlign = TextAlign.Center), c.ink, maxLines = 1,
        )
        IconKey("chevron-right", { f.offset = minOf(f.offset + 1, 0L); f.reset() }, enabled = f.offset < 0L, label = "Next period")
    }
    Field(
        typed, { typed = it }, Modifier.fillMaxWidth(), placeholder = "Search entries", leading = "search",
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { f.query = typed.trim(); f.reset() }),
        trailing = {
            if (typed.isNotEmpty()) Box(Modifier.size(28.dp).clip(Radii.icon).clickable(onClickLabel = "Clear the search", role = Role.Button) { typed = ""; f.query = ""; f.reset() }.semantics { contentDescription = "Clear the search" }, contentAlignment = Alignment.Center) {
                Glyph("close", 13.dp, c.ink3)
            }
        },
    )
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Pill(accountName ?: if (f.account != null) "One account" else "All accounts", selected = f.account != null, onClick = { picking = "accounts" })
        Pill(categoryName ?: if (f.category != null) "One category" else "All categories", selected = f.category != null, onClick = { picking = "categories" })
    }
    Reading(live, missing) { page ->
        EntriesBody(page, f, money, listOfNotNull(categoryName, accountName).joinToString(" · ")) {
            typed = ""
            f.query = ""
            f.account = null
            f.category = null
            f.reset()
        }
    }
    picking?.let { key ->
        val chosen = if (key == "accounts") f.account else f.category
        // An archived one stays listed while it is the one chosen (money_pages.rs `fill_filters`).
        val rows = lists?.objs(key).orEmpty().filter { it.b("archived") != true || it.l("id") == chosen }
        BoardSheet({ picking = null }, title = if (key == "accounts") "Show the entries of one account" else "Show the entries of one category") { close ->
            Column(Modifier.heightIn(max = 440.dp).verticalScroll(rememberScrollState())) {
                MenuRow(if (key == "accounts") "All accounts" else "All categories", "menu", { close { pick(key, null) } }, selected = chosen == null)
                for (o in rows) {
                    val id = o.l("id")
                    MenuRow(o.str("name"), "menu", { close { pick(key, id) } }, selected = id == chosen, lead = { Dot(if (key == "categories") hue(o.l("color")) else c.ink3, 8.dp) })
                }
                if (lists == null) Quiet("Reading the accounts and categories…", Modifier.padding(horizontal = 10.dp))
            }
        }
    }
}

/** Entries' line: "27 entries · in $2,150 · out $2,425 · net −$275 · Dining" (money_pages.rs `tx_context`). */
private fun entriesLine(count: Int, query: String, income: Long, spent: Long, filtered: String, money: Money): String {
    val n = if (count == 1) "1 entry" else "$count entries"
    val parts = mutableListOf(if (query.isEmpty()) n else "$n matching “$query”")
    if (income > 0) parts += "in ${money.whole(income)}"
    if (spent > 0) parts += "out ${money.whole(spent)}"
    // With only one of them, the net would say it again.
    if (income > 0 && spent > 0) parts += "net ${if (income > spent) "+" else ""}${money.whole(income - spent)}"
    if (filtered.isNotEmpty()) parts += filtered
    return parts.joinToString(" · ")
}

/** A read of Entries: its line and its question, then its days; [onClear] shows every entry of the period again. */
@Composable
private fun EntriesBody(page: JsonObject, f: EntriesFilter, money: Money, filtered: String, onClear: () -> Unit) {
    val c = Relay.colors
    val rows = page.objs("transactions")
    val name = periodName(page.o("period"))
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            // A read stops at the newest it asked for, and its figures count only those.
            Eyebrow(name + if (rows.size >= f.limit) " · latest ${rows.size}" else "")
            T(entriesLine(rows.size, f.query, page.l("income") ?: 0L, page.l("spent") ?: 0L, filtered, money), style = Relay.type.mono.copy(fontSize = 12.sp), color = c.ink2)
        }
        if (rows.isNotEmpty()) AskKey(entriesQuestion(name, filtered))
    }
    if (rows.isEmpty()) {
        val narrowed = f.account != null || f.category != null
        Card("Entries") {
            Quiet(
                when {
                    f.query.isNotEmpty() -> "Nothing in $name matches “${f.query}”."
                    narrowed -> "Nothing in $name for ${filtered.ifEmpty { "this filter" }}."
                    else -> "Nothing logged in $name."
                },
            )
            if (f.query.isNotEmpty() || narrowed) Key("Show every entry", onClear, Modifier.padding(bottom = 4.dp), kind = KeyKind.Quiet)
        }
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
    if (rows.size >= f.limit) Key("Show more", { f.limit += ENTRIES_SHOWN }, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
}

/** One `money.series` question per category: its spending in this period and the three before (money_pages.rs `past`). */
private fun spendingOf(category: String) = buildJsonObject {
    put("measure", "spending")
    put("by", "period")
    put("periods", 4)
    put("category", category)
}

/** A category's spending in the three whole periods before this one, oldest first; null until it is read. */
private fun pastOf(read: RelayData.Live?): List<Long>? {
    val values = (read?.result as? JsonObject)?.objs("series")?.firstOrNull()?.arr("values") ?: return null
    if (values.size != 4) return null
    return values.take(3).map { (it as? JsonPrimitive)?.longOrNull ?: 0L }
}

/** "last month $298" and "3-mo avg $310", as far as the ledger reaches back ([months]) (money_pages.rs `Past::lines`). */
private fun pastLines(values: List<Long>?, months: Int, money: Money): List<String> {
    if (values == null || months == 0) return emptyList()
    val lines = mutableListOf("last month ${money.whole(values[2])}")
    if (months > 1) {
        val counted = values.takeLast(months)
        lines += "$months-mo avg ${money.whole(counted.sum() / counted.size)}"
    }
    return lines
}

/**
 * Plan (money_pages.rs `plan`), read only: the monthly budget and what the categories hold of it,
 * then every expense category with this period's spending against its budget, last month's
 * spending and the average of the last three whole months, one `money.series` read per category
 * as the PC's Plan reads them. Categories with no budget and nothing spent fold under one key.
 */
@Composable
private fun TallyPlan(s: JsonObject, lists: JsonObject?) {
    val c = Relay.colors
    val nav = LocalNav.current
    if (s.b("empty") == true) {
        Quiet("Tally has nothing yet. Add an account, load the sample household or import a backup in Tally on the PC.")
        return
    }
    val money = Money.of(s)
    val pace = s.o("pace")
    val budget = if (pace?.s("status") == "NO_BUDGET") 0L else pace?.l("budget") ?: 0L
    val envelopes = s.objs("budgets").associateBy { it.l("category_id") }
    val assigned = envelopes.values.sumOf { maxOf(it.l("budget") ?: 0L, 0L) }
    val given = envelopes.values.count { (it.l("budget") ?: 0L) > 0 }
    // Every expense category the ledger keeps; until they are read, those the month has an envelope for.
    val categories = lists?.objs("categories")?.filter { it.s("kind") == "EXPENSE" && it.b("archived") != true }
        ?: s.objs("budgets").map { e -> buildJsonObject { put("id", e.l("category_id")); put("name", e.str("name")); put("color", e.l("color")) } }
    val names = categories.map { it.str("name") }
    val reads by remember(names) {
        if (names.isEmpty()) flowOf(emptyList<RelayData.Live>())
        else combine(names.map { nav.relay.live("money.series", spendingOf(it)) }) { it.toList() }
    }.collectAsStateWithLifecycle(emptyList())
    val past = categories.mapIndexed { i, cat -> cat.l("id") to pastOf(reads.getOrNull(i)) }.toMap()
    // The ledger reaches back to its first period with any spending: one two months old averages two.
    val first = (0 until 3).firstOrNull { i -> past.values.any { v -> v != null && v[i] > 0 } }
    val months = if (first == null) 0 else 3 - first
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Eyebrow(periodName(s.o("period")))
            T("Read only here: a budget changes in Tally's Plan.", style = Relay.type.caption, color = c.ink3)
        }
        AskKey(SUGGEST_BUDGETS, "Suggest budgets")
    }
    val free = budget - assigned
    StatGrid(listOf(
        { m -> Stat("Monthly budget", if (budget > 0) money.whole(budget) else "—", if (budget > 0) "everything, every month" else "no monthly budget yet", m) },
        { m -> Stat("Given to categories", money.whole(assigned), plural(given, "budget"), m) },
        { m ->
            when {
                budget == 0L -> Stat("Unassigned", "—", "set a monthly budget first", m)
                free < 0 -> Stat("Over by", money.whole(-free), "categories exceed the monthly budget", m, figureColor = c.heldText)
                else -> Stat("Unassigned", money.whole(free), "not given to a category", m)
            }
        },
    ))
    var all by rememberSaveable { mutableStateOf(false) }
    val folded = categories.count { cat -> envelopes[cat.l("id")].let { e -> (e?.l("budget") ?: 0L) <= 0 && (e?.l("spent") ?: 0L) <= 0 } }
    Card("By category") {
        if (categories.isEmpty()) Quiet("No expense categories yet. Import a backup or load sample data in Tally on the PC.")
        var shown = 0
        for (cat in categories) {
            val e = envelopes[cat.l("id")]
            val amount = e?.l("budget") ?: 0L
            val used = e?.l("spent") ?: 0L
            if (amount <= 0 && used <= 0 && !all) continue
            PlanRow(cat, e, amount, used, money, pastLines(past[cat.l("id")], months, money), divider = shown > 0)
            shown++
        }
        if (folded > 0) Key(
            when {
                all -> "Hide the $folded with no budget"
                folded == 1 -> "Show 1 more category"
                else -> "Show $folded more categories"
            },
            { all = !all }, Modifier.padding(bottom = 4.dp), kind = KeyKind.Quiet,
        )
    }
}

/** A category in Plan: its badge and name over this period's reading, a meter of its budget, what it spent before, and its budget. */
@Composable
private fun PlanRow(cat: JsonObject, e: JsonObject?, amount: Long, used: Long, money: Money, past: List<String>, divider: Boolean) {
    val c = Relay.colors
    val tone = statusTone(e?.s("status"))
    val detail = when {
        amount > 0 -> "${money.whole(used)} of ${money.whole(amount)}"
        used > 0 -> "${money.whole(used)} spent, no budget"
        else -> "Nothing spent"
    }
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        HueBadge(cat.str("name"), hue(cat.l("color")), 24.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(cat.str("name"), Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                T(if (amount > 0) money.whole(amount) else "No budget", style = Relay.type.mono.copy(fontSize = 12.sp), color = if (amount > 0) c.ink2 else c.ink3)
            }
            T(detail, style = Relay.type.caption, color = tone ?: c.ink3, maxLines = 1)
            if (amount > 0) PaceMeter(used.toFloat() / amount, tone ?: hue(cat.l("color")))
            if (past.isNotEmpty()) T(past.joinToString(" · "), style = Relay.type.mono.copy(fontSize = 11.5.sp), color = c.ink3, maxLines = 1)
        }
    }
}

/**
 * The Invest view (threads_view.rs `panel_invest`): the portfolio's value and gain, how it is
 * split, the accounts, the room left this year, the largest holdings and the activity. An account
 * no file has filled is worth its balance in [lists], as the PC counts it. An account and a
 * holding open to the question about them.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Invest(p: JsonObject, lists: JsonObject?) {
    val c = Relay.colors
    if (p.b("empty") == true) {
        val ask = rememberAsk()
        Card("Investments") {
            Quiet("See your TFSA, RRSP and FHSA together: what they are worth, what they have earned, and the room left this year.")
            // Wealthsimple's files go only into a ledger kept in Canadian dollars (threads_view.rs `invest_empty`).
            if (p.s("currency") == "CAD") {
                Key("Connect Wealthsimple", { ask(CONNECT_WEALTHSIMPLE) }, kind = KeyKind.Primary)
                Quiet("A thread walks you through downloading your Wealthsimple files; Tally reads them on the PC.")
            } else {
                Quiet("Wealthsimple's files need a ledger kept in Canadian dollars. Add your investment accounts by hand on the Investments page on the PC.")
            }
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
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                Eyebrow(p.s("as_of")?.takeIf { it.isNotBlank() }?.let { "As of ${inlineDate(it.take(10))}" } ?: "No report imported yet")
                Figure(money.whole((p.l("value") ?: 0L) + hand))
                T("${if (gain > 0) "+" else ""}${money.whole(gain)} gain${p.l("gain_bps")?.let { " · ${bps(it)} on book value" }.orEmpty()}", style = Relay.type.caption, color = gainColor(gain))
            }
            AskKey(INVESTMENTS_QUESTION)
        }
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
                    T("${registrationName(a.s("registration"))} ${share(a.l("share_bps") ?: 0L)}", style = Relay.type.caption, color = c.ink2)
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
            Askable(accountQuestion(a.str("name"))) { onClick ->
                InvestRow(a.str("name"), detail, money.whole(byHand(a) ?: a.l("value") ?: 0L), a.l("gain_bps"), divider = i > 0, onClick = onClick) {
                    HueBadge(a.str("name"), registrationHue(a.s("registration")), 24.dp)
                }
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
            val detail = listOf(h.str("account"), if (h.b("no_price") == true) "no price yet" else "${share(h.l("weight_bps") ?: 0L)} of the total").joinToString(" · ")
            Askable(holdingQuestion(symbol)) { onClick ->
                InvestRow(h.s("name")?.takeIf { it.isNotBlank() } ?: symbol, detail, money.whole(h.l("value") ?: 0L), h.l("gain_bps"), divider = i > 0, onClick = onClick) {
                    Box(Modifier.width(42.dp).height(24.dp).clip(Radii.keycap).background(c.wash), contentAlignment = Alignment.Center) {
                        T(symbol.take(5), style = Relay.type.mono.copy(fontSize = 10.5.sp), color = c.ink2, maxLines = 1)
                    }
                }
            }
        }
    }
    val income = p.l("income_12m") ?: 0L
    if (income > 0) Card("Income", aside = "12 months") {
        FigureRow("Dividends and interest", "", money.plus(income), figureColor = c.live, divider = false)
    }
    InvestActivity()
    // What the engine could not square, with the question about it (money_invest.rs, Needs a look).
    val issues = p.arr("issues").mapNotNull { (it as? JsonPrimitive)?.content }
    if (issues.isNotEmpty()) AskCard("To check", issuesQuestion(issues)) {
        for (issue in issues.take(3)) T(issue, Modifier.padding(vertical = 5.dp), Relay.type.caption, c.ink2)
    }
}

/** How many activities Invest shows before Show more (money_invest.rs `ACTIVITY_SHOWN`). */
private const val ACTIVITY_SHOWN = 8

/**
 * The newest investment activity (money_invest.rs, Activity), and all the engine keeps on asking:
 * a line imported into the wrong account may be among the older ones. Each deletes, with Undo.
 */
@Composable
private fun InvestActivity() {
    var all by rememberSaveable { mutableStateOf(false) }
    // One past those shown, to know there are more.
    val live = rememberLive("money.invest.list", buildJsonObject { put("limit", if (all) 200 else ACTIVITY_SHOWN + 1) })
    val items = (live.result as? JsonObject)?.objs("activities")
    Card("Activity") {
        when {
            items == null && live.error != null -> Quiet(said(live.error!!, "This engine does not read investment activity yet."))
            items == null -> Quiet("Reading the PC…")
            items.isEmpty() -> Quiet("No activity yet. Buys, sells, contributions and dividends come in with an activities export.")
            else -> {
                items.take(if (all) items.size else ACTIVITY_SHOWN).forEachIndexed { i, a -> ActivityRow(a, divider = i > 0) }
                if (all || items.size > ACTIVITY_SHOWN) Key(if (all) "Show the newest $ACTIVITY_SHOWN" else "Show more", { all = !all }, Modifier.padding(bottom = 4.dp), kind = KeyKind.Quiet)
            }
        }
    }
}

/**
 * An activity (money_invest.rs `activity_row`): what happened, to which security, in which
 * account and when, its amount in its own currency signed by the way the cash went, and its
 * delete. Undo on the toast brings the same line back (`money.invest.restore`), as Tally deletes
 * an entry.
 */
@Composable
private fun ActivityRow(a: JsonObject, divider: Boolean) {
    val nav = LocalNav.current
    val c = Relay.colors
    val kind = a.str("type")
    val symbol = a.s("symbol")?.takeIf { it.isNotBlank() }.orEmpty()
    val held = units(a.minor("quantity", 8) ?: 0L)
    fun from(what: String) = if (symbol.isEmpty()) what else "$what from $symbol"
    val title = when (kind) {
        "BUY" -> "Bought $held $symbol"
        "SELL" -> "Sold $held $symbol"
        "REINVEST" -> "Reinvested in $held $symbol"
        "SPLIT" -> "Split: $held more $symbol"
        "TRANSFER_IN" -> if (symbol.isNotEmpty()) "$held $symbol transferred in" else "Transfer in"
        "TRANSFER_OUT" -> if (symbol.isNotEmpty()) "$held $symbol transferred out" else "Transfer out"
        "DEPOSIT" -> "Deposit"
        "WITHDRAWAL" -> "Withdrawal"
        "DIVIDEND" -> from("Dividend")
        "INTEREST" -> "Interest"
        "RETURN_OF_CAPITAL" -> from("Return of capital")
        "NOTIONAL_DISTRIBUTION" -> from("Notional distribution")
        "FX" -> "Converted ${a.str("currency")} to ${a.str("to_currency")}"
        "TAX" -> "Tax withheld"
        else -> word(kind)
    }.trim()
    val detail = listOfNotNull(a.str("account"), humanDate(a.s("date")), a.str("note").trim().takeIf { it.isNotEmpty() }).joinToString(" · ")
    val currency = a.s("currency")?.takeIf { it.isNotBlank() } ?: "CAD"
    val money = Money(currency, Money.digitsOf(currency))
    val amount = a.minor("amount", money.digits) ?: 0L
    val minus = "−${money.format(amount)}"
    val (figure, tone) = when (kind) {
        "DIVIDEND", "INTEREST", "CREDIT" -> money.plus(amount) to c.live
        "DEPOSIT", "SELL", "RETURN_OF_CAPITAL" -> money.plus(amount) to c.ink2
        "WITHDRAWAL", "FEE", "TAX", "BUY" -> minus to c.ink2
        "TRANSFER_IN" -> (if (symbol.isEmpty()) money.plus(amount) else money.format(amount)) to c.ink2
        "TRANSFER_OUT" -> (if (symbol.isEmpty()) minus else money.format(amount)) to c.ink2
        "FX" -> {
            val to = a.s("to_currency")?.takeIf { it.isNotBlank() } ?: currency
            val toMoney = Money(to, Money.digitsOf(to))
            "${money.format(amount)} → ${toMoney.format(a.minor("to_amount", toMoney.digits) ?: 0L)}" to c.ink3
        }
        "SPLIT" -> "" to c.ink3
        else -> money.format(amount) to c.ink3
    }
    // The cash's way, as the card a thread's agent records shows it (Cards.kt `ActivityCard`).
    val glyph = when (kind) {
        "DEPOSIT", "TRANSFER_IN", "DIVIDEND", "INTEREST", "CREDIT", "RETURN_OF_CAPITAL", "SELL" -> "arrow-down"
        "WITHDRAWAL", "TRANSFER_OUT", "FEE", "TAX", "BUY" -> "arrow-up"
        else -> "refresh"
    }
    // Said as an entry's delete is: "Deleted “Bought 10 XEQT” · Thu 8 Oct".
    val deleted = "Deleted “$title” · ${humanDate(a.s("date"))}"
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        HueBadge("", c.ink3, 24.dp, glyph = glyph)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            T(title, style = Relay.type.ui, color = c.ink, maxLines = 1)
            T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 2)
        }
        if (figure.isNotEmpty()) T(figure, style = Relay.type.mono.copy(fontSize = 12.5.sp, fontFeatureSettings = "tnum"), color = tone, maxLines = 1)
        IconKey("trash", {
            val id = a.l("id") ?: return@IconKey
            val payload = buildJsonObject { put("id", id) }
            nav.shell.act("money.invest.delete", payload) { nav.shell.toast(deleted) { nav.attempt("money.invest.restore", payload) } }
        }, size = 15.dp, label = "Delete activity")
    }
}

@Composable
private fun InvestRow(title: String, detail: String, value: String, gainBps: Long?, divider: Boolean, onClick: (() -> Unit)? = null, lead: @Composable () -> Unit) {
    val c = Relay.colors
    if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
    Row(
        Modifier.fillMaxWidth().heightIn(min = 44.dp).then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
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

internal val MODES = mapOf("rules" to "Rules run, AI proposes", "agent" to "AI trades within limits", "ask" to "Every order asks")

/**
 * Arbiter, the trading tool, read from the PC (arbiter_pages.rs, Overview): the connection, the
 * portfolio, each strategy with its mode, what waits for the person (Approve or Dismiss), open
 * orders, today's limits and fills; and Halt all, after a yes. A strategy opens its own page;
 * Orders and Settings are read only.
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
            actions = { Key("Halt all", { halting = true }, kind = KeyKind.Danger, enabled = s != null && s.b("halted") != true) },
            ask = "arbiter" to "Ask about your strategies…",
        ) {
            PillTabs(listOf("overview" to "Overview", "orders" to "Orders", "settings" to "Settings"), tab, { tab = it })
            when (tab) {
                "orders" -> {
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
                }
                "settings" -> Reading(rememberLive("arbiter.settings.get"), missing) { ArbiterSettings(it, s) }
                else -> Reading(live, missing) { ArbiterOverview(it, nav) }
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

/** The limits across every strategy (arbiter_pages.rs `GLOBAL`): key, caption, what it means. */
private val GLOBAL_LIMITS = listOf(
    Triple("max_exposure", "Most held, all strategies", "At cost, in the home currency."),
    Triple("daily_loss", "Daily loss, then halt all", "A loss today across strategies that halts every one of them."),
    Triple("orders_per_hour", "Orders per hour", "Across every strategy."),
    Triple("approval_minutes", "Approvals wait", "Minutes a card waits for your yes before it expires."),
    Triple("price_band_pct", "Limit price band", "How far from the best price a limit order may sit, percent."),
    Triple("slippage_pct", "Slippage assumed", "Added to market orders in backtests and paper fills, percent."),
)

/**
 * Arbiter's settings, read only (arbiter_pages.rs `settings_page`): whether Coinbase answers, the
 * paper account, and the limits across every strategy. The key itself is never read here; it
 * stays in the PC's keyring.
 */
@Composable
private fun ArbiterSettings(st: JsonObject, s: JsonObject?) {
    val c = Relay.colors
    val home = st.s("home")?.takeIf { it.isNotBlank() } ?: "CAD"
    Card("Coinbase") {
        Box(Modifier.padding(top = 4.dp, bottom = 8.dp)) {
            s?.o("connection")?.let { Connection(it) } ?: T("Paper only · no Coinbase key yet", style = Relay.type.caption, color = c.ink3)
        }
    }
    Card("Paper account") {
        FigureRow("Home currency", "Totals are shown in it. Each strategy trades in its product's own currency.", home, divider = false)
        FigureRow("Paper starts with", "What the paper account holds when it starts, in the home currency.", st.d("paper_cash")?.let { coin(it, home) } ?: "—")
    }
    val global = st.o("global")
    Card("Limits across every strategy") {
        GLOBAL_LIMITS.forEachIndexed { i, (key, caption, about) ->
            val v = global?.d(key)
            val shown = when {
                v == null -> "No limit"
                key == "max_exposure" || key == "daily_loss" -> coin(v, home)
                key == "orders_per_hour" -> "${v.roundToLong()} an hour"
                key == "approval_minutes" -> "${v.roundToLong()} min"
                else -> "${trimmed(v, 2)}%"
            }
            FigureRow(caption, about, shown, figureColor = if (v == null) c.ink3 else c.ink2, divider = i > 0)
        }
    }
    T("Read only on this phone: these change in Arbiter's settings on the PC.", Modifier.fillMaxWidth(), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
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
            Key("Restart", { nav.shell.act("arbiter.restart", JsonObject(emptyMap()), done = "Restarted. Strategies run again from their next bar.") })
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
internal fun Sparkline(points: JsonArray?) {
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

/** A strategy: its name and mode pill over who decides and what it is doing, its profit; it opens its page, and a halted one restarts here too. */
@Composable
private fun StrategyRow(r: JsonObject, divider: Boolean, nav: Nav) {
    val c = Relay.colors
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
    Column(Modifier.fillMaxWidth().clickable { r.l("id")?.let { nav.arbiterStrategy(it) } }.padding(vertical = 9.dp)) {
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
                T(doing, style = Relay.type.caption, color = if (state == "halted") c.heldText else c.ink3, maxLines = if (state == "halted") 3 else 1)
            }
            Column(horizontalAlignment = Alignment.End) {
                T(coinSigned(pnl, currency), style = Relay.type.mono.copy(fontSize = 13.sp), color = gainColor(pnl))
                T("today ${coinSigned(r.d("pnl_today") ?: 0.0, currency)}", style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3)
            }
        }
        if (state == "halted") Key("Restart", {
            nav.shell.act("arbiter.restart", buildJsonObject { put("strategy_id", r.l("id") ?: 0L) }, done = "Restarted. It runs again from its next bar.")
        }, Modifier.padding(top = 8.dp))
    }
}

/** One line of the decision log, its lamp saying what kind of decision it was. */
@Composable
internal fun DecisionRow(d: JsonObject, divider: Boolean) {
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
 * and the last workout, then recent workouts, records, lifts and goals; Workouts with the cardio
 * between them, and Lifts moving up, holding and slipping, each opening its own page. Read only;
 * the export is imported on the PC.
 */
@Composable
fun AvexScreen(nav: Nav) {
    SpaceFrame(nav) {
        val c = Relay.colors
        var tab by rememberSaveable { mutableStateOf("overview") }
        val now = rememberLive("gym.summary")
        // A copy from before this week is read again as of its export's day, so its week is the
        // one it was exported in, never a week without training (avex_pages.rs `summary`).
        val exported = (now.result as? JsonObject)?.let(::staleExport)
        val asOf = if (exported != null) rememberLive("gym.summary", remember(exported) { buildJsonObject { put("today", exported) } }) else null
        // Until that reads, or with the PC away before it ever did, today's reading.
        val summary = asOf?.takeIf { it.result != null || it.error == null } ?: now
        val stale = summary !== now
        val s = (summary.result ?: now.result) as? JsonObject
        val unit = s?.s("unit") ?: "lb"
        val missing = "This engine does not read Avex yet. Update Relay's engine to see your training here."
        val imported = s?.o("imported")
        SourcePage("Avex", context = {
            T(
                imported?.let { i -> listOfNotNull(i.s("user_name")?.takeIf { it.isNotBlank() }?.let { "$it's training" }, i.s("exported_at")?.let { "exported ${inlineDate(it.take(10))}" }, agoOf(i.s("imported_at")).takeIf { it.isNotEmpty() }?.let { "imported $it" }).joinToString(" · ") }
                    ?: "Your training, read from Avex's export",
                style = Relay.type.caption, color = c.ink3,
            )
        }, ask = "avex" to "Ask about your training…") {
            PillTabs(listOf("overview" to "Overview", "workouts" to "Workouts", "lifts" to "Lifts"), tab, { tab = it })
            when (tab) {
                "workouts" -> AvexWorkouts(unit, missing)
                // The four weeks each lift is read over end where the copy does (avex_pages.rs `refresh`).
                "lifts" -> Reading(rememberLive("gym.lifts", remember(exported) { buildJsonObject { exported?.let { put("today", it) } } }), missing) { l ->
                    LiftGroups(l, l.s("unit") ?: unit, exported)
                }
                else -> Reading(summary, missing) { AvexOverview(it, unit, stale) }
            }
        }
    }
}

private fun minutes(m: Long) = if (m >= 60) "${m / 60} h ${(m % 60).toString().padStart(2, '0')} min" else "$m min"

internal fun volume(v: Double?, unit: String) = "${group((v ?: 0.0).roundToLong())} $unit"

/** `102.5`, `100`: a weight or a count without a trailing `.0`. */
internal fun weight(x: Double) = if (abs(x - Math.round(x)) < .05) group(Math.round(x)) else String.format(java.util.Locale.ROOT, "%.1f", x)

/** `225 lb × 5`, `× 12` for a bodyweight set. */
internal fun setWords(w: Double?, reps: Long, unit: String) = if (w != null) "${weight(w)} $unit × $reps" else "× $reps"

private fun word(key: String) = key.replace('_', ' ').lowercase().replaceFirstChar { it.uppercase() }

/** The export's day when it is from before this week (avex_pages.rs `age_of`); null for a fresh copy. */
internal fun staleExport(s: JsonObject): String? {
    val day = s.o("imported")?.s("exported_at")?.take(10)?.takeIf { it.length == 10 } ?: return null
    val start = s.o("this_week")?.s("start") ?: return null
    return day.takeIf { it < start }
}

/**
 * The four figures of a copy from before this week, read as of its export: its week is the one
 * it was exported in, named by its day, beside the week before (avex_pages.rs `home`).
 */
@Composable
private fun ExportWeek(s: JsonObject, unit: String) {
    val week = s.o("this_week")
    val last = s.o("last_week")
    val target = s.o("imported")?.l("days_per_week") ?: 0L
    val vol = week?.d("volume") ?: 0.0
    val before = last?.d("volume") ?: 0.0
    // An empty week says so: a percentage of nothing reads as a collapse.
    val change = when {
        vol <= 0 -> "No workouts that week"
        before > 0 -> "${Math.round((vol / before - 1) * 100).let { if (it < 0) "−${-it}" else "+$it" }}% on the week before"
        else -> "Nothing the week before"
    }
    // Days since, counted from the export, would be wrong by the copy's age: the day is said instead.
    val lastDay = s.objs("recent").firstOrNull { it.b("untracked") != true }?.s("day")?.let(::humanDate) ?: "Never"
    StatGrid(listOf(
        { m -> Stat("Week of ${dayWords(week?.s("start"))}", "${week?.l("sessions") ?: 0}${if (target > 0) " of $target" else ""}", "The week before ${last?.l("sessions") ?: 0}", m) },
        { m -> Stat("Volume", volume(vol, unit), change, m) },
        { m -> Stat("Streak", plural(maxOf(s.l("streak_weeks") ?: 0L, 0L), "week"), "In a row with a workout", m) },
        { m -> Stat("Last workout", lastDay, "${minutes(week?.l("active_minutes") ?: 0L)} training that week", m) },
    ))
}

/** Avex's overview; [stale] when it was read as of an export from before this week. */
@Composable
private fun AvexOverview(s: JsonObject, unit: String, stale: Boolean) {
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
    if (stale) ExportWeek(s, unit) else StatGrid(listOf(
        { m -> Stat("This week", "$sessions${if (target > 0) " of $target" else ""}", "Last week ${last?.l("sessions") ?: 0}", m) },
        { m -> Stat("Volume", volume(vol, unit), if (vol <= 0) "No workouts yet this week" else if (before > 0) String.format(java.util.Locale.ROOT, "%+.0f%% on last week", (vol / before - 1) * 100) else "Nothing last week", m) },
        { m -> Stat("Streak", plural(maxOf(streak, 0L), "week"), "In a row with a workout", m) },
        { m -> Stat("Last workout", since, "${minutes(week?.l("active_minutes") ?: 0L)} training this week", m) },
    ))
    val nav = LocalNav.current
    val weeks = s.objs("weeks")
    if (weeks.isNotEmpty()) AskCard("Volume by week", VOLUME_QUESTION) { WeekBars(weeks, unit) }
    val recent = s.objs("recent")
    Card("Recent workouts") {
        if (recent.isEmpty()) Quiet("No workouts in this export.")
        recent.forEachIndexed { i, w -> WorkoutRow(w, unit, divider = i > 0) }
    }
    // A record opens its lift (avex_pages.rs `record_row`).
    val records = s.objs("records")
    if (records.isNotEmpty()) Card("Records") {
        records.forEachIndexed { i, r ->
            val detail = listOfNotNull(humanDate(r.s("day")), r.d("e1rm")?.let { "estimated max ${weight(it)} $unit" }).joinToString(" · ")
            FigureRow(r.str("lift"), detail, setWords(r.d("weight"), r.l("reps") ?: 0L, unit), figureColor = c.live, divider = i > 0, onClick = { nav.avexLift(r.str("lift")) })
        }
    }
    val lifts = s.objs("lifts")
    if (lifts.isNotEmpty()) AskCard("Lifts", LIFTS_QUESTION) { lifts.forEachIndexed { i, l -> LiftRow(l, unit, divider = i > 0) } }
    val goals = s.objs("goals")
    if (goals.isNotEmpty()) Card("Goals") {
        goals.forEachIndexed { i, g ->
            val target2 = when {
                g.str("target_key").isEmpty() && g.d("target_value") != null -> weight(g.d("target_value")!!)
                g.d("target_value") != null -> "${word(g.str("target_key"))} · ${weight(g.d("target_value")!!)}"
                else -> word(g.str("target_key"))
            }
            FigureRow("${word(g.str("kind"))}: $target2", g.s("note")?.takeIf { it.isNotBlank() } ?: "Set ${inlineDate(g.s("created_day"))}", divider = i > 0)
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
            T("Week of ${dayWords(w.s("start"))} · ${volume(w.d("volume"), unit)} · ${plural(w.l("sessions") ?: 0L, "workout")}", style = Relay.type.caption, color = c.ink2)
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
    AnimatedVisibility(open) { w.l("id")?.let { Workout(it, unit, w.str("title")) } }
}

/**
 * One workout's exercises and sets (`gym.session.get`); with its [title], the key that plans the
 * next one from it (avex_pages.rs `workout_page`).
 */
@Composable
internal fun Workout(id: Long, unit: String, title: String? = null) {
    val c = Relay.colors
    val live = rememberLive("gym.session.get", remember(id) { buildJsonObject { put("id", id) } })
    Column(Modifier.padding(start = 4.dp, bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(7.dp)) {
        Reading(live, "This engine cannot read one workout yet.") { d ->
            d.s("journal")?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.caption, color = c.ink2) }
            if (title != null && d.objs("exercises").isNotEmpty()) planQuestion(title).let { AskKey(it, it) }
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

/** A lift: how often, its best set, and the last four weeks beside the four before; it opens its page. */
@Composable
private fun LiftRow(l: JsonObject, unit: String, divider: Boolean) {
    val c = Relay.colors
    val nav = LocalNav.current
    val best = l.o("best")
    val detail = listOfNotNull(
        plural(l.l("sessions") ?: 0L, "workout"),
        "last ${inlineDate(l.s("last_day"))}",
        best?.let { "best ${setWords(it.d("weight"), it.l("reps") ?: 0L, unit)}" } ?: l.l("best_reps")?.let { "best ${plural(maxOf(it, 0L), "rep")}" },
    ).joinToString(" · ")
    val e = l.d("recent_e1rm")
    val change = l.d("change_pct")
    val figure = when {
        e != null && change != null -> "${weight(e)} $unit ${pct(change)}"
        e != null -> "${weight(e)} $unit"
        else -> ""
    }
    FigureRow(l.str("name"), detail, figure, divider = divider, figureColor = when {
        change == null -> c.ink2
        change > 0 -> c.live
        change < 0 -> c.waiting
        else -> c.ink2
    }, onClick = { nav.avexLift(l.str("name")) })
}

/**
 * Lifts (avex_pages.rs `lifts_page`): moving up, holding and slipping, each done most first, the
 * moves biggest first. [asOf] is the export's day when the copy is from before this week.
 */
@Composable
private fun LiftGroups(l: JsonObject, unit: String, asOf: String?) {
    val c = Relay.colors
    val lifts = l.objs("lifts")
    if (lifts.isEmpty()) {
        Card("Lifts") { Quiet("No lifts with working sets yet.") }
        return
    }
    T(
        "${plural(lifts.size, "lift")} · each is the best estimated max of the last four weeks, against the four before." +
            asOf?.let { " Counted to ${dayWords(it)}, your last export." }.orEmpty(),
        style = Relay.type.caption, color = c.ink3,
    )
    val day = asOf ?: LocalDate.now().toString()
    var asked = false
    for ((which, title) in listOf(LiftTrend.Up to "Moving up", LiftTrend.Holding to "Holding", LiftTrend.Slipping to "Slipping")) {
        val group = lifts.filter { liftTrend(it, day) == which }.let { g ->
            when (which) {
                LiftTrend.Up -> g.sortedByDescending { it.d("change_pct") ?: 0.0 }
                LiftTrend.Slipping -> g.sortedBy { it.d("change_pct") ?: 0.0 }
                LiftTrend.Holding -> g
            }
        }
        if (group.isEmpty()) continue
        // The page's one question, on its first card.
        AskCard(title, if (asked) null else LIFTS_QUESTION) { group.forEachIndexed { i, lift -> LiftRow(lift, unit, divider = i > 0) } }
        asked = true
    }
}

/** How many workouts Workouts reads at a time (avex_pages.rs `PAGE_SIZE`). */
private const val WORKOUTS_PAGE = 60

private val MONTH_OF_YEAR = DateTimeFormatter.ofPattern("MMMM yyyy", Locale.ENGLISH)

/**
 * Workouts (avex_pages.rs `workouts_page`): every one with the cardio and rest days between them,
 * newest first by month, [WORKOUTS_PAGE] at a time. A workout opens to its exercises.
 */
@Composable
private fun AvexWorkouts(unit: String, missing: String) {
    val c = Relay.colors
    var shown by rememberSaveable { mutableStateOf(WORKOUTS_PAGE) }
    val cardio = rememberLive("gym.cardio", remember { buildJsonObject { put("limit", 1000) } })
    Reading(rememberLive("gym.sessions", buildJsonObject { put("limit", shown) }), missing) { l ->
        val list = l.objs("sessions")
        val total = l.l("total") ?: list.size.toLong()
        val all = (cardio.result as? JsonObject)?.objs("entries").orEmpty()
        // A page that stops short of the first workout stops its cardio there too.
        val oldest = list.lastOrNull()?.str("day")
        val entries = if (list.size < total && oldest != null) all.filter { it.str("day") >= oldest } else all
        T(
            listOfNotNull(
                if (list.size < total) "${list.size} of ${plural(total, "workout")}" else plural(total, "workout"),
                all.size.takeIf { it > 0 }?.let { if (it == 1) "1 cardio entry" else "$it cardio entries" },
            ).joinToString(" · "),
            style = Relay.type.caption, color = c.ink3,
        )
        if (list.isEmpty() && entries.isEmpty()) {
            Card("Workouts") { Quiet("No workouts in this export.") }
            return@Reading
        }
        // Both newest first, merged by day and time; a workout before cardio of the same minute.
        val items = (list.map { it to true } + entries.map { it to false }).sortedWith(
            compareByDescending<Pair<JsonObject, Boolean>> { it.first.str("day") }.thenByDescending { it.first.str("time") }.thenBy { if (it.second) 0 else 1 },
        )
        for ((month, group) in items.groupBy { it.first.str("day").take(7) }) {
            Card(dayOf("$month-01")?.format(MONTH_OF_YEAR) ?: month) {
                group.forEachIndexed { i, (row, workout) -> if (workout) WorkoutRow(row, unit, divider = i > 0) else CardioRow(row, divider = i > 0) }
            }
        }
        when (val more = moreWorkouts(shown, list.size, total)) {
            null -> if (list.size < total) T("The newest $SESSIONS_MAX show here.", style = Relay.type.caption, color = c.ink3)
            else -> Key("Show $more more", { shown += more }, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
        }
    }
}

/** How many more workouts Show more reads: a page, as far as the PC reads at once (`gym.sessions` stops at [SESSIONS_MAX]); null when none can come. */
internal fun moreWorkouts(shown: Int, listed: Int, total: Long): Int? =
    minOf(total - listed, WORKOUTS_PAGE.toLong(), (SESSIONS_MAX - shown).toLong()).toInt().takeIf { listed < total && it > 0 }

/** The most workouts `gym.sessions` reads at once (gym.rs). */
private const val SESSIONS_MAX = 500

/** A cardio or rest-day entry among the workouts (avex_pages.rs `cardio_row`): all there is of it, so it opens nothing. */
@Composable
private fun CardioRow(e: JsonObject, divider: Boolean) {
    val c = Relay.colors
    val kind = e.str("kind")
    val rest = kind.equals("rest", ignoreCase = true)
    val detail = listOfNotNull(
        humanDate(e.s("day")),
        e.d("distance_km")?.takeIf { it > 0 }?.let { "${trimmed(it, 2)} km" },
        // Avex's effort is a key ("very_hard"); the note is the person's own words, kept as typed.
        e.str("effort").takeIf { it.isNotEmpty() }?.lowercase()?.replace('_', ' '),
        e.str("note").trim().takeIf { it.isNotEmpty() },
    ).joinToString(" · ")
    val m = e.l("minutes") ?: 0L
    FigureRow(if (rest) "Rest day" else word(kind), detail, if (rest || m <= 0) "" else minutes(m), figureColor = c.ink3, divider = divider)
}
