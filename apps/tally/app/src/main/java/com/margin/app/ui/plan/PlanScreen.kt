package com.margin.app.ui.plan

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountBalanceWallet
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.DonutLarge
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Event
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.MoneyOff
import androidx.compose.material.icons.rounded.PauseCircleOutline
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.margin.app.ui.common.CategoryBadge
import com.margin.app.ui.common.Dates
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.GlyphBadge
import com.margin.app.ui.common.HeroAction
import com.margin.app.ui.common.HeroPanel
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PANEL_GAP
import com.margin.app.ui.common.PaceMeter
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.Panel
import com.margin.app.ui.common.PanelHeader
import com.margin.app.ui.common.RowPill
import com.margin.app.ui.common.SecondaryAction
import com.margin.app.ui.common.SlidingSegments
import com.margin.app.ui.common.StackedBar
import com.margin.app.ui.common.StatChip
import com.margin.app.ui.common.StatTile
import com.margin.app.ui.common.TopBar
import com.margin.app.ui.common.TransferBadge
import com.margin.app.ui.common.bounceClick
import com.margin.app.ui.nav.AppNav
import com.margin.app.ui.nav.FAB_CLEARANCE
import com.margin.app.ui.theme.categoryColor
import com.margin.core.Copy
import com.margin.core.PaceStatus
import com.margin.core.TxType

private val LENSES = listOf("Budgets", "Bills", "Goals")

private const val LENS_BUDGETS = 0
private const val LENS_BILLS = 1
private const val LENS_GOALS = 2

@Composable
fun PlanTab(nav: AppNav) {
    val viewModel: PlanViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(viewModel) {
        viewModel.onResume()
        onPauseOrDispose { }
    }
    var lens by rememberSaveable { mutableIntStateOf(LENS_BUDGETS) }
    val actions = remember(nav) {
        PlanActions(
            openBudget = { nav.budgetEdit(it) },
            openBill = { nav.billEdit(it) },
            addBill = { nav.billEdit(0) },
            openGoal = { nav.goal(it) },
            addGoal = { nav.goalEdit(0) },
        )
    }
    PlanScreen(state, actions, lens = lens, onLens = { lens = it })
}

/** Everything Plan can do, as plain lambdas, so the screen renders in a test with no graph. */
data class PlanActions(
    /** 0 opens the overall monthly budget. */
    val openBudget: (Long) -> Unit = {},
    val openBill: (Long) -> Unit = {},
    val addBill: () -> Unit = {},
    val openGoal: (Long) -> Unit = {},
    val addGoal: () -> Unit = {},
)

/**
 * Plan: the month's envelopes, the bills that post themselves, and what is being saved for, one
 * lens at a time. Each lens opens on its lit hero reading, then its stat tiles, then its panels.
 */
@Composable
fun PlanScreen(state: PlanState, actions: PlanActions, lens: Int = 0, onLens: (Int) -> Unit = {}) {
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = FAB_CLEARANCE),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = null)
                PageTitle(
                    "Plan",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = Dates.period(state.period, state.today) + " · " + Copy.plural(state.daysLeft, "day") + " left",
                )
            }
        }
        item(key = "lens") {
            SlidingSegments(LENSES, lens, onLens, Modifier.padding(horizontal = GUTTER).fillMaxWidth())
        }
        if (state.loaded) {
            when (lens) {
                LENS_BILLS -> billsLens(state, actions)
                LENS_GOALS -> goalsLens(state, actions)
                else -> budgetsLens(state, actions)
            }
        }
    }
}

/** Two stat tiles side by side; stacked past 1.5x font so their figures never wrap mid-number. */
@Composable
internal fun TilePair(modifier: Modifier = Modifier, first: @Composable (Modifier) -> Unit, second: @Composable (Modifier) -> Unit) {
    if (LocalDensity.current.fontScale > 1.5f) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(PANEL_GAP)) {
            first(Modifier.fillMaxWidth())
            second(Modifier.fillMaxWidth())
        }
    } else {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(PANEL_GAP)) {
            first(Modifier.weight(1f).fillMaxHeight())
            second(Modifier.weight(1f).fillMaxHeight())
        }
    }
}

// ── Budgets ──────────────────────────────────────────────────────────────────

private fun LazyListScope.budgetsLens(state: PlanState, actions: PlanActions) {
    item(key = "b-hero") { BudgetHero(state, actions, Modifier.padding(horizontal = GUTTER)) }
    item(key = "b-tiles") { BudgetTiles(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "b-categories") { CategoriesPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
    item(key = "b-unbudgeted") { UnbudgetedPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
}

/**
 * The overall monthly budget under the accent's light: what is left, the meter with today's pace
 * tick, three figures, and chips for the time left. The whole block opens the budget. Without
 * one, the meter sits at zero beside the act that sets it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BudgetHero(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val b = state.budgets
    val r = b.overall
    if (r == null) {
        HeroPanel(modifier) {
            HeroLabel(Icons.Rounded.AccountBalanceWallet, "Monthly budget")
            Spacer(Modifier.height(14.dp))
            HeroFigure(money.formatWhole(b.spent), description = "Spent ${money.formatWhole(b.spent)} so far")
            Text(
                "spent so far in " + Dates.period(state.period, state.today) + ", with no limit to read it against",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            PaceMeter(0f, null, "No monthly budget set", height = 14.dp)
            Spacer(Modifier.height(10.dp))
            EmptyLine(Icons.Rounded.DonutLarge, "No monthly budget")
            Spacer(Modifier.height(12.dp))
            HeroAction("Set monthly budget", { actions.openBudget(0L) }, Modifier.fillMaxWidth())
        }
        return
    }
    val over = r.status == PaceStatus.OVER_BUDGET
    val tint = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    val pace = Copy.paceLine(r, money)
    HeroPanel(modifier.bounceClick(label = "Change monthly budget") { actions.openBudget(0L) }, tint = tint) {
        HeroLabel(
            Icons.Rounded.AccountBalanceWallet,
            if (over) "Over budget" else "Monthly budget",
            end = pace.takeIf { !over && it.isNotEmpty() },
            tint = tint,
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(if (over) -r.remaining else r.remaining))
        Text(
            (if (over) "over your " else "left of your ") + money.formatWhole(r.budget) + " budget",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            r.spentFraction,
            r.paceFraction,
            "Spent ${money.formatWhole(r.spent)} of ${money.formatWhole(r.budget)}. " +
                "An even pace would be ${money.formatWhole(r.expected)} by today.",
            height = 14.dp,
        )
        Spacer(Modifier.height(16.dp))
        FigureRow(
            listOf(
                FigureCell("Spent", money.formatWhole(r.spent)),
                FigureCell("Even pace", money.formatWhole(r.expected)),
                FigureCell("Last month", money.formatWhole(b.lastPeriodSpent)),
            )
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(r.daysLeft, "day") + " left")
            if (!over) StatChip(Icons.Rounded.Speed, money.formatWhole(r.dailyAllowance) + " a day")
            StatChip(Icons.Rounded.Event, "Resets " + Dates.short(state.period.endExclusive, state.today))
        }
    }
}

/** What the category budgets cover, and what was spent outside them. */
@Composable
private fun BudgetTiles(state: PlanState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val b = state.budgets
    val overall = b.overall
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                Icons.Rounded.Category,
                "Budgeted",
                money.formatWhole(b.budgeted),
                m,
                detail = when {
                    b.envelopes.isEmpty() -> "No category budgets yet"
                    overall != null -> "In " + Copy.plural(b.envelopes.size, "category", "categories") +
                        ", of " + money.formatWhole(overall.budget)
                    else -> "In " + Copy.plural(b.envelopes.size, "category", "categories")
                },
            )
        },
        second = { m ->
            StatTile(
                Icons.Rounded.MoneyOff,
                "Unbudgeted",
                money.formatWhole(b.unbudgetedSpent),
                m,
                detail = when {
                    b.unbudgeted.isNotEmpty() -> "Spent in " + Copy.plural(b.unbudgeted.size, "category", "categories") + " without a budget"
                    b.spent > 0L -> "Every category you spent in has a budget"
                    else -> "Nothing spent yet this month"
                },
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
    )
}

@Composable
private fun CategoriesPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val b = state.budgets
    Panel(modifier) {
        PanelHeader(
            Icons.Rounded.DonutLarge,
            "Categories",
            meta = if (b.envelopes.isEmpty()) null else b.envelopes.size.toString(),
        )
        if (b.envelopes.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            val suggested = b.suggestedCategoryId
            EmptyLine(
                Icons.Rounded.Category,
                "No category budgets yet",
                action = if (suggested != null) "add one" else null,
                onAction = if (suggested != null) ({ actions.openBudget(suggested) }) else null,
            )
        } else {
            Spacer(Modifier.height(6.dp))
            val overall = b.overall
            Text(
                money.formatWhole(b.budgeted) + (if (overall != null) " of " + money.formatWhole(overall.budget) else "") + " budgeted",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            b.envelopes.forEach { e -> EnvelopeRow(e) { actions.openBudget(e.categoryId) } }
        }
    }
}

@Composable
private fun EnvelopeRow(e: BudgetLine, onClick: () -> Unit) {
    val money = LocalMoney.current
    val r = e.reading
    val reading = Copy.ofBudget(r.spent, r.budget, money)
    val pace = Copy.paceLine(r, money)
    Column(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Change ${e.name} budget", onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CategoryBadge(e.icon, e.color, size = 36.dp)
            Column(Modifier.weight(1f)) {
                Text(e.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                Text(
                    pace,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (r.status == PaceStatus.OVER_BUDGET) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
        }
        Spacer(Modifier.height(10.dp))
        PaceMeter(r.spentFraction, r.paceFraction, "${e.name}: $reading. $pace", height = 6.dp)
    }
}

@Composable
private fun UnbudgetedPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val b = state.budgets
    Panel(modifier) {
        PanelHeader(
            Icons.Rounded.MoneyOff,
            "Unbudgeted",
            meta = if (b.unbudgeted.isEmpty()) null else b.unbudgeted.size.toString(),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (b.unbudgeted.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine(
                Icons.Rounded.DonutLarge,
                if (b.spent > 0L) "Every category you spent in has a budget"
                else "Spending in a category with no budget shows here",
            )
        } else {
            Spacer(Modifier.height(4.dp))
            b.unbudgeted.forEach { u -> UnbudgetedRow(u) { actions.openBudget(u.categoryId) } }
        }
    }
}

@Composable
private fun UnbudgetedRow(u: UnbudgetedLine, onClick: () -> Unit) {
    val money = LocalMoney.current
    val spent = money.formatWhole(u.spent)
    val meta = Copy.plural(u.count, "entry", "entries") + " this month"
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Set a ${u.name} budget", onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "${u.name}, $spent spent, $meta, no budget" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CategoryBadge(u.icon, u.color, size = 36.dp)
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(u.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(spent, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.clearAndSetSemantics { })
        RowPill("Set", Modifier.clearAndSetSemantics { })
    }
}

// ── Bills ────────────────────────────────────────────────────────────────────

private fun LazyListScope.billsLens(state: PlanState, actions: PlanActions) {
    item(key = "c-hero") { BillsHero(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "c-tiles") { BillTiles(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "c-active") { ActiveBillsPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
    if (state.bills.paused.isNotEmpty()) {
        item(key = "c-paused") { PausedBillsPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
    }
    item(key = "c-add") {
        SecondaryAction("Add bill", actions.addBill, Modifier.padding(horizontal = GUTTER).fillMaxWidth())
    }
}

/**
 * The bills' monthly cost as the figure, the cost split by bill as one coloured bar (each bill in
 * its category's hue), and the income that repeats when there is some.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BillsHero(state: PlanState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val r = state.bills
    val expenseCount = r.costShares.size
    val shares = r.costShares
    val segments = remember(shares) { shares.map { it.perMonth.toFloat() to categoryColor(it.color) } }
    val description = if (shares.isEmpty()) {
        "No bills cost anything yet"
    } else {
        "Monthly cost ${money.formatWhole(r.monthlyCost)}: " +
            shares.take(4).joinToString(", ") { it.name + " " + money.formatWhole(it.perMonth) }
    }
    HeroPanel(modifier) {
        HeroLabel(
            Icons.Rounded.Repeat,
            "Monthly cost",
            end = if (r.active.isEmpty()) null else Copy.plural(r.active.size, "active bill"),
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(r.monthlyCost))
        Text(
            if (expenseCount == 0) "Nothing set to repeat yet"
            else "a month across " + Copy.plural(expenseCount, "bill") + ", averaged over the year",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        StackedBar(segments, description, height = 14.dp)
        if (r.monthlyIncome > 0L) {
            Spacer(Modifier.height(16.dp))
            FigureRow(
                listOf(
                    FigureCell("Monthly income", "+" + money.formatWhole(r.monthlyIncome), MaterialTheme.colorScheme.tertiary),
                    FigureCell("Left after bills", money.formatWhole(r.monthlyIncome - r.monthlyCost)),
                )
            )
        }
        val next = r.active.firstOrNull()
        if (next != null || r.reminders > 0) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (next != null) StatChip(Icons.Rounded.Event, next.name + " · " + Copy.dueLine(next.daysUntil))
                if (r.reminders > 0) StatChip(Icons.Rounded.EditNote, Copy.plural(r.reminders, "reminder") + " you log yourself")
            }
        }
    }
}

/** What the bills take in the next week and the next month. */
@Composable
private fun BillTiles(state: PlanState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val r = state.bills
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                Icons.Rounded.CalendarToday,
                "Next 7 days",
                money.formatWhole(r.week.total),
                m,
                detail = if (r.week.payments == 0) "Nothing due" else Copy.plural(r.week.payments, "payment") + " due",
            )
        },
        second = { m ->
            StatTile(
                Icons.Rounded.CalendarMonth,
                "Next 30 days",
                money.formatWhole(r.month.total),
                m,
                detail = if (r.month.payments == 0) "Nothing due" else Copy.plural(r.month.payments, "payment") + " due",
            )
        },
    )
}

@Composable
private fun ActiveBillsPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val bills = state.bills.active
    Panel(modifier) {
        PanelHeader(Icons.Rounded.Event, "Upcoming", meta = if (bills.isEmpty()) null else bills.size.toString())
        if (bills.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine(Icons.Rounded.Repeat, "Add rent, phone and subscriptions once, they post on their own")
        } else {
            Spacer(Modifier.height(4.dp))
            bills.forEach { b -> BillRow(b, paused = false) { actions.openBill(b.id) } }
        }
    }
}

@Composable
private fun PausedBillsPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val bills = state.bills.paused
    Panel(modifier) {
        PanelHeader(
            Icons.Rounded.PauseCircleOutline,
            "Paused",
            meta = bills.size.toString(),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(4.dp))
        bills.forEach { b -> BillRow(b, paused = true) { actions.openBill(b.id) } }
    }
}

/** One bill: its badge, when it is due and how often, the account, and the amount. Paused bills read muted. */
@Composable
private fun BillRow(b: BillLine, paused: Boolean, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = if (b.type == TxType.INCOME) money.formatSigned(b.amount) else money.format(b.amount)
    val where = if (b.type == TxType.TRANSFER) b.accountName + " to " + b.toAccountName.orEmpty() else b.accountName
    val meta = listOf(
        if (paused) "Paused" else Copy.dueLine(b.daysUntil),
        frequencyLabel(b.frequency, b.interval),
        where,
    ).joinToString(" · ")
    val reminder = !b.autoPost
    val titleColor = if (paused) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Edit ${b.name}", onClick = onClick)
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull(b.name, amount, meta, if (reminder) "reminder, you log it yourself" else null).joinToString(", ")
            }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        val badgeModifier = if (paused) Modifier.alpha(0.5f) else Modifier
        if (b.type == TxType.TRANSFER) TransferBadge(badgeModifier) else CategoryBadge(b.icon, b.color, badgeModifier)
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(b.name, style = MaterialTheme.typography.bodyLarge, color = titleColor)
            Text(meta, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Column(
            Modifier.clearAndSetSemantics { },
            horizontalAlignment = Alignment.End,
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                amount,
                style = MaterialTheme.typography.titleMedium,
                color = if (paused || b.type == TxType.TRANSFER) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground,
                textAlign = TextAlign.End,
            )
            if (reminder) {
                Text("REMINDER", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

// ── Goals ────────────────────────────────────────────────────────────────────

private fun LazyListScope.goalsLens(state: PlanState, actions: PlanActions) {
    item(key = "g-hero") { GoalsHero(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "g-list") { GoalsPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
    item(key = "g-add") {
        SecondaryAction("Add goal", actions.addGoal, Modifier.padding(horizontal = GUTTER).fillMaxWidth())
    }
}

/** Everything set aside across the goals, against everything they aim for. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GoalsHero(state: PlanState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val g = state.goals
    HeroPanel(modifier) {
        HeroLabel(
            Icons.Rounded.Savings,
            "Saved",
            end = if (g.goals.isEmpty()) null else Copy.plural(g.goals.size, "goal"),
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(g.saved))
        Text(
            if (g.goals.isEmpty()) "Nothing set aside yet"
            else "of " + money.formatWhole(g.target) + " across " + Copy.plural(g.goals.size, "goal"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            g.fraction,
            null,
            if (g.goals.isEmpty()) "No goals yet" else "Saved ${money.formatWhole(g.saved)} of ${money.formatWhole(g.target)}",
            height = 14.dp,
        )
        if (g.goals.isNotEmpty()) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                StatChip(Icons.Rounded.Flag, money.formatWhole(g.left) + " to go")
                if (g.reached > 0) StatChip(Icons.Rounded.EmojiEvents, Copy.plural(g.reached, "goal") + " reached")
                val date = g.nextDate
                if (date != null) {
                    StatChip(Icons.Rounded.CalendarToday, (g.nextDateName ?: "Next") + " by " + Dates.short(date, state.today))
                }
            }
        }
    }
}

@Composable
private fun GoalsPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val goals = state.goals.goals
    Panel(modifier) {
        PanelHeader(Icons.Rounded.Flag, "Goals", meta = if (goals.isEmpty()) null else goals.size.toString())
        if (goals.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine(Icons.Rounded.Savings, "Name something you are saving for")
        } else {
            goals.forEach { g -> GoalRow(g, state) { actions.openGoal(g.id) } }
        }
    }
}

@Composable
private fun GoalRow(g: GoalLine, state: PlanState, onClick: () -> Unit) {
    val money = LocalMoney.current
    val hue = categoryColor(g.color)
    val reading = money.formatWhole(g.saved) + " of " + money.formatWhole(g.target)
    val line = Copy.goalLine(g.saved, g.target, g.monthsLeft, money) +
        (g.targetDate?.let { " · by " + Dates.short(it, state.today) } ?: "")
    Column(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Open ${g.name}", onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphBadge(
                if (g.reached) Icons.Rounded.EmojiEvents else Icons.Rounded.Flag,
                tint = hue,
                fill = hue.copy(alpha = 0.15f),
                size = 36.dp,
            )
            Column(Modifier.weight(1f)) {
                Text(g.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                Text(line, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
        }
        Spacer(Modifier.height(10.dp))
        PaceMeter(g.fraction, null, "${g.name}: $reading. $line", height = 6.dp, fill = hue)
    }
}
