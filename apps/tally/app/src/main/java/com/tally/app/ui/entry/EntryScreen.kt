package com.tally.app.ui.entry

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.DonutLarge
import androidx.compose.material.icons.rounded.LibraryAdd
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.QuickPick
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChoiceRow
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupHeader
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.SwitchRow
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.core.CategoryKind
import com.tally.core.Copy
import com.tally.core.Frequency
import com.tally.core.TxType
import java.time.LocalDate

private val TYPES = listOf(TxType.EXPENSE, TxType.INCOME, TxType.TRANSFER)
private val TYPE_LABELS = TYPES.map { typeLabel(it) }
private val FREQUENCIES = listOf(Frequency.WEEKLY, Frequency.MONTHLY, Frequency.YEARLY)
private val FREQUENCY_LABELS = listOf("Weekly", "Monthly", "Yearly")

/** Below this height (a small phone, landscape) the amount scrolls with the form instead of pinning. */
private val PIN_MIN_HEIGHT = 680.dp

/** The display figure stops growing here, as Avex's hero does, so a 200% font keeps it on screen. */
private const val HERO_MAX_SCALE = 1.3f

@Composable
fun EntryRoute(nav: AppNav) {
    val viewModel: EntryViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        EntryActions(
            back = nav::back,
            delete = viewModel::delete,
            duplicate = viewModel::duplicate,
            setType = viewModel::setType,
            press = viewModel::press,
            pickCategory = viewModel::pickCategory,
            pickAccount = viewModel::pickAccount,
            pickToAccount = viewModel::pickToAccount,
            setDate = viewModel::setDate,
            setNote = viewModel::setNote,
            pickSuggestion = viewModel::pickSuggestion,
            setRepeat = viewModel::setRepeat,
            setFrequency = viewModel::setFrequency,
            pickQuick = viewModel::pickQuick,
            save = viewModel::save,
            saveAndNew = viewModel::saveAndNew,
            editCategories = nav::categories,
            addCategory = { kind -> nav.categoryEdit(kind = kind) },
            addAccount = { nav.accountEdit(0) },
        )
    }
    EntryScreen(state, actions)
}

/** Everything the editor can do, as plain lambdas, so the screen renders in a test with no graph. */
data class EntryActions(
    val back: () -> Unit = {},
    val delete: () -> Unit = {},
    val duplicate: () -> Unit = {},
    val setType: (TxType) -> Unit = {},
    val press: (KeypadKey) -> Unit = {},
    val pickCategory: (Long) -> Unit = {},
    val pickAccount: (Long) -> Unit = {},
    val pickToAccount: (Long) -> Unit = {},
    val setDate: (LocalDate) -> Unit = {},
    val setNote: (String) -> Unit = {},
    val pickSuggestion: (String) -> Unit = {},
    val setRepeat: (Boolean) -> Unit = {},
    val setFrequency: (Frequency) -> Unit = {},
    val pickQuick: (QuickPick) -> Unit = {},
    val save: () -> Unit = {},
    /** Saves and stays, emptied, for the next entry. New entries only. */
    val saveAndNew: () -> Unit = {},
    val editCategories: () -> Unit = {},
    val addCategory: (CategoryKind) -> Unit = {},
    val addAccount: () -> Unit = {},
)

/**
 * The entry editor, built for the two-second log: the amount in the one lit panel at the top,
 * the keypad and the save pinned at the bottom under the thumb, and between them the category
 * grid first, then the account, what the entry does to the month and the balance, the date, the
 * note and the repeat. Opening, typing, one tap on a category and Save is the whole act.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EntryScreen(state: EntryState, actions: EntryActions) {
    // While the note's keyboard is up the keypad steps aside; Done brings it back.
    val imeVisible = WindowInsets.isImeVisible
    val fontScale = LocalDensity.current.fontScale
    var picking by rememberSaveable { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        // maxHeight is the whole window, measured outside imePadding, so the keyboard does not
        // shrink it. Pinned under a keyboard the head would leave the note field no height at
        // all, so while it is up the head scrolls with the form. The scroll area grows by what
        // the head gave up, so the field being typed in stays about where it was on screen.
        val pinHead = !imeVisible && maxHeight >= PIN_MIN_HEIGHT && fontScale <= HERO_MAX_SCALE
        Column(Modifier.fillMaxSize().imePadding().navigationBarsPadding()) {
            TopBar(onBack = actions.back) {
                if (!state.isNew) {
                    ChromeButton(Icons.Rounded.ContentCopy, "Duplicate", actions.duplicate)
                    ChromeButton(Icons.Rounded.DeleteOutline, "Delete entry", actions.delete)
                } else if (state.canSave) {
                    // A stack of receipts: save this one and stay for the next.
                    ChromeButton(Icons.Rounded.LibraryAdd, "Save and add another", actions.saveAndNew)
                }
            }
            if (pinHead) {
                EntryHead(state, actions, Modifier.padding(start = GUTTER, end = GUTTER, bottom = 12.dp))
            }
            Column(
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(start = GUTTER, end = GUTTER, top = 2.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
            ) {
                if (!pinHead) EntryHead(state, actions)
                if (state.loaded) EntryBody(state, actions, onPickDate = { picking = true })
            }
            EntryBottom(state, actions, showKeypad = !imeVisible)
        }
    }
    if (picking) {
        EntryDatePicker(state.draft.date, onPick = actions.setDate, onDismiss = { picking = false })
    }
}

@Composable
private fun EntryHead(state: EntryState, actions: EntryActions, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SlidingSegments(
            TYPE_LABELS,
            TYPES.indexOf(state.draft.type),
            { actions.setType(TYPES[it]) },
            Modifier.fillMaxWidth(),
        )
        AmountHero(state)
    }
}

/**
 * The amount: the screen's one serif figure under the accent's light (money in takes the green),
 * what it is for under it, and chips for when and from where, so the save is read before it is
 * made.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AmountHero(state: EntryState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val minor = d.amount.minor
    val formatted = money.format(minor)
    val tint = if (d.type == TxType.INCOME) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary
    val from = state.account
    val to = state.toAccount
    val what = when (d.type) {
        TxType.TRANSFER -> when {
            from != null && to != null -> "${from.name} to ${to.name}"
            from != null -> "From ${from.name}, pick where it goes"
            else -> "Pick two accounts"
        }
        else -> state.category?.name ?: "Pick a category"
    }
    val line = listOf(what, d.note.trim()).filter { it.isNotEmpty() }.joinToString(" · ")
    HeroPanel(modifier) {
        PanelHeader(typeLabel(d.type), meta = money.currency.currencyCode, tint = tint)
        Spacer(Modifier.height(10.dp))
        val density = LocalDensity.current
        val scale = density.fontScale.coerceAtMost(HERO_MAX_SCALE)
        CompositionLocalProvider(LocalDensity provides Density(density.density, scale)) {
            Text(
                formatted,
                style = amountStyle(formatted.length, scale),
                color = if (minor > 0) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics {
                    contentDescription = "Amount, $formatted"
                    liveRegion = LiveRegionMode.Polite
                },
            )
        }
        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(12.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarToday, Dates.day(d.date, state.today))
            if (d.type != TxType.TRANSFER && from != null) StatChip(CategoryIcons.account(from.type), from.name)
            if (state.isNew && d.repeat) {
                StatChip(Icons.Rounded.Repeat, "Repeats " + FREQUENCY_LABELS[FREQUENCIES.indexOf(d.frequency)].lowercase())
            }
            if (!state.isNew && d.recurringId != null) StatChip(Icons.Rounded.Repeat, "Posted by a bill")
            val left = state.leftAfter
            if (left != null && minor > 0) {
                StatChip(
                    Icons.Rounded.Speed,
                    if (left >= 0) "Leaves " + money.formatWhole(left) + " of the month" else money.formatWhole(-left) + " over the month",
                )
            }
        }
    }
}

/** The figure steps down a size as the amount grows, so ten digits stay on one line. */
@Composable
private fun amountStyle(length: Int, scale: Float): TextStyle {
    val type = MaterialTheme.typography
    val width = length * scale
    return when {
        width <= 9.5f -> type.displayLarge
        width <= 12f -> type.displayMedium
        width <= 15f -> type.headlineLarge
        else -> type.headlineMedium
    }
}

/** The form between the amount and the keypad. Each call below is one block of the scroll. */
@Composable
private fun EntryBody(state: EntryState, actions: EntryActions, onPickDate: () -> Unit) {
    val noAccounts = state.accounts.isEmpty()
    if (noAccounts) NoAccountsPanel(state.draft.type, actions.addAccount)
    if (state.isNew && state.quickPicks.isNotEmpty() && !noAccounts) QuickPicksPanel(state, actions)
    if (state.draft.type == TxType.TRANSFER) {
        if (!noAccounts) TransferPanel(state, actions)
    } else {
        CategoryPanel(state, actions)
        if (!noAccounts) AccountPanel(state, actions)
    }
    if (!noAccounts) ReadingTiles(state)
    DetailsSection(state, actions, onPickDate)
    if (state.isNew) RepeatSection(state, actions)
}

/** A zero state: one quiet line, and the act that fills it. */
@Composable
private fun EmptyLine(
    text: String,
    action: String,
    onAction: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Aside(text, Modifier.weight(1f))
        TextAction(action, onAction, color = MaterialTheme.colorScheme.primary)
    }
}

/**
 * The entries logged most often, one tap from filled: note, category and the amount last paid.
 * Shown while the form is empty; the save is still the owner's.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun QuickPicksPanel(state: EntryState, actions: EntryActions) {
    Panel {
        PanelHeader("Quick add", meta = "LOGGED MOST")
        Spacer(Modifier.height(10.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            state.quickPicks.forEach { pick -> QuickChip(pick) { actions.pickQuick(pick) } }
        }
    }
}

@Composable
private fun QuickChip(pick: QuickPick, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = money.format(pick.lastAmount)
    val shape = RoundedCornerShape(14.dp)
    Row(
        Modifier
            .heightIn(min = 48.dp)
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceClick(label = "Fill ${pick.note}, ${pick.categoryName}, $amount", role = Role.Button, focusShape = shape, onClick = onClick)
            .padding(start = 6.dp, end = 14.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        CategoryBadge(pick.categoryIcon, pick.categoryColor, size = 32.dp)
        Column {
            Text(pick.note, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
            Text(amount, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun NoAccountsPanel(type: TxType, onAdd: () -> Unit) {
    Panel {
        PanelHeader(if (type == TxType.TRANSFER) "Between accounts" else "Account")
        Spacer(Modifier.height(8.dp))
        EmptyLine("Add an account first", "add account", onAdd)
    }
}

@Composable
private fun CategoryPanel(state: EntryState, actions: EntryActions) {
    val kind = kindOf(state.draft.type) ?: CategoryKind.EXPENSE
    Panel {
        PanelHeader(
            "Category",
            action = "edit",
            onAction = actions.editCategories,
        )
        Spacer(Modifier.height(10.dp))
        if (state.categories.isEmpty()) {
            EmptyLine(
                if (kind == CategoryKind.INCOME) "No income categories yet" else "No expense categories yet",
                "add one",
                { actions.addCategory(kind) },
            )
        } else {
            CategoryGrid(state.categories, state.draft.categoryId, actions.pickCategory)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AccountChips(accounts: List<AccountBalance>, selected: Long?, onPick: (Long) -> Unit) {
    FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        accounts.forEach { a -> ChoiceChip(a.name, a.id == selected) { onPick(a.id) } }
    }
}

@Composable
private fun AccountPanel(state: EntryState, actions: EntryActions) {
    Panel {
        PanelHeader("Account")
        Spacer(Modifier.height(6.dp))
        AccountChips(state.accounts, state.draft.accountId, actions.pickAccount)
    }
}

@Composable
private fun SideLabel(text: String) {
    Text(text.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun TransferPanel(state: EntryState, actions: EntryActions) {
    Panel {
        PanelHeader("Between accounts")
        Spacer(Modifier.height(10.dp))
        SideLabel("From")
        AccountChips(state.accounts, state.draft.accountId, actions.pickAccount)
        Spacer(Modifier.height(8.dp))
        SideLabel("To")
        AccountChips(state.accounts, state.draft.toAccountId, actions.pickToAccount)
        when {
            state.sameAccount -> {
                Spacer(Modifier.height(6.dp))
                Caption("From and To are the same account. A transfer needs two", color = MaterialTheme.colorScheme.error)
            }
            state.accounts.size < 2 -> {
                Spacer(Modifier.height(8.dp))
                EmptyLine("A transfer needs a second account", "add account", actions.addAccount)
            }
        }
    }
}

/**
 * Two different readings of what this entry does: to its category's month (against the budget
 * when there is one), and to the account's balance. A transfer reads both accounts instead.
 * Side by side, stacked at large font so the figures never wrap mid-number.
 */
@Composable
private fun ReadingTiles(state: EntryState) {
    if (LocalDensity.current.fontScale > 1.5f) {
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(PANEL_GAP)) {
            FirstTile(state, Modifier.fillMaxWidth())
            SecondTile(state, Modifier.fillMaxWidth())
        }
    } else {
        Row(
            Modifier.fillMaxWidth().height(IntrinsicSize.Min),
            horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP),
        ) {
            FirstTile(state, Modifier.weight(1f).fillMaxHeight())
            SecondTile(state, Modifier.weight(1f).fillMaxHeight())
        }
    }
}

@Composable
private fun FirstTile(state: EntryState, modifier: Modifier) {
    if (state.draft.type == TxType.TRANSFER) {
        BalanceTile(state.account, state.accountAfter, "Sends it", "From", "Pick where it comes from", modifier)
    } else {
        MonthTile(state, modifier)
    }
}

@Composable
private fun SecondTile(state: EntryState, modifier: Modifier) {
    if (state.draft.type == TxType.TRANSFER) {
        BalanceTile(state.toAccount, state.toAccountAfter, "Receives it", "To", "Pick where it goes", modifier)
    } else {
        BalanceTile(state.account, state.accountAfter, "After this", "Account", "Pick an account", modifier)
    }
}

/** The picked category's month with this entry in it, against its budget when it has one. */
@Composable
private fun MonthTile(state: EntryState, modifier: Modifier) {
    val money = LocalMoney.current
    val income = state.draft.type == TxType.INCOME
    val icon = if (income) Icons.Rounded.ArrowDownward else Icons.Rounded.DonutLarge
    val label = if (state.today in state.period) "This month" else Dates.period(state.period, state.today)
    val category = state.category
    if (category == null) {
        StatTile(
            label,
            money.formatWhole(0),
            modifier,
            detail = "Pick a category to see its month",
        )
    } else {
        val total = state.month.total
        val budget = state.budget
        val over = budget != null && total > budget
        val detail = when {
            budget == null -> Copy.plural(state.month.count, "entry", "entries") + " in " + category.name
            total > budget -> money.formatWhole(total - budget) + " over the " + money.formatWhole(budget) + " budget"
            else -> money.formatWhole(budget - total) + " left of " + money.formatWhole(budget)
        }
        StatTile(
            label,
            money.formatWhole(total),
            modifier,
            detail = detail,
        )
    }
}

/** One account's balance once this is saved, beside what it holds now. */
@Composable
private fun BalanceTile(
    account: AccountBalance?,
    after: Long?,
    role: String,
    missingLabel: String,
    missingDetail: String,
    modifier: Modifier,
) {
    val money = LocalMoney.current
    if (account == null || after == null) {
        StatTile(
            missingLabel,
            "None yet",
            modifier,
            detail = missingDetail,
        )
    } else {
        StatTile(
            account.name,
            money.format(after),
            modifier,
            detail = role + ", now " + money.format(account.balance),
        )
    }
}

/** When and what: quick days over the date row, then the note and the notes used before. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun DetailsSection(state: EntryState, actions: EntryActions, onPickDate: () -> Unit) {
    val d = state.draft
    val yesterday = state.today.minusDays(1)
    val dateRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Date",
            shape,
            leading = { GlyphBadge(Icons.Rounded.CalendarMonth) },
            trailing = {
                Text(
                    Dates.day(d.date, state.today),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            onClick = onPickDate,
        )
    }
    Column(Modifier.fillMaxWidth()) {
        GroupHeader("Details")
        FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ChoiceChip("Today", d.date == state.today) { actions.setDate(state.today) }
            ChoiceChip("Yesterday", d.date == yesterday) { actions.setDate(yesterday) }
        }
        Spacer(Modifier.height(6.dp))
        Group(rows = listOf(dateRow))
        Spacer(Modifier.height(10.dp))
        NoteField(d.note, actions.setNote)
        if (state.suggestions.isNotEmpty()) {
            Spacer(Modifier.height(6.dp))
            FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // A suggestion fills the note; it is an act, not one of a set of picks.
                state.suggestions.forEach { note -> ChoiceChip(note, false, role = Role.Button) { actions.pickSuggestion(note) } }
            }
        }
    }
}

/** New entries only: the same entry again on a schedule, as a bill that posts itself. */
@Composable
private fun RepeatSection(state: EntryState, actions: EntryActions) {
    val d = state.draft
    val switchRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            title = "Repeat",
            checked = d.repeat,
            onToggle = actions.setRepeat,
            shape = shape,
            subtitle = if (d.repeat) {
                "Posts itself again on " + Dates.short(state.repeatNext, state.today)
            } else {
                "Post this again on a schedule"
            },
        )
    }
    val frequencyRow: @Composable (Shape) -> Unit = { shape ->
        ChoiceRow(
            "How often",
            FREQUENCY_LABELS,
            FREQUENCIES.indexOf(d.frequency),
            { actions.setFrequency(FREQUENCIES[it]) },
            shape,
        )
    }
    Group(
        rows = if (d.repeat) listOf(switchRow, frequencyRow) else listOf(switchRow),
        title = "Schedule",
        footer = if (d.repeat) "It joins your bills in Plan, where you can change or stop it" else null,
    )
}

/** The keypad and the save, pinned under the thumb. */
@Composable
private fun EntryBottom(state: EntryState, actions: EntryActions, showKeypad: Boolean) {
    val money = LocalMoney.current
    // Said only once there is an amount: an empty figure is its own prompt, and a transfer's
    // same-account clash already reads in its panel.
    val hint = state.problem?.takeIf {
        state.loaded && state.accounts.isNotEmpty() && !state.sameAccount && state.draft.amount.minor > 0
    }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(start = GUTTER, end = GUTTER, top = 10.dp, bottom = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (showKeypad) {
            Keypad(showDecimal = money.fractionDigits > 0, separator = money.decimalSeparator, onKey = actions.press)
        }
        HeroAction(
            "Save " + typeLabel(state.draft.type).lowercase(),
            actions.save,
            Modifier.fillMaxWidth(),
            enabled = state.canSave,
        )
        if (hint != null) Caption(hint, Modifier.fillMaxWidth())
    }
}
