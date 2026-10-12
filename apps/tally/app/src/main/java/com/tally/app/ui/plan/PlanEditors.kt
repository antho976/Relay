package com.tally.app.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.refusalLine
import com.tally.app.ui.entry.frequencyWord
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.CategoryGlyph
import com.tally.app.ui.kit.DateSheet
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.GridKey
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyGrid
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.ToggleRow
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.CategoryPalette
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.AmountInput
import com.tally.core.Copy
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.MoneyFormatter
import com.tally.core.TxType

/** An amount typed on the keypad, shown as it reads: the symbol, the digits, the phone's separator. */
internal fun padText(input: AmountInput, fmt: MoneyFormatter): String {
    val text = input.text.ifEmpty { "0" }
    val whole = text.substringBefore('.').toLongOrNull() ?: 0L
    val grouped = java.text.NumberFormat.getIntegerInstance(fmt.locale).format(whole)
    val frac = if ('.' in text) fmt.decimalSeparator + text.substringAfter('.') else ""
    return fmt.symbol + grouped + frac
}

@Composable
internal fun PadKeys(digits: Int, separator: Char, onKey: (PadKey) -> Unit) {
    val rows = remember(digits, separator, onKey) {
        padRows(digits > 0).map { row ->
            row.map { key ->
                key?.let {
                    when (it) {
                        is PadKey.Digit -> GridKey(it.value.toString()) { onKey(it) }
                        PadKey.Decimal -> GridKey(separator.toString()) { onKey(it) }
                        PadKey.Backspace -> GridKey("Delete", backspace = true) { onKey(it) }
                    }
                }
            }
        }
    }
    KeyGrid(rows)
}

// ── Budget ───────────────────────────────────────────────────────────────────

@Composable
fun BudgetEditRoute(nav: AppNav) {
    val vm: BudgetEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    BudgetEditScreen(s, back = nav::back, key = vm::press, fill = vm::fill, save = vm::save, remove = vm::remove)
}

/** One monthly limit, typed on the keypad, against this month and the three before. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BudgetEditScreen(s: BudgetEditState, back: () -> Unit, key: (PadKey) -> Unit, fill: (Long) -> Unit, save: () -> Unit, remove: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val name = if (s.isOverall) "Monthly budget" else s.name ?: "Budget"
    DetailPage(
        name,
        back,
        subtitle = resetLine(s.startDay),
        actions = { if (s.existing != null) IconKey("trash", remove, label = "Remove the budget") },
        footer = {
            PadKeys(s.input.fractionDigits, fmt.decimalSeparator, key)
            Key(if (s.existing == null) "Set budget" else "Save budget", save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, enabled = s.input.minor > 0, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            if (!s.isOverall) CategoryBadge(s.icon, s.color, size = 40.dp)
            Column(Modifier.weight(1f)) {
                Eyebrow("A month")
                T(padText(s.input, fmt), style = Tally.type.hero.copy(fontSize = 40.sp), color = if (s.input.isEmpty) c.ink3 else c.ink, maxLines = 1)
            }
        }
        if (s.input.minor > 0) {
            PaceMeter(s.reading.spentFraction, s.reading.paceFraction, height = 8.dp)
            T(Copy.ofBudget(s.reading.spent, s.reading.budget, fmt) + " so far · " + Copy.daysLine(s.reading), style = Tally.type.caption, color = paceTone(s.reading))
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            s.suggestAverage?.let { Pill("Average " + fmt.formatWhole(it), glyph = "sparkle", onClick = { fill(it) }) }
            s.suggestLast?.let { Pill("Last month " + fmt.formatWhole(it), onClick = { fill(it) }) }
        }
        Panel("What it spent", glyph = "bars") {
            val top = (s.history.maxOrNull() ?: 0L).coerceAtLeast(1L)
            s.history.forEachIndexed { i, v ->
                val start = s.historyStarts.getOrNull(i)
                Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    T(start?.let { Dates.monthShort(it) } ?: "", Modifier.padding(end = 2.dp), Tally.type.mono, if (i == s.history.size - 1) c.ink else c.ink3)
                    Meter(v.toFloat() / top, Modifier.weight(1f), color = if (i == s.history.size - 1) c.ink else c.ink3)
                    T(fmt.formatWhole(v), style = Tally.type.mono, color = c.ink2)
                }
            }
            s.average?.let { T("Average " + fmt.formatWhole(it) + " over " + Copy.plural(s.monthsOnRecord, "month"), Modifier.padding(4.dp), Tally.type.caption, c.ink3) }
        }
    }
}

// ── Bill ─────────────────────────────────────────────────────────────────────

@Composable
fun BillEditRoute(nav: AppNav) {
    val vm: BillEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val fmt = LocalMoney.current
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    BillEditScreen(
        s,
        BillEditActions(
            back = nav::back,
            name = vm::setName,
            type = vm::setType,
            amount = { vm.setAmount(it, fmt.parse(it)) },
            account = vm::pickAccount,
            toAccount = vm::pickToAccount,
            category = vm::pickCategory,
            frequency = vm::setFrequency,
            step = vm::stepInterval,
            anchor = vm::setAnchor,
            autoPost = vm::setAutoPost,
            active = vm::setActive,
            save = vm::save,
            delete = vm::delete,
        ),
    )
}

data class BillEditActions(
    val back: () -> Unit = {},
    val name: (String) -> Unit = {},
    val type: (TxType) -> Unit = {},
    val amount: (String) -> Unit = {},
    val account: (Long) -> Unit = {},
    val toAccount: (Long) -> Unit = {},
    val category: (Long) -> Unit = {},
    val frequency: (Frequency) -> Unit = {},
    val step: (Int) -> Unit = {},
    val anchor: (java.time.LocalDate) -> Unit = {},
    val autoPost: (Boolean) -> Unit = {},
    val active: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
    val delete: () -> Unit = {},
)

/** A bill or subscription: what, how much, from where, how often, and whether it posts itself. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BillEditScreen(s: BillEditState, a: BillEditActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    val p = s.problems
    val errors = s.showErrors
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    DetailPage(
        if (s.isNew) "New bill" else d.name.ifBlank { "Bill" },
        a.back,
        actions = { if (!s.isNew) IconKey("trash", a.delete, label = "Delete the bill") },
        footer = {
            if (errors) refusalLine(listOf(p.name, p.amount, p.account))?.let { T(it, style = Tally.type.caption, color = c.heldText) }
            Key(if (s.isNew) "Add bill" else "Save bill", a.save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        Segmented(TxType.entries.map { it to com.tally.app.ui.entry.typeLabel(it) }, d.type, a.type, Modifier.fillMaxWidth(), fill = true)
        Labelled("Name", problem = p.name.takeIf { errors }) {
            Field(d.name, a.name, Modifier.fillMaxWidth(), placeholder = "Rent, Netflix, Salary", error = errors && p.name != null, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words))
        }
        Labelled("Amount", problem = p.amount.takeIf { errors }, hint = if (s.perMonth > 0 && d.frequency != Frequency.MONTHLY) fmt.formatWhole(s.perMonth) + " a month" else null) {
            Field(d.amountText, a.amount, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "coins", error = errors && p.amount != null, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
        }
        Labelled(if (d.type == TxType.TRANSFER) "From" else "Account", problem = p.account.takeIf { errors }) {
            PickField(s.accountName ?: "Pick an account", s.accountName != null) { picking = "from" }
        }
        if (d.type == TxType.TRANSFER) {
            Labelled("To") { PickField(s.toAccountName ?: "Pick an account", s.toAccountName != null) { picking = "to" } }
        } else {
            Labelled("Category") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    s.categories.forEach { cat -> Pill(cat.name, selected = cat.id == d.categoryId, onClick = { a.category(cat.id) }, dot = categoryColor(cat.color)) }
                }
            }
        }
        Labelled("How often") {
            Segmented(Frequency.entries.map { it to frequencyWord(it) }, d.frequency, a.frequency, Modifier.fillMaxWidth(), fill = true)
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T(everyLabel(d.frequency, d.interval), Modifier.weight(1f), Tally.type.ui, c.ink2)
            IconKey("minus", { a.step(-1) }, enabled = d.interval > 1, label = "Less often")
            IconKey("plus", { a.step(1) }, label = "More apart")
        }
        Labelled(if (s.isNew) "First due" else "Counted from") {
            PickField(Dates.day(d.anchorDate, s.today), true, glyph = "calendar") { picking = "date" }
        }
        if (s.preview.isNotEmpty()) {
            T("Next: " + s.preview.take(3).joinToString(", ") { Dates.short(it, s.today) }, style = Tally.type.caption, color = c.ink3)
        }
        Slab(padding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
            ToggleRow("Posts itself", d.autoPost, a.autoPost, glyph = "bolt", subtitle = if (d.autoPost) "Logged on its day, with nothing to do" else "A reminder on Overview instead; you log it")
            ToggleRow("Active", d.active, a.active, glyph = "play", subtitle = if (d.active) "Running" else "Paused: nothing posts until it is resumed")
        }
    }
    when (picking) {
        "from", "to" -> Sheet(onDismiss = { picking = null }, title = "Account") {
            val chosen = if (picking == "to") d.toAccountId else d.accountId
            s.accounts.filter { !it.archived || it.id == chosen }.forEach { acc ->
                PickRow(acc.name, acc.id == chosen, onClick = {
                    if (picking == "to") a.toAccount(acc.id) else a.account(acc.id)
                    picking = null
                }, leading = { AccountBadge(acc.type, size = 28.dp) })
            }
        }
        "date" -> DateSheet(d.anchorDate, onPick = a.anchor, onDismiss = { picking = null })
    }
}

/** A field that opens a picker: what is picked, or what to pick, and a chevron. */
@Composable
fun PickField(text: String, filled: Boolean, glyph: String? = null, onClick: () -> Unit) {
    val c = Tally.colors
    ListRow(Modifier.clip(Radii.key).background(c.raised).border(1.dp, c.strong, Radii.key), onClick = onClick) {
        glyph?.let { Glyph(it, 16.dp, c.ink3) }
        T(text, Modifier.weight(1f), Tally.type.ui, if (filled) c.ink else c.ink3, maxLines = 1)
        Glyph("chevron-down", 14.dp, c.ink3)
    }
}

// ── Goal ─────────────────────────────────────────────────────────────────────

@Composable
fun GoalRoute(nav: AppNav) {
    val vm: GoalViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val fmt = LocalMoney.current
    GoalScreen(s, back = nav::back, edit = { s.goal?.let { nav.goalEdit(it.id) } }, contribute = { text, note, out ->
        val amount = fmt.parse(text) ?: 0L
        if (amount > 0) vm.contribute(if (out) -amount else amount, note)
    }, deleteContribution = vm::deleteContribution)
}

/** One goal: where it stands, what it needs a month, and (for a pot) money put in or taken out. */
@Composable
fun GoalScreen(s: GoalState, back: () -> Unit, edit: () -> Unit, contribute: (String, String, Boolean) -> Unit, deleteContribution: (Long) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val g = s.goal
    var amount by rememberSaveable { mutableStateOf("") }
    var note by rememberSaveable { mutableStateOf("") }
    DetailPage(g?.name ?: "Goal", back, subtitle = g?.let { goalKindLabel(it.kind) }, actions = { if (g != null) IconKey("edit", edit, label = "Edit the goal") }) {
        if (!s.loaded || g == null) return@DetailPage
        val hue = categoryColor(g.color)
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 8.dp) {
            Eyebrow(if (g.reached) "Reached" else goalKindLabel(g.kind))
            T(goalReading(g, fmt), style = Tally.type.hero.copy(fontSize = 34.sp), color = c.ink, maxLines = 1)
            if (g.paceFraction != null) PaceMeter(g.fraction, g.paceFraction, height = 8.dp) else Meter(g.fraction, color = if (g.reached) c.live else hue, height = 8.dp)
            T(goalRowLine(g, fmt, s.today), style = Tally.type.caption, color = c.ink2)
        }
        s.pace?.let { pace ->
            StatGrid(
                listOf(
                    { m -> Stat("A month so far", fmt.formatWhole(pace.perMonth), m, glyph = "trend", sub = "over " + Copy.plural(pace.months, "month")) },
                    { m -> Stat("At that pace", pace.reachBy?.let { Dates.short(it, s.today) } ?: "–", m, glyph = "flag", sub = if (pace.reachBy != null) "reached by then" else "no date yet") },
                ),
            )
        }
        if (g.kind == GoalKind.SAVINGS) {
            Panel("Put money in, or take it out", glyph = "wallet") {
                Column(Modifier.padding(4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Field(amount, { amount = it }, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "coins", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
                    Field(note, { note = it }, Modifier.fillMaxWidth(), placeholder = "Note (optional)", leading = "notes")
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Key("Take out", { contribute(amount, note, true); amount = ""; note = "" }, Modifier.weight(1f), glyph = "money-out", enabled = fmt.parse(amount)?.let { it > 0 } == true)
                        Key("Put in", { contribute(amount, note, false); amount = ""; note = "" }, Modifier.weight(1f), kind = KeyKind.Primary, glyph = "money-in", enabled = fmt.parse(amount)?.let { it > 0 } == true)
                    }
                }
            }
            if (s.contributions.isNotEmpty()) {
                Panel("History", glyph = "history", subtitle = fmt.formatWhole(s.added) + " in · " + fmt.formatWhole(s.withdrawn) + " out") {
                    s.contributions.forEach { row ->
                        ListRow(onLongClick = { deleteContribution(row.id) }, onClick = {}, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                            Glyph(if (row.amount < 0) "money-out" else "money-in", 16.dp, c.ink3)
                            Column(Modifier.weight(1f)) {
                                T(row.note.ifBlank { if (row.amount < 0) "Taken out" else "Put in" }, style = Tally.type.ui, color = c.ink, maxLines = 1)
                                T(Dates.day(row.date, s.today), style = Tally.type.caption, color = c.ink3)
                            }
                            T((if (row.amount < 0) "−" else "+") + fmt.format(kotlin.math.abs(row.amount)), style = Tally.type.amount, color = if (row.amount < 0) c.ink2 else c.liveText)
                        }
                    }
                    T("Hold a row to remove it.", Modifier.padding(4.dp), Tally.type.caption, c.ink3)
                }
            }
        }
        if (g.monthly && g.history.isNotEmpty()) {
            Panel("Earlier months", glyph = "calendar") {
                g.history.asReversed().forEach { m ->
                    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Box(Modifier.size(8.dp).clip(CircleShape).background(if (m.met) c.live else c.track))
                        T(Dates.month(m.start), Modifier.weight(1f), Tally.type.ui, c.ink2)
                        T(fmt.formatWhole(m.achieved) + " of " + fmt.formatWhole(m.asked), style = Tally.type.mono, color = c.ink3)
                    }
                }
            }
        }
    }
}

@Composable
fun GoalEditRoute(nav: AppNav) {
    val vm: GoalEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val fmt = LocalMoney.current
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    GoalEditScreen(
        s,
        back = nav::back,
        name = vm::setName,
        target = { vm.setTarget(it, fmt.parse(it)) },
        date = vm::setDate,
        color = vm::setColor,
        kind = vm::setKind,
        account = vm::setAccount,
        percent = vm::setPercent,
        byShare = vm::setByShare,
        save = vm::save,
        delete = vm::delete,
    )
}

/** A goal's form: its kind (fixed once saved), name, target, date, account, share, colour. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun GoalEditScreen(
    s: GoalEditState,
    back: () -> Unit,
    name: (String) -> Unit,
    target: (String) -> Unit,
    date: (java.time.LocalDate?) -> Unit,
    color: (Int) -> Unit,
    kind: (GoalKind) -> Unit,
    account: (Long?) -> Unit,
    percent: (Int) -> Unit,
    byShare: (Boolean) -> Unit,
    save: () -> Unit,
    delete: () -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    val p = s.problems
    val errors = s.showErrors
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    DetailPage(
        if (s.isNew) "New goal" else d.name.ifBlank { "Goal" },
        back,
        actions = { if (!s.isNew) IconKey("trash", delete, label = "Delete the goal") },
        footer = {
            if (errors) refusalLine(listOf(p.name, p.target))?.let { T(it, style = Tally.type.caption, color = c.heldText) }
            Key(if (s.isNew) "Add goal" else "Save goal", save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        if (s.isNew) {
            Labelled("Kind") {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    GoalKind.entries.forEach { k ->
                        PickRow(goalKindLabel(k), d.kind == k, onClick = { kind(k) }, sub = goalKindHint(k), leading = { Glyph(goalGlyph(k), 18.dp, c.ink3) })
                    }
                }
            }
        }
        Labelled("Name", problem = p.name.takeIf { errors }) {
            Field(d.name, name, Modifier.fillMaxWidth(), placeholder = "Lisbon, an emergency fund, the Visa", error = errors && p.name != null, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
        }
        val monthly = d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE
        if (monthly) {
            Labelled("Each month") {
                Segmented(listOf(true to "A share of income", false to "A set amount"), d.byShare, byShare, Modifier.fillMaxWidth(), fill = true)
            }
        }
        if (monthly && d.byShare) {
            Labelled("Share", problem = p.target.takeIf { errors }) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    IconKey("minus", { percent(d.percent - 1) }, label = "Less")
                    T(d.percent.toString() + "%", Modifier.weight(1f), Tally.type.figure, c.ink, align = androidx.compose.ui.text.style.TextAlign.Center)
                    IconKey("plus", { percent(d.percent + 1) }, label = "More")
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    listOf(5, 10, 15, 20, 25).forEach { v -> Pill("$v%", selected = d.percent == v, onClick = { percent(v) }) }
                }
            }
        } else {
            Labelled(if (d.kind == GoalKind.BALANCE) "Amount to reach" else if (monthly) "A month" else "Target", problem = p.target.takeIf { errors },
                hint = if (d.kind == GoalKind.BALANCE) "It reads " + fmt.format(s.balanceNow) + " now" else null) {
                Field(d.targetText, target, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "coins", error = errors && p.target != null, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
            }
        }
        if (!monthly) {
            Labelled("By", hint = s.monthsLeft?.let { Copy.plural(it, "month") + " from now" }) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f)) { PickField(d.targetDate?.let { Dates.long(it) } ?: "No date", d.targetDate != null, glyph = "calendar") { picking = "date" } }
                    if (d.targetDate != null) IconKey("close", { date(null) }, label = "No date")
                }
            }
        }
        if (d.kind == GoalKind.BALANCE || d.kind == GoalKind.INVEST) {
            Labelled(if (d.kind == GoalKind.INVEST) "Into" else "Account") {
                PickField(s.accounts.firstOrNull { it.id == d.accountId }?.name ?: if (d.kind == GoalKind.INVEST) "Every investment account" else "Everything you own", true) { picking = "account" }
            }
        }
        Labelled("Colour") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                CategoryPalette.forEachIndexed { i, hue ->
                    Box(
                        Modifier.size(32.dp).clip(CircleShape).background(hue)
                            .border(2.dp, if (d.color == i) c.ink else androidx.compose.ui.graphics.Color.Transparent, CircleShape)
                            .clickable(role = Role.RadioButton) { color(i) },
                    )
                }
            }
        }
    }
    when (picking) {
        "date" -> DateSheet(d.targetDate ?: s.today.plusMonths(6), onPick = { date(it) }, onDismiss = { picking = null })
        "account" -> Sheet(onDismiss = { picking = null }, title = "Account") {
            PickRow(if (d.kind == GoalKind.INVEST) "Every investment account" else "Everything you own", d.accountId == null, onClick = { account(null); picking = null })
            s.accounts.filter { !it.archived && (d.kind != GoalKind.INVEST || it.type == com.tally.core.AccountType.INVESTMENT) }.forEach { acc ->
                PickRow(acc.name, d.accountId == acc.id, onClick = { account(acc.id); picking = null }, sub = fmt.format(acc.balance), leading = { AccountBadge(acc.type, size = 28.dp) })
            }
        }
    }
}
