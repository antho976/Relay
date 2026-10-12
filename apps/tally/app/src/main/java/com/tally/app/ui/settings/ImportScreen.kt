package com.tally.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.invest.investActionLabel
import com.tally.app.ui.invest.investFileLine
import com.tally.app.ui.invest.investPlanLine
import com.tally.app.ui.invest.newAccountName
import com.tally.app.ui.invest.registrationLabel
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.ToggleRow
import com.tally.app.ui.kit.TransferBadge
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.core.AccountType

@Composable
fun ImportRoute(nav: AppNav) {
    val vm: ImportViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    // The Storage Access Framework: the person picks the file, so Tally needs no storage permission.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri -> if (uri != null) vm.read(uri) }
    ImportScreen(
        s,
        back = nav::back,
        pick = { runCatching { picker.launch(arrayOf("text/*", "application/csv", "application/vnd.ms-excel", "*/*")) } },
        choose = vm::choose,
        target = vm::setTarget,
        newName = vm::setNewName,
        newType = vm::setNewType,
        flip = vm::setFlip,
        fileAccount = vm::setFileAccount,
        transfers = vm::setTransfers,
        counterpart = vm::setCounterpart,
        mapAccount = vm::mapAccount,
        statementAccount = vm::setStatementAccount,
        confirm = vm::confirm,
        again = vm::again,
        done = { if (s.invest != null || s.investFirst) nav.investmentsAfterImport() else nav.back() },
    )
}

/**
 * A bank statement read on the phone: which bank, the file, then a preview of every line (filed the
 * way the owner filed the payee before, lines already logged left out) and one import.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ImportScreen(
    s: ImportState,
    back: () -> Unit,
    pick: () -> Unit,
    choose: (BankSource?) -> Unit,
    target: (Long?) -> Unit,
    newName: (String) -> Unit,
    newType: (AccountType) -> Unit,
    flip: (Boolean) -> Unit,
    fileAccount: (String?) -> Unit,
    transfers: (TransferMode) -> Unit,
    counterpart: (Long) -> Unit,
    mapAccount: (String, Long?) -> Unit,
    statementAccount: (Long) -> Unit,
    confirm: () -> Unit,
    again: () -> Unit,
    done: () -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    val short: (java.time.LocalDate) -> String = { Dates.short(it, s.today) }
    val previewFooter: @Composable ColumnScope.() -> Unit = {
        val invest = s.invest
        val label = if (invest != null) investActionLabel(invest.preview) else "Import " + (if (s.plan.count == 1) "1 entry" else "${s.plan.count} entries")
        val can = if (invest != null) invest.ready && invest.preview.count > 0 else s.plan.count > 0
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Key("Another file", again, Modifier.weight(1f))
            Key(label, confirm, Modifier.weight(1.4f), kind = KeyKind.Primary, enabled = can, glyph = "download")
        }
    }
    val doneFooter: @Composable ColumnScope.() -> Unit = {
        Key("Done", done, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "check")
    }
    DetailPage(
        if (s.investFirst) "Import investments" else "Import a statement",
        back,
        footer = when (s.stage) {
            ImportStage.PREVIEW -> previewFooter
            ImportStage.DONE -> doneFooter
            else -> null
        },
    ) {
        s.problem?.let { T(it, style = Tally.type.ui, color = c.heldText) }
        when (s.stage) {
            ImportStage.CHOOSE -> {
                if (!s.investFirst) {
                    Labelled("Bank") {
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            BankSource.entries.forEach { b -> PickRow(b.label, s.source == b, onClick = { choose(b) }, sub = b.steps, leading = { Glyph("bank", 18.dp, c.ink3) }) }
                        }
                    }
                } else {
                    T("Wealthsimple's holdings report, its activities export, or a TFSA's or RRSP's monthly statement. Download it from Documents on wealthsimple.com.", style = Tally.type.ui, color = c.ink2)
                }
                Key("Choose the file", pick, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "file")
                T("The file is read on this phone; only the entries you import are kept, and they sync like any other.", style = Tally.type.caption, color = c.ink3)
            }
            ImportStage.READING -> Empty("file", "Reading the file", null)
            ImportStage.WRITING -> Empty("download", "Importing", null)
            ImportStage.DONE -> Slab { Empty("check", s.done ?: "Imported", "The entries are in, filed and ready.") }
            ImportStage.PREVIEW -> {
                val invest = s.invest
                if (invest != null) InvestPreviewBody(s, invest, short, onPick = { picking = it })
                else StatementPreviewBody(s, short, fmt, target = { picking = "target" }, newName, newType, flip, fileAccount, transfers, onCounterpart = { picking = "counterpart" })
            }
        }
    }
    when (picking) {
        "target" -> Sheet(onDismiss = { picking = null }, title = "Into") {
            PickRow("A new account", s.targetId == null, onClick = { target(null); picking = null }, leading = { Glyph("plus", 18.dp, c.ink3) })
            s.accounts.forEach { a -> PickRow(a.name, s.targetId == a.id, onClick = { target(a.id); picking = null }, sub = fmt.format(a.balance), leading = { AccountBadge(a.type, size = 28.dp) }) }
        }
        "counterpart" -> Sheet(onDismiss = { picking = null }, title = "The other account") {
            s.accounts.filter { it.id != s.targetId }.forEach { a -> PickRow(a.name, s.choices.counterpartId == a.id, onClick = { counterpart(a.id); picking = null }, leading = { AccountBadge(a.type, size = 28.dp) }) }
        }
        "statement" -> Sheet(onDismiss = { picking = null }, title = "Its account") {
            s.accounts.filter { it.type == AccountType.INVESTMENT }.forEach { a -> PickRow(a.name, s.invest?.statementAccountId == a.id, onClick = { statementAccount(a.id); picking = null }, leading = { AccountBadge(a.type, size = 28.dp) }) }
        }
        null -> Unit
        else -> {
            // A file account to map: "map:<number>".
            val number = picking?.removePrefix("map:").orEmpty()
            Sheet(onDismiss = { picking = null }, title = "Goes into") {
                PickRow("A new account", s.invest?.mapping?.get(number) == null, onClick = { mapAccount(number, null); picking = null }, leading = { Glyph("plus", 18.dp, c.ink3) })
                s.accounts.filter { it.type == AccountType.INVESTMENT }.forEach { a ->
                    PickRow(a.name, s.invest?.mapping?.get(number) == a.id, onClick = { mapAccount(number, a.id); picking = null }, leading = { AccountBadge(a.type, size = 28.dp) })
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StatementPreviewBody(
    s: ImportState,
    short: (java.time.LocalDate) -> String,
    fmt: com.tally.core.MoneyFormatter,
    target: () -> Unit,
    newName: (String) -> Unit,
    newType: (AccountType) -> Unit,
    flip: (Boolean) -> Unit,
    fileAccount: (String?) -> Unit,
    transfers: (TransferMode) -> Unit,
    onCounterpart: () -> Unit,
) {
    val c = Tally.colors
    val p = s.plan
    T(planSpan(p, short) { fmt.formatWhole(it) }, style = Tally.type.caption, color = c.ink3)
    StatGrid(
        listOf(
            { m -> Stat("To import", p.count.toString(), m, glyph = "download", sub = if (p.duplicates > 0) "${p.duplicates} already there" else "none already there") },
            { m -> Stat("Filed", p.categorized.toString(), m, glyph = "tag", sub = if (p.learned > 0) "${p.learned} the way you filed them" else "${p.uncategorized} left unfiled") },
            { m -> Stat("Out", fmt.formatWhole(p.out), m, glyph = "money-out") },
            { m -> Stat("In", fmt.formatWhole(p.inn), m, glyph = "money-in") },
        ),
    )
    Labelled("Into") {
        ListRow(onClick = target) {
            Glyph("bank", 16.dp, c.ink3)
            T(s.targetName, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
            Glyph("chevron-down", 14.dp, c.ink3)
        }
    }
    if (s.targetId == null) {
        Field(s.newName, newName, Modifier.fillMaxWidth(), placeholder = "The new account's name", leading = "edit")
        Segmented(listOf(AccountType.CHEQUING to "Chequing", AccountType.CREDIT to "Card", AccountType.SAVINGS to "Savings"), s.newType, newType, Modifier.fillMaxWidth(), fill = true)
    }
    val statement = s.statement
    if (statement != null && statement.accounts.size > 1) {
        Labelled("Which account in the file") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Pill("All", selected = s.choices.fileAccount == null, onClick = { fileAccount(null) })
                statement.accounts.forEach { a -> Pill(a, selected = s.choices.fileAccount == a, onClick = { fileAccount(a) }) }
            }
        }
    }
    Slab(padding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
        ToggleRow("Purchases are positive", s.choices.flip, flip, glyph = "card", subtitle = "Most card statements write money out as positive")
    }
    if (p.transferLike > 0) {
        Labelled("Card payments and transfers (${p.transferLike})") {
            Segmented(TransferMode.entries.map { it to it.label }, s.choices.transfers, transfers, Modifier.fillMaxWidth(), fill = true)
        }
        if (s.choices.transfers == TransferMode.TRANSFERS) {
            ListRow(onClick = onCounterpart) {
                Glyph("transfer", 16.dp, c.ink3)
                T(s.accounts.firstOrNull { it.id == s.choices.counterpartId }?.name ?: "Pick the other account", Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                Glyph("chevron-down", 14.dp, c.ink3)
            }
        }
    }
    Panel("Lines", glyph = "list", subtitle = "Dimmed lines are already logged, or left out") {
        p.lines.take(200).forEach { line ->
            val out = line.duplicate || (line.transfer && s.choices.transfers == TransferMode.SKIP)
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 7.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (line.transfer) TransferBadge(size = 28.dp) else CategoryBadge(line.categoryIcon, line.categoryColor, size = 28.dp)
                Column(Modifier.weight(1f)) {
                    T(line.note.ifBlank { "No description" }, style = Tally.type.ui, color = if (out) c.ink3 else c.ink, maxLines = 1)
                    T(
                        listOfNotNull(short(line.date), line.categoryName ?: if (line.transfer) "Transfer" else "Unfiled", if (line.duplicate) "already there" else null, if (line.learned) "as you filed it" else null).joinToString(" · "),
                        style = Tally.type.caption,
                        color = c.ink3,
                        maxLines = 1,
                    )
                }
                T((if (line.amount > 0) "+" else "") + fmt.format(line.amount), style = Tally.type.amount, color = if (out) c.ink3 else if (line.amount > 0) c.liveText else c.ink)
            }
        }
        if (p.lines.size > 200) T("And ${p.lines.size - 200} more.", Modifier.padding(4.dp), Tally.type.caption, c.ink3)
    }
}

@Composable
private fun InvestPreviewBody(
    s: ImportState,
    invest: com.tally.app.ui.invest.InvestImport,
    short: (java.time.LocalDate) -> String,
    onPick: (String) -> Unit,
) {
    val c = Tally.colors
    val p = invest.preview
    T(investFileLine(p, short), style = Tally.type.ui, color = c.ink2)
    Slab(spacing = 4.dp) {
        T(p.count.toString() + " " + investPlanLine(p, s.accounts.firstOrNull { it.id == invest.statementAccountId }?.name), style = Tally.type.title, color = c.ink)
        if (p.duplicates > 0) T("${p.duplicates} already in Tally", style = Tally.type.caption, color = c.ink3)
    }
    if (p.kind == com.tally.app.ui.invest.KIND_STATEMENT) {
        Labelled("Its account") {
            ListRow(onClick = { onPick("statement") }) {
                Glyph("trend", 16.dp, c.ink3)
                T(s.accounts.firstOrNull { it.id == invest.statementAccountId }?.name ?: "Pick the account", Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                Glyph("chevron-down", 14.dp, c.ink3)
            }
        }
    } else {
        Panel("Accounts in the file", glyph = "bank") {
            p.accounts.forEach { f ->
                ListRow(onClick = { onPick("map:" + f.number) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                    Column(Modifier.weight(1f)) {
                        T(f.name.ifBlank { registrationLabel(f.registration) } + " · " + f.number, style = Tally.type.ui, color = c.ink, maxLines = 1)
                        val into = invest.mapping[f.number]?.let { id -> s.accounts.firstOrNull { it.id == id }?.name } ?: ("New: " + newAccountName(f))
                        T("Into " + into, style = Tally.type.caption, color = c.ink3, maxLines = 1)
                    }
                    Glyph("chevron-down", 14.dp, c.ink3)
                }
            }
        }
    }
    p.skipped.take(5).forEach { T(it, style = Tally.type.caption, color = c.waiting) }
}
