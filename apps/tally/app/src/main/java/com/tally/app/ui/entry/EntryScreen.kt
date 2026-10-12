package com.tally.app.ui.entry

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.db.AccountBalance
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.CategoryGlyph
import com.tally.app.ui.kit.DateSheet
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.GridKey
import com.tally.app.ui.kit.KeyGrid
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
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
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.core.AmountInput
import com.tally.core.Copy
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
 * used first), and save. Everything else (account, day, note, repeat) is already filled with what
 * is usual, one tap away when it is not.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EntryScreen(s: EntryState, a: EntryActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    val title = if (s.isNew) "New " + typeLabel(d.type).lowercase() else "Edit " + typeLabel(d.type).lowercase()
    DetailPage(
        title = title,
        onBack = a.back,
        actions = {
            if (!s.isNew) {
                IconKey("copy", a.duplicate, label = "Copy to today")
                IconKey("trash", a.delete, label = "Delete")
            }
        },
        footer = {
            Keypad(showDecimal = s.digits > 0, separator = fmt.decimalSeparator, onKey = a.key)
            if (!s.canSave && !d.amount.isEmpty) T(s.problem.orEmpty(), style = Tally.type.caption, color = c.waiting)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (s.isNew) Key("Save, next", a.saveAndNew, Modifier.weight(1f), enabled = s.canSave)
                Key(if (s.isNew) "Save" else "Save changes", a.save, Modifier.weight(1f), kind = KeyKind.Primary, enabled = s.canSave, glyph = "check")
            }
        },
    ) {
        if (!s.loaded) return@DetailPage
        Segmented(TxType.entries.map { it to typeLabel(it) }, d.type, a.type, Modifier.fillMaxWidth(), fill = true)

        // The amount, and what it leaves.
        Column(Modifier.fillMaxWidth().padding(vertical = 6.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            T(
                typedAmount(d.amount, fmt),
                Modifier.semantics { contentDescription = "Amount " + fmt.format(d.amount.minor) },
                Tally.type.hero.copy(fontSize = 44.sp),
                if (d.amount.isEmpty) c.ink3 else if (d.type == TxType.INCOME) c.liveText else c.ink,
                maxLines = 1,
            )
            val after = s.leftAfter
            when {
                after != null && after < 0 -> T(fmt.formatWhole(-after) + " over this month's budget after this", style = Tally.type.caption, color = c.heldText)
                after != null -> T(fmt.formatWhole(after) + " left this month after this", style = Tally.type.caption, color = c.ink3)
                s.accountAfter != null && s.account != null -> T(s.account.name + " at " + fmt.format(s.accountAfter) + " after this", style = Tally.type.caption, color = c.ink3)
            }
        }

        if (s.quickPicks.isNotEmpty() && s.isNew) {
            Labelled("Usual") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    s.quickPicks.forEach { q ->
                        QuickChip(q.note, fmt.formatWhole(q.lastAmount), q.categoryIcon, q.categoryColor) { a.quick(q) }
                    }
                }
            }
        }

        if (d.type == TxType.TRANSFER) {
            AccountField("From", s.account, fmt) { picking = "from" }
            AccountField("To", s.toAccount, fmt) { picking = "to" }
            if (s.sameAccount) T("Pick two different accounts", style = Tally.type.caption, color = c.heldText)
        } else {
            Labelled("Category") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    s.categories.forEach { cat ->
                        CategoryChip(cat.name, cat.icon, cat.color, selected = cat.id == d.categoryId) { a.category(cat.id) }
                    }
                    Pill("New", glyph = "plus", onClick = a.newCategory)
                }
            }
            if (s.category != null && d.type == TxType.EXPENSE) {
                val budget = s.budget
                val line = Copy.plural(s.month.count, "entry", "entries") + " in " + s.category.name + " this month · " + fmt.formatWhole(s.month.total) +
                    (if (budget != null && budget > 0) " of " + fmt.formatWhole(budget) else "")
                T(line, style = Tally.type.caption, color = if (budget != null && s.month.total > budget) c.heldText else c.ink3)
            }
            AccountField("Account", s.account, fmt) { picking = "from" }
        }

        Labelled("Day") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Pill("Today", selected = d.date == s.today) { a.date(s.today) }
                Pill("Yesterday", selected = d.date == s.today.minusDays(1)) { a.date(s.today.minusDays(1)) }
                val other = d.date != s.today && d.date != s.today.minusDays(1)
                Pill(if (other) Dates.day(d.date, s.today) else "Another day", glyph = "calendar", selected = other) { picking = "date" }
            }
        }

        Labelled("Note") {
            Field(
                d.note,
                a.note,
                Modifier.fillMaxWidth(),
                placeholder = if (d.type == TxType.INCOME) "Who paid" else "Where, or what for",
                leading = "notes",
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
            )
        }
        if (s.suggestions.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                s.suggestions.take(6).forEach { note -> Pill(note, onClick = { a.suggestion(note) }) }
            }
        }

        if (s.isNew) {
            Slab(spacing = 8.dp) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Glyph("repeat", 16.dp, c.ink3)
                    Column(Modifier.weight(1f).padding(start = 10.dp)) {
                        T("Repeat", style = Tally.type.ui, color = c.ink)
                        T(if (d.repeat) "Posts itself next on " + Dates.day(s.repeatNext, s.today) else "Rent, a salary, a subscription", style = Tally.type.caption, color = c.ink3)
                    }
                    Toggle(d.repeat, a.repeat)
                }
                if (d.repeat) {
                    Segmented(Frequency.entries.map { it to frequencyWord(it) }, d.frequency, a.frequency, Modifier.fillMaxWidth(), fill = true)
                }
            }
        }
    }

    when (picking) {
        "from", "to" -> Sheet(onDismiss = { picking = null }, title = if (picking == "to") "To account" else "Account") {
            val chosen = if (picking == "to") d.toAccountId else d.accountId
            s.accounts.filter { !it.archived || it.id == chosen }.forEach { acc ->
                PickRow(acc.name, acc.id == chosen, onClick = {
                    if (picking == "to") a.toAccount(acc.id) else a.account(acc.id)
                    picking = null
                }, sub = fmt.format(acc.balance), leading = { AccountBadge(acc.type) })
            }
            Key("New account", { picking = null; a.newAccount() }, glyph = "plus", kind = KeyKind.Quiet)
        }
        "date" -> DateSheet(d.date, onPick = a.date, onDismiss = { picking = null })
    }
}

internal fun frequencyWord(f: Frequency): String = when (f) {
    Frequency.WEEKLY -> "Weekly"
    Frequency.MONTHLY -> "Monthly"
    Frequency.YEARLY -> "Yearly"
}

@Composable
private fun AccountField(label: String, account: AccountBalance?, fmt: MoneyFormatter, onClick: () -> Unit) {
    val c = Tally.colors
    Labelled(label) {
        ListRow(
            Modifier.clip(Radii.key).background(c.raised).border(1.dp, c.strong, Radii.key),
            onClick = onClick,
        ) {
            if (account != null) AccountBadge(account.type, size = 28.dp)
            T(account?.name ?: "Pick an account", Modifier.weight(1f), Tally.type.ui, if (account != null) c.ink else c.ink3, maxLines = 1)
            account?.let { T(fmt.format(it.balance), style = Tally.type.mono, color = c.ink3) }
            Glyph("chevron-down", 14.dp, c.ink3)
        }
    }
}

/** A category to pick: its icon in its hue, its name; ink when it is the one. */
@Composable
private fun CategoryChip(name: String, icon: String?, color: Int?, selected: Boolean, onClick: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.height(36.dp).clip(Radii.pill).background(if (selected) c.ink else c.slab)
            .border(1.dp, if (selected) c.ink else c.strong, Radii.pill)
            .clickable(role = Role.RadioButton, onClick = onClick).padding(start = 10.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        CategoryGlyph(icon, color, 15.dp)
        T(name, style = Tally.type.caption, color = if (selected) c.wall else c.ink2, maxLines = 1, weight = FontWeight.Medium)
    }
}

/** One of the usual entries: its note and its last amount, filled in one tap. */
@Composable
private fun QuickChip(note: String, amount: String, icon: String?, color: Int?, onClick: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.height(36.dp).clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill)
            .clickable(role = Role.Button, onClick = onClick).padding(start = 8.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        CategoryBadge(icon, color, size = 22.dp)
        T(note, style = Tally.type.caption, color = c.ink, maxLines = 1, weight = FontWeight.Medium)
        T(amount, style = Tally.type.mono, color = c.ink3, maxLines = 1)
    }
}

/** The editor's keypad, on Relay's key grid. */
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
    KeyGrid(rows, modifier)
}
