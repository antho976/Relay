package com.tally.app.ui.entry

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.db.AccountBalance
import com.tally.app.ui.common.CategoryGrid
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.toneColor
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.DateSheet
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.GridKey
import com.tally.app.ui.kit.Hairline
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyGrid
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.Toggle
import com.tally.app.ui.kit.accountGlyph
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.plan.everyLabel
import com.tally.app.ui.theme.Tally
import com.tally.core.AmountInput
import com.tally.core.Frequency
import com.tally.core.MoneyFormatter
import com.tally.core.TxType

data class EntryActions(
    val back: () -> Unit = {},
    val type: (TxType) -> Unit = {},
    val key: (KeypadKey) -> Unit = {},
    val category: (Long) -> Unit = {},
    val account: (Long) -> Unit = {},
    val toAccount: (Long) -> Unit = {},
    val date: (java.time.LocalDate) -> Unit = {},
    val note: (String) -> Unit = {},
    val suggestion: (String) -> Unit = {},
    val quick: (com.tally.app.data.db.QuickPick) -> Unit = {},
    val repeat: (Boolean) -> Unit = {},
    val frequency: (Frequency) -> Unit = {},
    val save: () -> Unit = {},
    val saveAndNew: () -> Unit = {},
    val delete: () -> Unit = {},
    val duplicate: () -> Unit = {},
    val newCategory: () -> Unit = {},
    val newAccount: () -> Unit = {},
    /** The bill this entry belongs to, opened in its editor. */
    val bill: (Long) -> Unit = {},
)

@Composable
fun EntryRoute(nav: AppNav) {
    val vm: EntryViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    EntryScreen(
        state,
        EntryActions(
            back = nav::back,
            type = vm::setType,
            key = vm::press,
            category = vm::pickCategory,
            account = vm::pickAccount,
            toAccount = vm::pickToAccount,
            date = vm::setDate,
            note = vm::setNote,
            suggestion = vm::pickSuggestion,
            quick = vm::pickQuick,
            repeat = vm::setRepeat,
            frequency = vm::setFrequency,
            save = vm::save,
            saveAndNew = vm::saveAndNew,
            delete = vm::delete,
            duplicate = vm::duplicate,
            newCategory = { nav.categoryEdit(kind = if (state.draft.type == TxType.INCOME) com.tally.core.CategoryKind.INCOME else com.tally.core.CategoryKind.EXPENSE) },
            newAccount = { nav.accountEdit() },
            bill = { nav.billEdit(it) },
        ),
    )
}

/** The typed amount as the keypad builds it: the currency's symbol, then the digits in the phone's separators. */
internal fun typedAmount(input: AmountInput, fmt: MoneyFormatter): String {
    val text = input.text.ifEmpty { "0" }
    val whole = text.substringBefore('.').toLongOrNull() ?: 0L
    val grouped = java.text.NumberFormat.getIntegerInstance(fmt.locale).format(whole)
    val frac = if ('.' in text) fmt.decimalSeparator + text.substringAfter('.') else ""
    return fmt.symbol + grouped + frac
}

/**
 * The two-second log: the kind, the amount on Relay's keypad, a category from the grid (the most
 * used first), and save. The keypad shows while the amount is being typed and folds away once the
 * amount is in and a category is picked, so the rest (account, day, note, repeat) sits in one card
 * already filled with what is usual, one tap away when it is not.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EntryScreen(s: EntryState, a: EntryActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    val focus = LocalFocusManager.current
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    // Open on a new entry, folded on an edit: an edit is usually about something other than the amount.
    var padOpen by rememberSaveable { mutableStateOf(s.isNew) }
    val hasAmount = d.amount.minor > 0
    // Save, next empties the form: the next entry starts on the keypad again.
    LaunchedEffect(s.isNew, d.amount.isEmpty) { if (s.isNew && d.amount.isEmpty) padOpen = true }
    fun unfold() { focus.clearFocus(); padOpen = true }
    fun foldIfTyped() { if (hasAmount) padOpen = false }
    val title = (if (s.isNew) "New " else "Edit ") + typeLabel(d.type).lowercase()
    DetailPage(
        title = title,
        onBack = a.back,
        spacing = 14.dp,
        actions = {
            if (!s.isNew) {
                IconKey("copy", a.duplicate, label = "Copy to today")
                IconKey("trash", a.delete, label = "Delete")
            }
        },
        footer = {
            AnimatedVisibility(padOpen, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
                Keypad(showDecimal = s.digits > 0, separator = fmt.decimalSeparator, onKey = a.key)
            }
            // What still stops the save, once the amount is typed and the keypad is folded: a line that
            // appeared while typing would push the keys up under the thumb.
            if (!s.canSave && !d.amount.isEmpty && !padOpen) T(s.problem.orEmpty(), style = Tally.type.caption, color = c.waiting)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (s.isNew) Key("Save, next", a.saveAndNew, Modifier.weight(1f), enabled = s.canSave)
                Key(if (s.isNew) "Save" else "Save changes", a.save, Modifier.weight(1f), kind = KeyKind.Primary, enabled = s.canSave, glyph = "check")
            }
        },
    ) {
        if (!s.loaded) return@DetailPage
        Segmented(TxType.entries.map { it to typeLabel(it) }, d.type, a.type, Modifier.fillMaxWidth(), fill = true)

        AmountBlock(s, padOpen, fmt, onClick = ::unfold)

        if (s.quickPicks.isNotEmpty() && s.isNew) {
            Labelled("Usual") {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(s.quickPicks, key = { it.note + "/" + it.categoryId }) { q ->
                        QuickChip(q.note, fmt.formatWhole(q.lastAmount), q.categoryIcon, q.categoryColor) {
                            a.quick(q)
                            padOpen = false
                        }
                    }
                }
            }
        }

        if (d.type != TxType.TRANSFER) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                T("Category", style = Tally.type.caption, color = c.ink3, weight = FontWeight.Medium)
                CategoryGrid(
                    s.categories,
                    d.categoryId,
                    onPick = { id ->
                        a.category(id)
                        foldIfTyped()
                    },
                    onNew = a.newCategory,
                )
                s.category?.let { cat ->
                    val line = categoryLine(d.type, cat.name, s.month, s.budget, fmt)
                    T(line.text, Modifier.padding(horizontal = 2.dp), Tally.type.caption, toneColor(line.tone))
                }
            }
        }

        // The rest, already filled with what is usual: one card, one row each.
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 2.dp)) {
            if (d.type == TxType.TRANSFER) {
                AccountRow("From", s.account) { foldIfTyped(); picking = "from" }
                Hairline()
                AccountRow("To", s.toAccount) { foldIfTyped(); picking = "to" }
                if (s.sameAccount) T("Pick two different accounts", Modifier.padding(start = 38.dp, bottom = 8.dp), Tally.type.caption, c.heldText)
            } else {
                AccountRow(if (d.type == TxType.INCOME) "Paid into" else "Paid from", s.account) { foldIfTyped(); picking = "from" }
            }
            Hairline()
            DayRow(d.date, s.today, pick = a.date, other = { foldIfTyped(); picking = "date" })
            Hairline()
            NoteRow(d.note, d.type, s.suggestions, onNote = a.note, onSuggestion = a.suggestion, onFocus = { padOpen = false })
            Hairline()
            RepeatRow(s, a, onBill = { s.bill?.let { a.bill(it.id) } })
        }
    }

    when (picking) {
        "from", "to" -> Sheet(onDismiss = { picking = null }, title = if (picking == "to") "To account" else "Account") {
            val chosen = if (picking == "to") d.toAccountId else d.accountId
            s.accounts.filter { !it.archived || it.id == chosen }.forEach { acc ->
                PickRow(acc.name, acc.id == chosen, onClick = {
                    if (picking == "to") a.toAccount(acc.id) else a.account(acc.id)
                    picking = null
                }, sub = com.tally.app.ui.accounts.typeLabel(acc.type), leading = { AccountBadge(acc.type) })
            }
            Key("New account", { picking = null; a.newAccount() }, glyph = "plus", kind = KeyKind.Quiet)
        }
        "date" -> DateSheet(d.date, onPick = a.date, onDismiss = { picking = null })
    }
}

/**
 * The amount as typed, and where the month stands once it is in. Tapping it brings the keypad
 * back; while the keypad shows, a caret marks where the digits go.
 */
@Composable
private fun AmountBlock(s: EntryState, padOpen: Boolean, fmt: MoneyFormatter, onClick: () -> Unit) {
    val c = Tally.colors
    val d = s.draft
    val after = afterLine(d.type, s.leftAfter, s.spentAfter, fmt)
    Column(
        Modifier.fillMaxWidth().clip(Radii.card)
            .clickable(role = Role.Button, onClickLabel = "Change the amount", onClick = onClick)
            .semantics { contentDescription = "Amount " + fmt.format(d.amount.minor) }
            .padding(vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            T(
                typedAmount(d.amount, fmt),
                style = Tally.type.hero.copy(fontSize = 44.sp, lineHeight = 50.sp),
                color = if (d.amount.isEmpty) c.ink3 else if (d.type == TxType.INCOME) c.liveText else c.ink,
                maxLines = 1,
            )
            if (padOpen) Caret() else Glyph("edit", 15.dp, c.ink3, Modifier.padding(start = 10.dp))
        }
        after?.let { T(it.text, style = Tally.type.caption, color = toneColor(it.tone), align = androidx.compose.ui.text.style.TextAlign.Center) }
    }
}

/** A text caret after the amount, blinking at a reading pace. */
@Composable
private fun Caret() {
    val blink by rememberInfiniteTransition(label = "caret").animateFloat(
        initialValue = 1f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(tween(530, easing = LinearEasing), RepeatMode.Reverse),
        label = "caret",
    )
    Box(Modifier.padding(start = 4.dp).width(2.dp).height(36.dp).alpha(if (blink > .5f) 1f else 0f).clip(Radii.pill).background(Tally.colors.ink2))
}

/** The account an entry comes from (or goes to): its kind's glyph, what the row is, the account's name. */
@Composable
private fun AccountRow(label: String, account: AccountBalance?, onClick: () -> Unit) {
    val c = Tally.colors
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 10.dp, vertical = 10.dp)) {
        Glyph(account?.let { accountGlyph(it.type) } ?: "bank", 16.dp, c.ink3)
        T(label, style = Tally.type.ui, color = c.ink2, maxLines = 1)
        T(
            account?.name ?: "Pick an account",
            Modifier.weight(1f),
            Tally.type.uiMedium,
            if (account != null) c.ink else c.waiting,
            maxLines = 1,
            align = androidx.compose.ui.text.style.TextAlign.End,
        )
        Glyph("chevron-down", 13.dp, c.ink3)
    }
}

/** The day: today and yesterday one tap each, any other from the calendar. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun DayRow(date: java.time.LocalDate, today: java.time.LocalDate, pick: (java.time.LocalDate) -> Unit, other: () -> Unit) {
    val c = Tally.colors
    Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(horizontal = 10.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Glyph("calendar", 16.dp, c.ink3)
        FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val yesterday = today.minusDays(1)
            Pill("Today", selected = date == today) { pick(today) }
            Pill("Yesterday", selected = date == yesterday) { pick(yesterday) }
            val elsewhere = date != today && date != yesterday
            Pill(if (elsewhere) Dates.day(date, today) else "Another day", selected = elsewhere, onClick = other)
        }
    }
}

/** The note, and notes used before that start with what is typed. */
@Composable
private fun NoteRow(note: String, type: TxType, suggestions: List<String>, onNote: (String) -> Unit, onSuggestion: (String) -> Unit, onFocus: () -> Unit) {
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    // The system keyboard and the keypad never share the screen.
    LaunchedEffect(focused) { if (focused) onFocus() }
    Column(Modifier.fillMaxWidth()) {
        Field(
            note,
            onNote,
            Modifier.fillMaxWidth(),
            placeholder = if (type == TxType.INCOME) "Who paid" else "Where, or what for",
            leading = "notes",
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
            bare = true,
            interactionSource = source,
        )
        if (suggestions.isNotEmpty()) {
            LazyRow(
                Modifier.fillMaxWidth().padding(bottom = 8.dp),
                contentPadding = PaddingValues(horizontal = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                items(suggestions.take(6)) { s -> Pill(s, onClick = { onSuggestion(s) }) }
            }
        }
    }
}

/**
 * Repeat: on a new entry, or one that is not yet a bill's, a switch that makes a bill from it on
 * save. An entry that is already one's says when it posts next and opens that bill.
 */
@Composable
private fun RepeatRow(s: EntryState, a: EntryActions, onBill: () -> Unit) {
    val c = Tally.colors
    val d = s.draft
    val bill = s.bill
    Column(Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Glyph("repeat", 16.dp, c.ink3)
            Column(Modifier.weight(1f).padding(start = 10.dp, end = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                if (bill != null) {
                    T("Repeats " + everyLabel(bill.frequency, bill.interval).lowercase(), style = Tally.type.ui, color = c.ink)
                    T(linkedBillLine(bill) { Dates.day(it, s.today) }, style = Tally.type.caption, color = c.ink3)
                } else {
                    T("Repeat", style = Tally.type.ui, color = c.ink)
                    T(
                        if (d.repeat) "Posts itself next on " + Dates.day(s.repeatNext, s.today) else "Rent, a salary, a subscription",
                        style = Tally.type.caption,
                        color = c.ink3,
                    )
                }
            }
            if (bill != null) Key("Open bill", onBill, kind = KeyKind.Plain, compact = true)
            else Toggle(d.repeat, a.repeat)
        }
        if (bill == null && d.repeat) {
            Segmented(Frequency.entries.map { it to frequencyWord(it) }, d.frequency, a.frequency, Modifier.fillMaxWidth(), fill = true)
        }
    }
}

internal fun frequencyWord(f: Frequency): String = when (f) {
    Frequency.WEEKLY -> "Weekly"
    Frequency.MONTHLY -> "Monthly"
    Frequency.YEARLY -> "Yearly"
}

/** One of the usual entries: its note and its last amount, filled in one tap. */
@Composable
private fun QuickChip(note: String, amount: String, icon: String?, color: Int?, onClick: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.height(40.dp).clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill)
            .clickable(role = Role.Button, onClick = onClick).padding(start = 6.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        CategoryBadge(icon, color, size = 26.dp)
        T(note, style = Tally.type.caption, color = c.ink, maxLines = 1, weight = FontWeight.Medium)
        T(amount, style = Tally.type.mono, color = c.ink3, maxLines = 1)
    }
}

/**
 * The editor's keypad, on Relay's key grid at the 44dp floor: it shares the screen with the
 * category grid, so its rows stay as low as a thumb allows.
 */
@Composable
fun Keypad(showDecimal: Boolean, separator: Char, onKey: (KeypadKey) -> Unit, modifier: Modifier = Modifier) {
    val rows = remember(showDecimal, separator, onKey) {
        keypadRows(showDecimal).map { row ->
            row.map { key ->
                key?.let {
                    when (it) {
                        is KeypadKey.Digit -> GridKey(it.value.toString()) { onKey(it) }
                        KeypadKey.Decimal -> GridKey(separator.toString()) { onKey(it) }
                        KeypadKey.Backspace -> GridKey("Delete", backspace = true) { onKey(it) }
                    }
                }
            }
        }
    }
    KeyGrid(rows, modifier.padding(bottom = 2.dp), keyHeight = 44.dp, gap = 5.dp)
}
