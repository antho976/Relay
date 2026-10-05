package com.tally.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.AddCard
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CopyAll
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.TableChart
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.accounts.FieldRow
import com.tally.app.ui.accounts.typeLabel
import com.tally.app.ui.common.AccountBadge
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChoiceRow
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupBlock
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.RowPill
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TransferBadge
import com.tally.app.ui.nav.AppNav
import com.tally.core.AccountType
import com.tally.core.Copy

@Composable
fun ImportRoute(nav: AppNav) {
    val viewModel: ImportViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val opener = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.read(uri)
    }
    val types = arrayOf("text/csv", "text/comma-separated-values", "application/csv", "text/tab-separated-values", "text/plain", "*/*")
    ImportScreen(
        state,
        ImportActions(
            back = nav::back,
            pick = { bank ->
                viewModel.choose(bank)
                runCatching { opener.launch(types) }
            },
            setTarget = viewModel::setTarget,
            setNewName = viewModel::setNewName,
            setNewType = viewModel::setNewType,
            setFlip = viewModel::setFlip,
            setFileAccount = viewModel::setFileAccount,
            setTransfers = viewModel::setTransfers,
            setCounterpart = viewModel::setCounterpart,
            confirm = viewModel::confirm,
            again = viewModel::again,
            history = nav::history,
        ),
    )
}

/** Everything the import can do, as plain lambdas; the file picker stays in the route. */
data class ImportActions(
    val back: () -> Unit = {},
    /** Opens the file picker under [BankSource]'s directions (null: a Tally or any CSV). */
    val pick: (BankSource?) -> Unit = {},
    val setTarget: (Long?) -> Unit = {},
    val setNewName: (String) -> Unit = {},
    val setNewType: (AccountType) -> Unit = {},
    val setFlip: (Boolean) -> Unit = {},
    val setFileAccount: (String?) -> Unit = {},
    val setTransfers: (TransferMode) -> Unit = {},
    val setCounterpart: (Long) -> Unit = {},
    val confirm: () -> Unit = {},
    val again: () -> Unit = {},
    val history: () -> Unit = {},
)

private val NEW_TYPES = listOf(AccountType.CHEQUING, AccountType.SAVINGS, AccountType.CREDIT, AccountType.INVESTMENT, AccountType.CASH)

/** How many of a statement's lines the preview lists before "and N more". */
private const val PREVIEW_LINES = 8

/**
 * Import from your bank. First the banks, each with where its CSV lives; then, with a file read,
 * the preview: how many lines go in under the light, the account they go into, how the file's
 * signs read, what to do with card payments and transfers, and the first lines as they will look.
 */
@Composable
fun ImportScreen(state: ImportState, actions: ImportActions) {
    LazyColumn(
        Modifier.fillMaxSize().imePadding().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        when (state.stage) {
            ImportStage.CHOOSE, ImportStage.READING -> chooseItems(state, actions)
            ImportStage.PREVIEW, ImportStage.WRITING -> previewItems(state, actions)
            ImportStage.DONE -> doneItems(state, actions)
        }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.chooseItems(state: ImportState, actions: ImportActions) {
    item(key = "head") {
        PageHead(
            "Import from your bank",
            onBack = actions.back,
            context = if (state.stage == ImportStage.READING) "Reading the file" else "A statement joins your entries; nothing is replaced",
        )
    }
    item(key = "banks") {
        val rows = BankSource.entries.map { bank -> bankRow(bank, state.source == bank, enabled = state.stage == ImportStage.CHOOSE, actions.pick) }
        Group(
            rows = rows,
            modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp),
            title = "Your bank",
            footer = state.problem ?: "Neither bank lets an app sign in for you offline, so the bridge is the CSV you download.",
            footerIsError = state.problem != null,
        )
    }
    item(key = "tally") {
        val tally: @Composable (Shape) -> Unit = { shape ->
            GroupRow(
                "A Tally or spreadsheet CSV",
                shape,
                subtitle = "Exported from Tally, or your own with date and amount columns",
                leading = { GlyphBadge(Icons.Rounded.TableChart) },
                trailing = { RowPill("Choose file") },
                chevron = false,
                onClick = if (state.stage == ImportStage.CHOOSE) ({ actions.pick(null) }) else null,
            )
        }
        val share: @Composable (Shape) -> Unit = { shape ->
            GroupRow(
                "Or share it to Tally",
                shape,
                subtitle = "In your browser's downloads or Files, open the CSV with Tally",
                leading = { GlyphBadge(Icons.Rounded.Share) },
            )
        }
        Group(
            rows = listOf(tally, share),
            modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp),
            title = "Other ways",
            footer = "The file is read on this phone and not kept. Lines already in the account are found and left out.",
        )
    }
}

private fun bankRow(bank: BankSource, picked: Boolean, enabled: Boolean, onPick: (BankSource?) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        bank.label,
        shape,
        modifier = Modifier.semantics { selected = picked },
        subtitle = bank.steps,
        leading = { GlyphBadge(if (bank == BankSource.OTHER) Icons.AutoMirrored.Rounded.InsertDriveFile else Icons.Rounded.AccountBalance) },
        trailing = { RowPill("Choose file") },
        chevron = false,
        onClick = if (enabled) ({ onPick(bank) }) else null,
    )
}

private fun androidx.compose.foundation.lazy.LazyListScope.previewItems(state: ImportState, actions: ImportActions) {
    val statement = state.statement ?: return
    val plan = state.plan
    item(key = "head") {
        PageHead(
            "Import",
            onBack = actions.back,
            context = statement.format.label + " · " + Copy.plural(statement.rows.size, "line"),
        )
    }
    item(key = "hero") { PlanHero(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "into") { IntoGroup(state, actions) }
    if (statement.accounts.size > 1) {
        item(key = "file-accounts") { FileAccountsGroup(state, actions) }
    }
    item(key = "signs") {
        val row: @Composable (Shape) -> Unit = { shape ->
            ChoiceRow("Money out reads", listOf("Below zero", "Above zero"), if (state.choices.flip) 1 else 0, { actions.setFlip(it == 1) }, shape)
        }
        Group(
            rows = listOf(row),
            modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp),
            title = "Reading the file",
            footer = if (state.choices.flip) "Purchases are written as positive, as on most card statements" else "Purchases are written with a minus, as on most bank accounts",
        )
    }
    if (plan.transferLike > 0) {
        item(key = "moves") { MovesGroup(state, actions) }
    }
    item(key = "lines") { LinesPanel(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "acts") {
        Column(Modifier.padding(horizontal = GUTTER), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val problem = state.problem
            if (problem != null) {
                Text(problem, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            HeroAction(
                when {
                    state.stage == ImportStage.WRITING -> "Importing"
                    plan.count == 0 -> "Nothing new to import"
                    else -> "Import " + Copy.plural(plan.count, "entry", "entries")
                },
                actions.confirm,
                Modifier.fillMaxWidth(),
                enabled = state.stage == ImportStage.PREVIEW && plan.count > 0,
            )
            SecondaryAction("Choose another file", actions.again, Modifier.fillMaxWidth(), enabled = state.stage == ImportStage.PREVIEW)
        }
    }
}

/** How many lines go in, under the light, and what the rest of the file came to. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PlanHero(state: ImportState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val plan = state.plan
    HeroPanel(modifier) {
        HeroHead("Ready to import", end = state.statement?.format?.label?.uppercase())
        Spacer(Modifier.height(12.dp))
        HeroNumber(plan.count.toString(), description = Copy.plural(plan.count, "entry", "entries") + " to import")
        Text(
            (if (plan.count == 1) "entry" else "entries") + " into " + state.targetName + " · " +
                planSpan(plan, { Dates.short(it, state.today) }, { money.formatWhole(it) }),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (plan.categorized > 0) StatChip(Icons.Rounded.AutoAwesome, Copy.plural(plan.categorized, "line") + " filed by payee")
            if (plan.uncategorized > 0) StatChip(Icons.Rounded.EditNote, "${plan.uncategorized} to file yourself")
            if (plan.duplicates > 0) StatChip(Icons.Rounded.CopyAll, "${plan.duplicates} already in Tally")
            if (plan.skipped > 0) StatChip(Icons.Rounded.SwapHoriz, Copy.plural(plan.skipped, "move") + " between accounts left out")
            val end = plan.endBalance
            if (end != null) StatChip(Icons.Rounded.AccountBalance, "Statement ends at " + money.format(end))
        }
    }
}

/** The account the lines go into: each open one, or a new one named for the bank. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun IntoGroup(state: ImportState, actions: ImportActions) {
    val money = LocalMoney.current
    val rows: List<@Composable (Shape) -> Unit> = state.accounts.map { a ->
        accountRow(a.name, typeLabel(a.type) + " · " + money.format(a.balance), a.id == state.targetId, { AccountBadge(a.type) }) { actions.setTarget(a.id) }
    } + accountRow(
        "A new account",
        "Opens at " + money.format(state.plan.openingFromStatement ?: 0L) + ", where the statement starts",
        state.targetId == null,
        { GlyphBadge(Icons.Rounded.AddCard) },
    ) { actions.setTarget(null) }
    Column(Modifier.padding(horizontal = GUTTER).padding(top = 14.dp).selectableGroup()) {
        Group(rows = rows, title = "Into")
    }
    if (state.targetId == null) {
        val name: @Composable (Shape) -> Unit = { shape ->
            FieldRow(state.newName, actions.setNewName, "Account name", Icons.Rounded.EditNote, shape, capitalization = KeyboardCapitalization.Sentences)
        }
        val type: @Composable (Shape) -> Unit = { shape ->
            GroupBlock(shape) {
                FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    NEW_TYPES.forEach { t -> ChoiceChip(typeLabel(t), t == state.newType) { actions.setNewType(t) } }
                }
            }
        }
        Group(rows = listOf(name, type), modifier = Modifier.padding(horizontal = GUTTER).padding(top = 10.dp), title = "New account")
    }
}

private fun accountRow(
    title: String,
    subtitle: String,
    picked: Boolean,
    badge: @Composable () -> Unit,
    onClick: () -> Unit,
): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        title,
        shape,
        modifier = Modifier.semantics { selected = picked },
        subtitle = subtitle,
        leading = badge,
        trailing = { if (picked) Icon(Icons.Rounded.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary) },
        chevron = false,
        onClick = onClick,
    )
}

/** A file that holds several accounts (a Desjardins export can): which one to take. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FileAccountsGroup(state: ImportState, actions: ImportActions) {
    val accounts = state.statement?.accounts.orEmpty()
    val block: @Composable (Shape) -> Unit = { shape ->
        GroupBlock(shape) {
            FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                accounts.forEach { a -> ChoiceChip(a, state.choices.fileAccount == a) { actions.setFileAccount(a) } }
                ChoiceChip("All of them", state.choices.fileAccount == null) { actions.setFileAccount(null) }
            }
        }
    }
    Group(
        rows = listOf(block),
        modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp),
        title = "From the file",
        footer = "The file holds " + Copy.plural(accounts.size, "account") + ". Import one at a time into its own account in Tally.",
    )
}

/** Card payments and transfers: left out, kept as moves to another account, or kept as entries. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MovesGroup(state: ImportState, actions: ImportActions) {
    val modes = TransferMode.entries
    val mode: @Composable (Shape) -> Unit = { shape ->
        ChoiceRow("Do with them", modes.map { it.label }, modes.indexOf(state.choices.transfers), { actions.setTransfers(modes[it]) }, shape)
    }
    val others = state.accounts.filter { it.id != state.targetId }
    val counterpart: @Composable (Shape) -> Unit = { shape ->
        GroupBlock(shape) {
            Text("The other account", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(6.dp))
            FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                others.forEach { a -> ChoiceChip(a.name, a.id == state.choices.counterpartId) { actions.setCounterpart(a.id) } }
            }
        }
    }
    val transfers = state.choices.transfers == TransferMode.TRANSFERS && others.isNotEmpty()
    Group(
        rows = if (transfers) listOf(mode, counterpart) else listOf(mode),
        modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp),
        title = "Moves between your accounts",
        trailing = state.plan.transferLike.toString(),
        footer = when (state.choices.transfers) {
            TransferMode.SKIP -> "Card payments and transfers are left out: imported as spending, they would count twice"
            TransferMode.TRANSFERS -> if (others.isEmpty()) "Add the other account first, then import them as transfers" else "Each one moves money to or from the account picked"
            TransferMode.ENTRIES -> "They come in as spending and income, filed like any other line"
        },
    )
}

/** The first lines as they will be written, and what becomes of the ones left out. */
@Composable
private fun LinesPanel(state: ImportState, modifier: Modifier = Modifier) {
    val plan = state.plan
    Panel(modifier.padding(top = 14.dp)) {
        PanelHeader("First lines", meta = Copy.plural(plan.lines.size, "line").uppercase())
        Spacer(Modifier.height(4.dp))
        plan.lines.take(PREVIEW_LINES).forEach { line -> PlannedRow(line, state) }
        val more = plan.lines.size - PREVIEW_LINES
        if (more > 0) {
            Spacer(Modifier.height(6.dp))
            Text("and $more more", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun PlannedRow(line: PlannedLine, state: ImportState) {
    val money = LocalMoney.current
    val amount = if (line.amount > 0) money.formatSigned(line.amount) else money.format(-line.amount)
    val left = line.duplicate || (line.transfer && state.choices.transfers == TransferMode.SKIP)
    val meta = listOf(
        Dates.day(line.date, state.today),
        when {
            line.duplicate -> "Already in Tally"
            line.transfer && state.choices.transfers == TransferMode.SKIP -> "Left out"
            line.transfer && state.choices.transfers == TransferMode.TRANSFERS -> "Transfer"
            else -> line.categoryName ?: "Uncategorized"
        },
    ).joinToString(" · ")
    Row(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { contentDescription = "${line.note}, $amount, $meta" }
            .heightIn(min = 60.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (line.transfer && !line.duplicate) {
            TransferBadge(size = 40.dp)
        } else {
            CategoryBadge(line.categoryIcon, line.categoryColor, size = 40.dp)
        }
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        line.note,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (left) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground,
                    )
                    Text(meta, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = {
                Text(
                    amount,
                    style = MaterialTheme.typography.titleMedium,
                    color = if (left) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground,
                    textAlign = TextAlign.End,
                )
            },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
            gap = 14.dp,
        )
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.doneItems(state: ImportState, actions: ImportActions) {
    item(key = "head") { PageHead("Imported", onBack = actions.back, context = state.done ?: "Done") }
    item(key = "acts") {
        Column(Modifier.padding(horizontal = GUTTER).padding(top = 14.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            HeroAction("See them in History", actions.history, Modifier.fillMaxWidth())
            SecondaryAction("Import another file", actions.again, Modifier.fillMaxWidth())
        }
    }
}
