package com.tally.app.ui.plan

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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Event
import androidx.compose.material.icons.rounded.PauseCircleOutline
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.db.AccountEntity
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChoiceRow
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupFooter
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.SaveRefusal
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.SwitchRow
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.refusalLine
import com.tally.app.ui.common.slab
import com.tally.app.ui.nav.AppNav
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.TxType
import java.time.LocalDate

private val BILL_TYPES = listOf(TxType.EXPENSE, TxType.INCOME, TxType.TRANSFER)
private val BILL_TYPE_LABELS = listOf("Expense", "Income", "Transfer")
private val BILL_FREQUENCIES = listOf(Frequency.WEEKLY, Frequency.MONTHLY, Frequency.YEARLY)
private val BILL_FREQUENCY_LABELS = listOf("Weekly", "Monthly", "Yearly")

@Composable
fun BillEditRoute(nav: AppNav) {
    val viewModel: BillEditViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        BillEditActions(
            back = nav::back,
            delete = viewModel::delete,
            setName = viewModel::setName,
            setType = viewModel::setType,
            setAmount = viewModel::setAmount,
            pickAccount = viewModel::pickAccount,
            pickToAccount = viewModel::pickToAccount,
            pickCategory = viewModel::pickCategory,
            setFrequency = viewModel::setFrequency,
            stepInterval = viewModel::stepInterval,
            setAnchor = viewModel::setAnchor,
            setAutoPost = viewModel::setAutoPost,
            setActive = viewModel::setActive,
            save = viewModel::save,
            addAccount = { nav.accountEdit(0) },
            addCategory = { kind -> nav.categoryEdit(kind = kind) },
        )
    }
    BillEditScreen(state, actions)
}

/** Everything the bill editor can do, as plain lambdas. */
data class BillEditActions(
    val back: () -> Unit = {},
    val delete: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setType: (TxType) -> Unit = {},
    val setAmount: (String, Long?) -> Unit = { _, _ -> },
    val pickAccount: (Long) -> Unit = {},
    val pickToAccount: (Long) -> Unit = {},
    val pickCategory: (Long) -> Unit = {},
    val setFrequency: (Frequency) -> Unit = {},
    val stepInterval: (Int) -> Unit = {},
    val setAnchor: (LocalDate) -> Unit = {},
    val setAutoPost: (Boolean) -> Unit = {},
    val setActive: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
    val addAccount: () -> Unit = {},
    val addCategory: (CategoryKind) -> Unit = {},
)

/**
 * The bill editor: the amount and its rhythm in the lit panel at the top, then what it is, where
 * it goes, how often, and whether it posts itself, each in its own block. The next three dates it
 * would post on read under the schedule before it is saved.
 */
@Composable
fun BillEditScreen(state: BillEditState, actions: BillEditActions) {
    var picking by rememberSaveable { mutableStateOf(false) }
    var saveAttempts by rememberSaveable { mutableIntStateOf(0) }
    val d = state.draft
    Column(Modifier.fillMaxSize().imePadding().navigationBarsPadding()) {
        TopBar(onBack = actions.back) {
            if (!state.isNew) ChromeButton(Icons.Rounded.DeleteOutline, "Delete bill", actions.delete)
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
                if (state.isNew) "New bill" else "Edit bill",
                context = if (d.autoPost) "Posts itself on each due date" else "A reminder: it shows when due and you log it yourself",
            )
            BillHero(state)
            if (state.loaded) {
                SlidingSegments(
                    BILL_TYPE_LABELS,
                    BILL_TYPES.indexOf(d.type),
                    { actions.setType(BILL_TYPES[it]) },
                    Modifier.fillMaxWidth(),
                )
                DetailsBlock(state, actions)
                if (d.type == TxType.TRANSFER) TransferPanel(state, actions) else AccountPanel(state, actions)
                if (d.type != TxType.TRANSFER) CategoryPanel(state, actions)
                ScheduleGroup(state, actions, onPickDate = { picking = true })
                PostingGroup(state, actions)
                Column(Modifier.fillMaxWidth()) {
                    val p = state.problems
                    SaveRefusal(if (state.showErrors) refusalLine(listOf(p.name, p.amount, p.account)) else null, saveAttempts)
                    HeroAction("Save bill", { saveAttempts++; actions.save() }, Modifier.fillMaxWidth())
                }
            }
        }
    }
    if (picking) {
        PlanDatePicker(d.anchorDate, onPick = actions.setAnchor, onDismiss = { picking = false })
    }
}

/** The amount and its rhythm under the light; money in takes the green. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BillHero(state: BillEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val amount = d.amount ?: 0L
    val formatted = money.format(amount)
    val tint = if (d.type == TxType.INCOME) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary
    val account = state.accountName
    val toAccount = state.toAccountName
    val where = when {
        d.type == TxType.TRANSFER && account != null && toAccount != null -> "$account to $toAccount"
        else -> account
    }
    val line = listOfNotNull(d.name.trim().ifEmpty { "Name it below" }, where).joinToString(" · ")
    HeroPanel(modifier) {
        HeroLabel(
            BILL_TYPE_LABELS[BILL_TYPES.indexOf(d.type)],
            end = frequencyLabel(d.frequency, d.interval).uppercase(),
            tint = tint,
        )
        Spacer(Modifier.height(10.dp))
        // Not a live region: the figure follows a text field, and TalkBack already echoes each key.
        HeroFigure(
            formatted,
            color = if (amount > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            description = "Amount, $formatted",
        )
        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(12.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val next = state.preview.firstOrNull()
            if (next != null) StatChip(Icons.Rounded.Event, "Next " + Dates.day(next, state.today))
            if (amount > 0L && !(d.frequency == Frequency.MONTHLY && d.interval == 1)) {
                StatChip(Icons.Rounded.Speed, money.formatWhole(state.perMonth) + " a month")
            }
            if (!d.autoPost) StatChip(Icons.Rounded.EditNote, "Reminder only")
            if (!d.active) StatChip(Icons.Rounded.PauseCircleOutline, "Paused")
        }
    }
}

/** What it is called and how much: two filled fields, each with its reason under it when it is missing. */
@Composable
private fun DetailsBlock(state: BillEditState, actions: BillEditActions) {
    val money = LocalMoney.current
    val d = state.draft
    val errors = state.showErrors
    Column(Modifier.fillMaxWidth()) {
        PlanField(
            d.name,
            actions.setName,
            "Name (rent, phone, salary)",
            Icons.Rounded.EditNote,
            capitalization = KeyboardCapitalization.Sentences,
            isError = errors && state.problems.name != null,
        )
        val nameProblem = state.problems.name
        if (errors && nameProblem != null) GroupFooter(nameProblem, isError = true)
        Spacer(Modifier.height(10.dp))
        PlanField(
            d.amountText,
            { text -> actions.setAmount(text, money.parse(text)) },
            "Amount",
            Icons.Rounded.Payments,
            keyboardType = KeyboardType.Decimal,
            suffix = money.currency.currencyCode,
            isError = errors && state.problems.amount != null,
        )
        val amountProblem = state.problems.amount
        if (errors && amountProblem != null) GroupFooter(amountProblem, isError = true)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AccountChips(accounts: List<AccountEntity>, selected: Long?, onPick: (Long) -> Unit) {
    FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        accounts.forEach { a -> ChoiceChip(a.name, a.id == selected) { onPick(a.id) } }
    }
}

@Composable
private fun AccountPanel(state: BillEditState, actions: BillEditActions) {
    val d = state.draft
    Panel {
        PanelHeader(
            if (d.type == TxType.INCOME) "Paid into" else "Paid from",
        )
        Spacer(Modifier.height(6.dp))
        if (state.accounts.isEmpty()) {
            Spacer(Modifier.height(2.dp))
            EmptyLine("Add an account first", action = "add account", onAction = actions.addAccount)
        } else {
            AccountChips(state.accounts, d.accountId, actions.pickAccount)
        }
        val problem = state.problems.account
        if (state.showErrors && problem != null) {
            Spacer(Modifier.height(4.dp))
            Text(problem, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
private fun TransferPanel(state: BillEditState, actions: BillEditActions) {
    val d = state.draft
    Panel {
        PanelHeader("Between accounts")
        Spacer(Modifier.height(10.dp))
        if (state.accounts.size < 2) {
            EmptyLine("A transfer needs two accounts", action = "add account", onAction = actions.addAccount)
        } else {
            SideLabel("From")
            AccountChips(state.accounts, d.accountId, actions.pickAccount)
            Spacer(Modifier.height(8.dp))
            SideLabel("To")
            AccountChips(state.accounts, d.toAccountId, actions.pickToAccount)
        }
        val problem = state.problems.account
        if ((state.showErrors || (d.accountId != null && d.toAccountId == d.accountId)) && problem != null) {
            Spacer(Modifier.height(4.dp))
            Text(problem, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CategoryPanel(state: BillEditState, actions: BillEditActions) {
    val d = state.draft
    val kind = if (d.type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
    val picked = state.category
    Panel {
        PanelHeader(
            "Category",
            meta = if (picked == null) "OPTIONAL" else null,
        )
        Spacer(Modifier.height(6.dp))
        if (state.categories.isEmpty()) {
            Spacer(Modifier.height(2.dp))
            EmptyLine(
                if (kind == CategoryKind.INCOME) "No income categories yet" else "No expense categories yet",
                action = "add one",
                onAction = { actions.addCategory(kind) },
            )
        } else {
            FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                state.categories.forEach { c -> ChoiceChip(c.name, c.id == d.categoryId) { actions.pickCategory(c.id) } }
            }
        }
    }
}

/** How often, how many between, and from when; the next three dates under it. */
@Composable
private fun ScheduleGroup(state: BillEditState, actions: BillEditActions, onPickDate: () -> Unit) {
    val d = state.draft
    val frequencyRow: @Composable (Shape) -> Unit = { shape ->
        ChoiceRow(
            "How often",
            BILL_FREQUENCY_LABELS,
            BILL_FREQUENCIES.indexOf(d.frequency),
            { actions.setFrequency(BILL_FREQUENCIES[it]) },
            shape,
        )
    }
    val stepperRow: @Composable (Shape) -> Unit = { shape -> IntervalRow(d, actions, shape) }
    val dateRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            if (state.isNew) "Starts" else "First date",
            shape,
            leading = { GlyphBadge(Icons.Rounded.CalendarMonth) },
            trailing = {
                Text(
                    Dates.day(d.anchorDate, state.today),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            onClick = onPickDate,
        )
    }
    Group(
        rows = listOf(frequencyRow, stepperRow, dateRow),
        title = "Schedule",
        footer = if (state.preview.isEmpty()) null else {
            "Next: " + state.preview.joinToString(", ") { Dates.short(it, state.today) }
        },
    )
}

/** "Every 2 weeks" with a minus and a plus, 1 to 12. */
@Composable
private fun IntervalRow(d: BillDraft, actions: BillEditActions, shape: Shape) {
    val unit = when (d.frequency) {
        Frequency.WEEKLY -> "week"
        Frequency.MONTHLY -> "month"
        Frequency.YEARLY -> "year"
    }
    Row(
        Modifier
            .slab(shape)
            .heightIn(min = 56.dp)
            .padding(start = ROW_PAD, end = 10.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) { }) {
            Text("Repeats", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            Text(
                everyLabel(d.frequency, d.interval),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        StepButton(Icons.Rounded.Remove, "Fewer ${unit}s between", enabled = d.interval > 1) { actions.stepInterval(-1) }
        Text(
            d.interval.toString(),
            modifier = Modifier.widthIn(min = 28.dp),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground,
            textAlign = TextAlign.Center,
        )
        StepButton(Icons.Rounded.Add, "More ${unit}s between", enabled = d.interval < MAX_INTERVAL) { actions.stepInterval(1) }
    }
}

/** Whether it writes itself, and, for a stored bill, whether it runs at all. */
@Composable
private fun PostingGroup(state: BillEditState, actions: BillEditActions) {
    val d = state.draft
    val autoRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            title = "Post automatically",
            checked = d.autoPost,
            onToggle = actions.setAutoPost,
            shape = shape,
            subtitle = "Off makes it a reminder that never writes an entry",
        )
    }
    val activeRow: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            title = "Active",
            checked = d.active,
            onToggle = actions.setActive,
            shape = shape,
            subtitle = if (d.active) "Counts in your bills and posts when due" else "Paused: it never posts until you turn it back on",
        )
    }
    Group(
        rows = if (state.isNew) listOf(autoRow) else listOf(autoRow, activeRow),
        title = "Posting",
    )
}
