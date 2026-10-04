package com.margin.app.ui.accounts

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
import androidx.compose.material.icons.rounded.Inventory2
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
import androidx.compose.runtime.remember
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
import com.margin.app.ui.common.AccountBadge
import com.margin.app.ui.common.Caption
import com.margin.app.ui.common.CategoryIcons
import com.margin.app.ui.common.ChromeButton
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.Group
import com.margin.app.ui.common.GroupFooter
import com.margin.app.ui.common.HeroAction
import com.margin.app.ui.common.HeroPanel
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PANEL_GAP
import com.margin.app.ui.common.PaceMeter
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.ROW_PAD
import com.margin.app.ui.common.StatChip
import com.margin.app.ui.common.SwitchRow
import com.margin.app.ui.common.TextAction
import com.margin.app.ui.common.TopBar
import com.margin.app.ui.common.bounceClick
import com.margin.app.ui.common.selectableColors
import com.margin.app.ui.common.slab
import com.margin.app.ui.nav.AppNav
import com.margin.core.AccountType
import com.margin.core.Copy

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
            setOpening = viewModel::setOpening,
            setOwe = viewModel::setOwe,
            setDefault = viewModel::setDefault,
            setArchived = viewModel::setArchived,
            save = viewModel::save,
            requestDelete = viewModel::requestDelete,
            dismissDelete = viewModel::dismissDelete,
            delete = viewModel::delete,
            viewEntries = { nav.transactions(accountId = it) },
        )
    }
    AccountEditScreen(state, actions)
}

/** Everything the account editor can do, as plain lambdas. */
data class AccountEditActions(
    val back: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setType: (AccountType) -> Unit = {},
    /** The typed text and that text read as an amount (null when it is not one). */
    val setOpening: (String, Long?) -> Unit = { _, _ -> },
    val setOwe: (Boolean) -> Unit = {},
    val setDefault: (Boolean) -> Unit = {},
    val setArchived: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
    val requestDelete: () -> Unit = {},
    val dismissDelete: () -> Unit = {},
    val delete: () -> Unit = {},
    val viewEntries: (Long) -> Unit = {},
)

/**
 * The account editor: the balance it will read under the light, then the name, the type, what it
 * opened with and how it is used. Deleting names the entries that go with it before it acts.
 */
@Composable
fun AccountEditScreen(state: AccountEditState, actions: AccountEditActions) {
    val d = state.draft
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
                NameGroup(state, actions)
                TypeGroup(d.type, actions.setType)
                OpeningGroup(state, actions)
                UseGroup(d, actions)
                HeroAction("Save account", actions.save, Modifier.fillMaxWidth())
                if (state.showErrors && state.problems.any) {
                    GroupFooter("Fix what is marked above to save", isError = true)
                }
            }
        }
    }
    val count = state.confirmDelete
    if (count != null) {
        DeleteDialog(state.storedName, count, onConfirm = actions.delete, onDismiss = actions.dismissDelete)
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
            CategoryIcons.account(d.type),
            if (state.isNew) "Opens with" else "Balance",
            end = money.currency.currencyCode,
        )
        Spacer(Modifier.height(14.dp))
        HeroNumber(
            money.format(state.balance),
            description = (if (state.isNew) "Opens with " else "Balance, ") + money.format(state.balance),
            live = true,
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
            "I owe this",
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

/** The one confirm in this package: deleting an account takes its entries, and cannot be undone. */
@Composable
private fun DeleteDialog(name: String, entries: Int, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onConfirm, colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error)) {
                Text("Delete account")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Keep it") }
        },
        icon = { Icon(Icons.Rounded.DeleteOutline, contentDescription = null) },
        title = { Text("Delete account") },
        text = { Text(deleteAccountLine(name, entries)) },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    )
}
