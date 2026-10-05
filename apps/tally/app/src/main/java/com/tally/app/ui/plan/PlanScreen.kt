package com.tally.app.ui.plan

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Event
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
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_FOCUS_OUTSET
import com.tally.app.ui.common.RowPill
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StackedBar
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.TransferBadge
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.FAB_CLEARANCE
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.PaceStatus
import com.tally.core.TxType
import java.time.LocalDate

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
            applySuggestions = viewModel::applySuggestions,
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
    /** Sets every budget the averages suggest. */
    val applySuggestions: () -> Unit = {},
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
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.fillMaxWidth())
            second(Modifier.fillMaxWidth())
        }
    } else {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
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
    if (state.budgets.suggestions.isNotEmpty()) {
        item(key = "b-suggested") { SuggestedPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
    }
    item(key = "b-unbudgeted") { UnbudgetedPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
}

/**
 * The overall monthly budget under the accent's light: what is left, the meter with today's pace
 * tick, three figures, and chips for the allowance and the reset. The whole block opens the
 * budget. Without one, the meter sits at zero beside the act that sets it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BudgetHero(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val b = state.budgets
    val r = b.overall
    if (r == null) {
        HeroPanel(modifier) {
            HeroLabel("Monthly budget")
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
            EmptyLine("No monthly budget")
            Spacer(Modifier.height(12.dp))
            HeroAction("Set monthly budget", { actions.openBudget(0L) }, Modifier.fillMaxWidth())
        }
        return
    }
    val over = r.status == PaceStatus.OVER_BUDGET
    val tint = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    val pace = Copy.paceLine(r, money)
    HeroPanel(
        modifier.bounceClick(label = "Change monthly budget", focusShape = RoundedCornerShape(24.dp)) { actions.openBudget(0L) },
    ) {
        HeroLabel(
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
            // The days left are in the context line under the title; the allowance names them.
            if (!over) StatChip(Icons.Rounded.Speed, Copy.marginLine(r, money))
            val projected = b.projected
            if (projected != null && projected != r.spent) {
                StatChip(Icons.AutoMirrored.Rounded.TrendingUp, "On pace for " + money.formatWhole(projected))
            }
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
                "Unbudgeted",
                money.formatWhole(b.unbudgetedSpent),
                m,
                detail = when {
                    b.unbudgeted.isNotEmpty() -> "Spent in " + Copy.plural(b.unbudgeted.size, "category", "categories") + " without a budget"
                    b.spent > 0L -> "Every category you spent in has a budget"
                    else -> "Nothing spent yet this month"
                },
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
            "Categories",
            meta = if (b.envelopes.isEmpty()) null else b.envelopes.size.toString(),
        )
        if (b.envelopes.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            val suggested = b.suggestedCategoryId
            EmptyLine(
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
            .bounceClick(label = "Change ${e.name} budget", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CategoryBadge(e.icon, e.color, size = 36.dp)
            // The reading shares the name's line until a word of the name would break; then it
            // drops under the name (200% font, a long reading).
            EndsRow(
                start = {
                    Column {
                        Text(e.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                        Text(
                            pace,
                            style = MaterialTheme.typography.bodySmall,
                            color = if (r.status == PaceStatus.OVER_BUDGET) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                end = {
                    Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
                },
                modifier = Modifier.weight(1f),
            )
        }
        Spacer(Modifier.height(10.dp))
        PaceMeter(r.spentFraction, r.paceFraction, "${e.name}: $reading. $pace", height = 6.dp)
    }
}

/**
 * Budgets for the categories that have none, each the average of the last three months it was
 * spent in: one tap sets them all (with an undo), or a row opens that budget to set it by hand.
 */
@Composable
private fun SuggestedPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val b = state.budgets
    Panel(modifier) {
        PanelHeader("Suggested budgets", meta = "3-MONTH AVERAGE")
        Spacer(Modifier.height(6.dp))
        Text(
            money.formatWhole(b.suggestedTotal) + " a month across " + Copy.plural(b.suggestions.size, "category", "categories") +
                ", from what you spent",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(4.dp))
        b.suggestions.forEach { sug -> SuggestionRow(sug) { actions.openBudget(sug.categoryId) } }
        Spacer(Modifier.height(12.dp))
        SecondaryAction(
            if (b.suggestions.size == 1) "Set this budget" else "Set all " + b.suggestions.size,
            actions.applySuggestions,
            Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun SuggestionRow(sug: BudgetSuggestion, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = money.formatWhole(sug.amount)
    val meta = "Spent in " + sug.months + " of the last " + SUGGEST_PERIODS + " months"
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Set a ${sug.name} budget", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "${sug.name}, suggested $amount a month, $meta" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CategoryBadge(sug.icon, sug.color, size = 36.dp)
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(sug.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = {
                Text(amount, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
            },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
        )
    }
}

@Composable
private fun UnbudgetedPanel(state: PlanState, actions: PlanActions, modifier: Modifier = Modifier) {
    val b = state.budgets
    Panel(modifier) {
        PanelHeader(
            "Unbudgeted",
            meta = if (b.unbudgeted.isEmpty()) null else b.unbudgeted.size.toString(),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (b.unbudgeted.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine(
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
            .bounceClick(label = "Set a ${u.name} budget", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "${u.name}, $spent spent, $meta, no budget" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CategoryBadge(u.icon, u.color, size = 36.dp)
        // The amount and the pill travel together, so a stacked row gives the name the full width.
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(u.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(spent, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
                    RowPill("Set")
                }
            },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
        )
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
        if (next != null || r.reminders > 0 || r.monthlyCost > 0L) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (next != null) StatChip(Icons.Rounded.Event, next.name + " · " + Copy.dueLine(next.daysUntil))
                if (r.monthlyCost > 0L) StatChip(Icons.Rounded.CalendarMonth, money.formatWhole(r.monthlyCost * 12) + " a year")
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
                "Next 7 days",
                money.formatWhole(r.week.total),
                m,
                detail = if (r.week.payments == 0) "Nothing due" else Copy.plural(r.week.payments, "payment") + " due",
            )
        },
        second = { m ->
            StatTile(
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
        PanelHeader("Upcoming", meta = if (bills.isEmpty()) null else bills.size.toString())
        if (bills.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine("Add rent, phone and subscriptions once, they post on their own")
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
            .bounceClick(label = "Edit ${b.name}", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
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
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(b.name, style = MaterialTheme.typography.bodyLarge, color = titleColor)
                    Text(meta, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = {
                Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
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
            },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
            gap = 14.dp,
        )
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

/**
 * How the goals stand: how many are on track (or reached) as the figure, the share of them as the
 * meter, and chips for the money in savings pots and the next date. Goals of different kinds do
 * not add up as money, so the figure counts goals.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GoalsHero(state: PlanState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val g = state.goals
    val good = g.onTrack + g.reached
    val total = g.goals.size
    HeroPanel(modifier) {
        HeroLabel(
            "Goals",
            end = if (total == 0) null else Copy.plural(total, "goal").uppercase(),
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(if (total == 0) "0" else "$good of $total", description = if (total == 0) "No goals yet" else "$good of $total goals on track or reached")
        Text(
            when {
                total == 0 -> "Save for something, reach an amount by a date, or invest a share of every month"
                g.behind == 0 -> "on track or reached"
                else -> "on track or reached · " + g.behind + " behind its pace"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            if (total == 0) 0f else good.toFloat() / total,
            null,
            if (total == 0) "No goals yet" else "$good of $total goals on track or reached",
            height = 14.dp,
        )
        if (total > 0) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (g.pots > 0) StatChip(Icons.Rounded.Savings, money.formatWhole(g.saved) + " in " + Copy.plural(g.pots, "pot"))
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
        PanelHeader("Goals", meta = if (goals.isEmpty()) null else goals.size.toString())
        if (goals.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine("Name something you are saving for")
        } else {
            goals.forEach { g -> GoalRow(g, state.today) { actions.openGoal(g.id) } }
        }
    }
}

/**
 * One goal of any kind: its badge in its own hue (a cup once reached), its name over what it needs
 * next, its reading at the end, and its meter with the even-pace tick where it has one.
 */
@Composable
internal fun GoalRow(g: GoalLine, today: LocalDate, onClick: () -> Unit) {
    val money = LocalMoney.current
    val hue = categoryColor(g.color)
    val reading = goalReading(g, money)
    val line = goalRowLine(g, money, today)
    val behind = g.status == GoalStatus.BEHIND
    Column(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Open ${g.name}", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphBadge(
                if (g.reached) Icons.Rounded.EmojiEvents else goalKindIcon(g.kind),
                tint = hue,
                fill = hue.copy(alpha = 0.15f),
                size = 36.dp,
            )
            EndsRow(
                start = {
                    Column {
                        Text(g.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                        Text(line, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
                end = {
                    Column(horizontalAlignment = Alignment.End) {
                        Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
                        if (behind) Text("BEHIND", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
                modifier = Modifier.weight(1f),
            )
        }
        Spacer(Modifier.height(10.dp))
        PaceMeter(
            g.fraction,
            g.paceFraction,
            "${g.name}: $reading, ${goalMeterWords(g.fraction, g.paceFraction)}. $line",
            height = 6.dp,
            fill = hue,
        )
    }
}
