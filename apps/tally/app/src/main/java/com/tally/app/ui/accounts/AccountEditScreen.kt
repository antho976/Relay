package com.tally.app.ui.accounts

import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Inventory2
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.AccountUse
import com.tally.app.ui.common.AccountBadge
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupBlock
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.SaveRefusal
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.SwitchRow
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.common.refusalLine
import com.tally.app.ui.common.selectableColors
import com.tally.app.ui.common.slab
import com.tally.app.ui.nav.AppNav
import com.tally.app.data.db.AccountValueEntity
import com.tally.core.AccountType
import com.tally.core.Copy
import java.time.LocalDate

@Composable
fun AccountEditRoute(nav: AppNav) {
    val viewModel: AccountEditViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        AccountEditActions(
            back = nav::back,
            setName = viewModel::setName,
            setType = viewModel::setType,
            applyTemplate = viewModel::applyTemplate,
            recordValue = viewModel::recordValue,
            deleteValue = viewModel::deleteValue,
            setOpening = viewModel::setOpening,
            setOwe = viewModel::setOwe,
            setDefault = viewModel::setDefault,
            setArchived = viewModel::setArchived,
            save = viewModel::save,
            requestDelete = viewModel::requestDelete,
            dismissDelete = viewModel::dismissDelete,
            delete = viewModel::delete,
            archiveInstead = viewModel::archiveInstead,
            viewEntries = nav::accountEntries,
        )
    }
    AccountEditScreen(state, actions)
}

/** Everything the account editor can do, as plain lambdas. */
data class AccountEditActions(
    val back: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setType: (AccountType) -> Unit = {},
    val applyTemplate: (AccountTemplate) -> Unit = {},
    /** Records an investment account's value today. */
    val recordValue: (Long) -> Unit = {},
    val deleteValue: (Long) -> Unit = {},
    /** The typed text and that text read as an amount (null when it is not one). */
    val setOpening: (String, Long?) -> Unit = { _, _ -> },
    val setOwe: (Boolean) -> Unit = {},
    val setDefault: (Boolean) -> Unit = {},
    val setArchived: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
    val requestDelete: () -> Unit = {},
    val dismissDelete: () -> Unit = {},
    val delete: () -> Unit = {},
    /** The delete dialog's way out that keeps everything: archive the account instead. */
    val archiveInstead: () -> Unit = {},
    val viewEntries: (Long) -> Unit = {},
)

/**
 * The account editor: the balance it will read under the light, then the name, the type, what it
 * opened with and how it is used. Deleting names what goes with it (entries, bills, and how far
 * each other account moves once its transfers with this one are gone) before it acts, and offers
 * archiving instead.
 */
@Composable
fun AccountEditScreen(state: AccountEditState, actions: AccountEditActions) {
    val d = state.draft
    var saveAttempts by rememberSaveable { mutableIntStateOf(0) }
    Column(Modifier.fillMaxSize().imePadding().navigationBarsPadding()) {
        TopBar(onBack = actions.back) {
            if (!state.isNew) ChromeButton(Icons.Rounded.DeleteOutline, "Delete account", actions.requestDelete)
        }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = GUTTER, end = GUTTER, top = 4.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
        ) {
            PageTitle(
                if (state.isNew) "New account" else state.storedName.ifEmpty { "Account" },
                context = if (state.isNew) "Where your money sits, and what it held to start"
                else typeLabel(d.type) + " · " + Copy.plural(state.entryCount, "entry", "entries"),
            )
            if (state.loaded) {
                BalanceHero(state, onEntries = { actions.viewEntries(state.id) })
                if (state.isNew && state.templates.isNotEmpty()) QuickStartGroup(state, actions)
                NameGroup(state, actions)
                TypeGroup(d.type, actions.setType)
                OpeningGroup(state, actions)
                if (!state.isNew && d.type == AccountType.INVESTMENT) ValueGroup(state, actions)
                UseGroup(d, actions)
                Column(Modifier.fillMaxWidth()) {
                    SaveRefusal(
                        if (state.showErrors) refusalLine(listOf(state.problems.name, state.problems.opening)) else null,
                        saveAttempts,
                    )
                    HeroAction("Save account", { saveAttempts++; actions.save() }, Modifier.fillMaxWidth())
                }
            }
        }
    }
    val use = state.confirmDelete
    if (use != null) {
        DeleteDialog(
            state.storedName,
            use,
            canArchive = state.canArchive,
            onConfirm = actions.delete,
            onArchive = actions.archiveInstead,
            onDismiss = actions.dismissDelete,
        )
    }
}

/**
 * What the account will read once saved, live as the opening balance changes: the figure, its
 * share of what you hold (or owe) across the active accounts, and chips for its standing.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BalanceHero(state: AccountEditState, onEntries: () -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val owing = state.balance < 0
    val percent = Math.round(state.share.share * 100f)
    val shareText = when {
        d.archived -> "Archived accounts sit outside the net"
        state.balance > 0 -> "$percent% of the ${money.formatWhole(state.share.total)} you hold"
        owing -> "$percent% of the ${money.formatWhole(state.share.total)} you owe"
        else -> "Holds nothing yet"
    }
    HeroPanel(modifier) {
        HeroHead(
            if (state.isNew) "Opens with" else "Balance",
            end = money.currency.currencyCode,
        )
        Spacer(Modifier.height(14.dp))
        // Not a live region: the figure follows a text field, and TalkBack already echoes each key.
        HeroNumber(
            money.format(state.balance),
            description = (if (state.isNew) "Opens with " else "Balance, ") + money.format(state.balance),
        )
        Text(
            d.name.trim().ifEmpty { "Name it below" } + " · " + typeLabel(d.type),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            state.share.share,
            null,
            shareText,
            height = 8.dp,
            fill = if (owing) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(8.dp))
        Caption(shareText)
        Spacer(Modifier.height(14.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (!state.isNew) StatChip(Icons.AutoMirrored.Rounded.ReceiptLong, Copy.plural(state.entryCount, "entry", "entries"))
            if (!state.isNew) StatChip(Icons.Rounded.Flag, "Opened at " + money.format(d.signedOpening ?: 0L))
            if (d.effectiveDefault) StatChip(Icons.Rounded.Star, "Default for new entries")
            if (d.archived) StatChip(Icons.Rounded.Inventory2, "Archived")
        }
        if (!state.isNew) {
            Spacer(Modifier.height(4.dp))
            TextAction("view entries", onEntries, Modifier.align(Alignment.End), color = MaterialTheme.colorScheme.primary)
        }
    }
}

/** A new account in one tap: the banks the owner uses, each with its usual type. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun QuickStartGroup(state: AccountEditState, actions: AccountEditActions) {
    val block: @Composable (Shape) -> Unit = { shape ->
        GroupBlock(shape) {
            FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                state.templates.forEach { t ->
                    ChoiceChip(t.name, state.draft.name == t.name && state.draft.type == t.type, role = Role.Button) { actions.applyTemplate(t) }
                }
            }
        }
    }
    Group(
        rows = listOf(block),
        title = "Quick start",
        footer = "Fills the name and the type. Import its statement from Settings once it exists",
    )
}

/**
 * What an investment account is worth: the value read off a statement today, and the ones
 * recorded before. The newest value is the balance from its day on; a tap on an older one removes
 * it, with Undo.
 */
@Composable
private fun ValueGroup(state: AccountEditState, actions: AccountEditActions) {
    val money = LocalMoney.current
    var text by rememberSaveable { mutableStateOf("") }
    val value = money.parse(text)
    val field: @Composable (Shape) -> Unit = { shape ->
        FieldRow(
            text,
            { text = it },
            "Worth today",
            Icons.Rounded.Payments,
            shape,
            keyboardType = KeyboardType.Decimal,
            suffix = money.currency.currencyCode,
        )
    }
    val record: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            if (value != null) "Record " + money.format(value) else "Record the value",
            shape,
            subtitle = "Market moves land in the balance, never in spending or income",
            leading = { GlyphBadge(Icons.Rounded.Check) },
            chevron = false,
            onClick = if (value != null) ({ actions.recordValue(value); text = "" }) else null,
        )
    }
    val history: List<@Composable (Shape) -> Unit> = state.values.take(4).map { v -> valueRow(v, state.today, actions.deleteValue) }
    Group(
        rows = listOf(field, record) + history,
        title = "Value",
        footer = if (state.values.isEmpty()) "Until a value is recorded, the balance counts what went in" else "Tap an older value to remove it",
    )
}

private fun valueRow(v: AccountValueEntity, today: LocalDate, onDelete: (Long) -> Unit): @Composable (Shape) -> Unit = { shape ->
    val money = LocalMoney.current
    GroupRow(
        money.format(v.value),
        shape,
        subtitle = "Read " + Dates.day(v.date, today),
        leading = { GlyphBadge(Icons.Rounded.History) },
        chevron = false,
        onClick = { onDelete(v.id) },
    )
}

@Composable
private fun NameGroup(state: AccountEditState, actions: AccountEditActions) {
    val problem = state.problems.name
    val showProblem = state.showErrors && problem != null
    val nameRow: @Composable (Shape) -> Unit = { shape ->
        FieldRow(
            state.draft.name,
            actions.setName,
            "Account name (Visa, Chequing)",
            Icons.Rounded.EditNote,
            shape,
            capitalization = KeyboardCapitalization.Words,
            isError = showProblem,
        )
    }
    Group(rows = listOf(nameRow), title = "Name", footer = if (showProblem) problem else null, footerIsError = true)
}

/** The four types as one radio group of slabs, each with its badge and what it is for. */
@Composable
private fun TypeGroup(selected: AccountType, onPick: (AccountType) -> Unit) {
    val rows = AccountType.entries.map { type ->
        val row: @Composable (Shape) -> Unit = { shape -> TypeRow(type, type == selected, shape) { onPick(type) } }
        row
    }
    Group(rows = rows, modifier = Modifier.selectableGroup(), title = "Type")
}

@Composable
private fun TypeRow(type: AccountType, selected: Boolean, shape: Shape, onClick: () -> Unit) {
    val (border, fill) = selectableColors(selected)
    val label = typeLabel(type)
    Row(
        Modifier
            .slab(shape)
            .background(fill)
            .border(1.5.dp, border, shape)
            .bounceClick(label = label, role = Role.RadioButton, onClick = onClick)
            .semantics { this.selected = selected }
            .heightIn(min = 64.dp)
            .padding(horizontal = ROW_PAD, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        AccountBadge(type, size = 40.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            Text(typeHint(type), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (selected) {
            Icon(Icons.Rounded.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        }
    }
}

/** The opening amount; a card (or anything opening below zero) also says whether it is owed. */
@Composable
private fun OpeningGroup(state: AccountEditState, actions: AccountEditActions) {
    val money = LocalMoney.current
    val d = state.draft
    val problem = state.problems.opening
    val showProblem = state.showErrors && problem != null
    val amountRow: @Composable (Shape) -> Unit = { shape ->
        FieldRow(
            d.openingText,
            { text -> actions.setOpening(text, money.parse(text)) },
            "Opening balance",
            Icons.Rounded.Flag,
            shape,
            keyboardType = KeyboardType.Decimal,
            suffix = money.currency.currencyCode,
            isError = showProblem,
        )
    }
    val oweRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            "You owe this",
            d.owe,
            actions.setOwe,
            shape,
            subtitle = "Kept as a balance below zero",
        )
    }
    Group(
        rows = if (d.showsOwe) listOf(amountRow, oweRow) else listOf(amountRow),
        title = "Opening balance",
        footer = if (showProblem) problem else "What the account held before your first entry here",
        footerIsError = showProblem,
    )
}

@Composable
private fun UseGroup(d: AccountDraft, actions: AccountEditActions) {
    val defaultRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            "Default for new entries",
            d.effectiveDefault,
            actions.setDefault,
            shape,
            subtitle = if (d.archived) "An archived account cannot be the default" else "New entries start in this account",
            enabled = !d.archived,
        )
    }
    val archivedRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            "Archived",
            d.archived,
            actions.setArchived,
            shape,
            subtitle = "Hidden from pickers, kept in history",
        )
    }
    Group(rows = listOf(defaultRow, archivedRow), title = "Use")
}

/**
 * The one confirm in this package: deleting an account takes its entries and bills and moves the
 * balance of every account it has transfers with, and cannot be undone. With anything to keep,
 * archiving is offered beside it.
 */
@Composable
private fun DeleteDialog(
    name: String,
    use: AccountUse,
    canArchive: Boolean,
    onConfirm: () -> Unit,
    onArchive: () -> Unit,
    onDismiss: () -> Unit,
) {
    val money = LocalMoney.current
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onConfirm, colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error)) {
                Text("Delete account")
            }
        },
        dismissButton = {
            if (canArchive) TextButton(onClick = onArchive) { Text("Archive instead") }
            TextButton(onClick = onDismiss) { Text("Keep it") }
        },
        icon = { Icon(Icons.Rounded.DeleteOutline, contentDescription = null) },
        title = { Text("Delete account") },
        text = {
            // A long list of other accounts can outgrow a short screen; the dialog's text scrolls.
            Text(
                deleteAccountLine(name, use, money, canArchive),
                modifier = Modifier.verticalScroll(rememberScrollState()),
            )
        },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    )
}
