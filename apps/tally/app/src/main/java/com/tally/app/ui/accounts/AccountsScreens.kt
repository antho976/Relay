package com.tally.app.ui.accounts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.refusalLine
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.Balance
import com.tally.app.ui.kit.ConfirmDialog
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.ToggleRow
import com.tally.app.ui.kit.accountGlyph
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.core.AccountType
import com.tally.core.Copy
import com.tally.core.Registration

// ── Accounts ─────────────────────────────────────────────────────────────────

@Composable
fun AccountsRoute(nav: AppNav) {
    val vm: AccountsViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    AccountsScreen(s, open = { nav.transactions(accountId = it) }, add = { nav.accountEdit() }, transfer = { nav.entry(type = com.tally.core.TxType.TRANSFER) })
}

/** Where the money sits: net worth, held against owed, and every account with its share. */
@Composable
fun AccountsScreen(s: AccountsState, open: (Long) -> Unit, add: () -> Unit, transfer: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    SpacePage(
        "Accounts",
        context = if (s.loaded) Copy.plural(s.active.size, "account") + " · " + Copy.plural(s.entries, "entry", "entries") else null,
        actions = {
            if (s.canTransfer) IconKey("transfer", transfer, label = "Move money between accounts")
            IconKey("plus", add, label = "New account")
        },
    ) {
        if (!s.loaded) return@SpacePage
        if (s.active.isEmpty() && s.archived.isEmpty()) {
            Slab { Empty("bank", "No accounts yet", "An account is where money sits: a chequing account, a card, cash. Entries come out of one.") { Key("Add an account", add, glyph = "plus") } }
            return@SpacePage
        }
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 6.dp) {
            Eyebrow("Net worth")
            T(fmt.formatWhole(s.net), style = Tally.type.hero, color = if (s.net < 0) c.heldText else c.ink, maxLines = 1)
            T(
                (if (s.sinceOpening >= 0) "Up " else "Down ") + fmt.formatWhole(kotlin.math.abs(s.sinceOpening)) + " since the accounts opened",
                style = Tally.type.caption,
                color = if (s.sinceOpening >= 0) c.liveText else c.heldText,
            )
        }
        StatGrid(
            listOf(
                { m -> Stat("Held", fmt.formatWhole(s.held), m, glyph = "wallet", sub = s.largest?.let { "Most in " + it.account.name }) },
                { m -> Stat("Owed", fmt.formatWhole(s.owed), m, glyph = "card", figureColor = if (s.owed > 0) c.heldText else c.ink, sub = s.busiest?.let { "Busiest: " + it.account.name }) },
            ),
        )
        Panel("Accounts", glyph = "bank", subtitle = s.defaultName?.let { "New entries start in $it" }) {
            s.active.forEach { line -> AccountLineRow(line, onClick = { open(line.account.id) }) }
        }
        if (s.archived.isNotEmpty()) {
            Panel("Archived", glyph = "save") {
                s.archived.forEach { line -> AccountLineRow(line, onClick = { open(line.account.id) }) }
            }
        }
    }
}

@Composable
private fun AccountLineRow(line: AccountLine, onClick: () -> Unit) {
    val c = Tally.colors
    val a = line.account
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 10.dp)) {
        AccountBadge(a.type)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(5.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(a.name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                Balance(a.balance)
            }
            if (!a.archived) Meter(line.share, color = if (a.balance < 0) c.held else c.ink3, height = 3.dp)
            T(
                listOfNotNull(typeLabel(a.type), if (line.isDefault) "default" else null, a.valuedOn?.let { "valued " + Dates.monthShort(it) + " " + it.dayOfMonth }, shareLine(line)).joinToString(" · "),
                style = Tally.type.caption,
                color = c.ink3,
                maxLines = 1,
            )
        }
    }
}

// ── The editor ───────────────────────────────────────────────────────────────

@Composable
fun AccountEditRoute(nav: AppNav) {
    val vm: AccountEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val fmt = LocalMoney.current
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    AccountEditScreen(
        s,
        AccountEditActions(
            back = nav::back,
            name = vm::setName,
            type = vm::setType,
            registration = vm::setRegistration,
            template = vm::applyTemplate,
            opening = { vm.setOpening(it, fmt.parse(it)) },
            owe = vm::setOwe,
            default = vm::setDefault,
            archived = vm::setArchived,
            recordValue = { text -> fmt.parse(text)?.let { vm.recordValue(it) } },
            deleteValue = vm::deleteValue,
            save = vm::save,
            requestDelete = vm::requestDelete,
            dismissDelete = vm::dismissDelete,
            delete = vm::delete,
            archiveInstead = vm::archiveInstead,
            entries = { nav.accountEntries(s.id) },
        ),
    )
}

data class AccountEditActions(
    val back: () -> Unit = {},
    val name: (String) -> Unit = {},
    val type: (AccountType) -> Unit = {},
    val registration: (Registration?) -> Unit = {},
    val template: (AccountTemplate) -> Unit = {},
    val opening: (String) -> Unit = {},
    val owe: (Boolean) -> Unit = {},
    val default: (Boolean) -> Unit = {},
    val archived: (Boolean) -> Unit = {},
    val recordValue: (String) -> Unit = {},
    val deleteValue: (Long) -> Unit = {},
    val save: () -> Unit = {},
    val requestDelete: () -> Unit = {},
    val dismissDelete: () -> Unit = {},
    val delete: () -> Unit = {},
    val archiveInstead: () -> Unit = {},
    val entries: () -> Unit = {},
)

/** An account: its name and kind, the balance it opened at, and whether new entries start in it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AccountEditScreen(s: AccountEditState, a: AccountEditActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    val p = s.problems
    val errors = s.showErrors
    var value by rememberSaveable { mutableStateOf("") }
    DetailPage(
        if (s.isNew) "New account" else s.storedName.ifBlank { "Account" },
        a.back,
        subtitle = if (s.isNew) null else "Balance " + fmt.format(s.balance) + " · " + Copy.plural(s.entryCount, "entry", "entries"),
        actions = {
            if (!s.isNew) {
                IconKey("list", a.entries, label = "Its entries")
                IconKey("trash", a.requestDelete, label = "Delete the account")
            }
        },
        footer = {
            if (errors) refusalLine(listOf(p.name, p.opening))?.let { T(it, style = Tally.type.caption, color = c.heldText) }
            Key(if (s.isNew) "Add account" else "Save account", a.save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        if (s.isNew && s.templates.isNotEmpty()) {
            Labelled("Quick start") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    s.templates.forEach { t -> Pill(t.name, glyph = accountGlyph(t.type), onClick = { a.template(t) }) }
                }
            }
        }
        Labelled("Name", problem = p.name.takeIf { errors }) {
            Field(d.name, a.name, Modifier.fillMaxWidth(), placeholder = "Chequing, Visa, Cash", error = errors && p.name != null, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words))
        }
        Labelled("Kind") {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                AccountType.entries.forEach { t ->
                    PickRow(typeLabel(t), d.type == t, onClick = { a.type(t) }, sub = typeHint(t), leading = { Glyph(accountGlyph(t), 18.dp, c.ink3) })
                }
            }
        }
        if (d.type == AccountType.INVESTMENT) {
            Labelled("Registration") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Pill("None", selected = d.registration == null, onClick = { a.registration(null) })
                    Registration.entries.forEach { r -> Pill(r.label, selected = d.registration == r, onClick = { a.registration(r) }) }
                }
            }
        }
        Labelled(if (d.owe) "Owed when it opened" else "Balance when it opened", problem = p.opening.takeIf { errors }, hint = "What it held before the first entry in Tally") {
            Field(d.openingText, a.opening, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "coins", error = errors && p.opening != null, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
        }
        Slab(padding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
            if (d.showsOwe) ToggleRow("Money owed", d.owe, a.owe, glyph = "card", subtitle = "A card's balance counts against what you hold")
            ToggleRow("New entries start here", d.isDefault, a.default, glyph = "flag", enabled = !d.archived)
            if (!s.isNew) ToggleRow("Archived", d.archived, a.archived, glyph = "save", subtitle = "Kept with its entries, out of the pickers")
        }
        if (!s.isNew && d.type == AccountType.INVESTMENT) {
            Panel("Values", glyph = "trend", subtitle = "What it was worth on a day; the balance follows the newest") {
                Row(Modifier.padding(4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Field(value, { value = it }, Modifier.weight(1f), placeholder = "Worth today", leading = "coins", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
                    Key("Record", { a.recordValue(value); value = "" }, enabled = fmt.parse(value) != null, compact = true)
                }
                s.values.forEach { v ->
                    ListRow(onLongClick = { a.deleteValue(v.id) }, onClick = {}, padding = PaddingValues(horizontal = 4.dp, vertical = 6.dp)) {
                        T(Dates.day(v.date, s.today), Modifier.weight(1f), Tally.type.ui, c.ink2)
                        T(fmt.format(v.value), style = Tally.type.amount, color = c.ink)
                    }
                }
            }
        }
    }
    s.confirmDelete?.let { use ->
        ConfirmDialog(
            title = "Delete " + s.storedName + "?",
            body = deleteAccountLine(s.storedName, use, fmt, s.canArchive),
            confirm = "Delete",
            danger = true,
            onConfirm = a.delete,
            onDismiss = a.dismissDelete,
            alternate = if (s.canArchive) "Archive" to a.archiveInstead else null,
        )
    }
}
