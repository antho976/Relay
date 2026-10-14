package com.tally.app.ui.plan

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.categories.CategoriesViewModel
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.StackBar
import com.tally.app.ui.kit.T
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy

// ── Budgets ──────────────────────────────────────────────────────────────────

@Composable
fun BudgetsRoute(nav: AppNav) {
    val vm: PlanViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    val categories: CategoriesViewModel = hiltViewModel()
    val cats by categories.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    var adding by rememberSaveable { mutableStateOf(false) }
    BudgetsScreen(
        state,
        edit = { nav.budgetEdit(it) },
        applySuggestions = vm::applySuggestions,
        add = { adding = true },
    )
    if (adding) {
        val budgeted = state.budgets.envelopes.map { it.categoryId }.toSet()
        Sheet(onDismiss = { adding = false }, title = "Budget a category") {
            cats.spending.active.filter { it.id !in budgeted }.forEach { line ->
                PickRow(line.name, false, onClick = { adding = false; nav.budgetEdit(line.id) }, leading = { CategoryBadge(line.icon, line.color, size = 28.dp) })
            }
            if (cats.spending.active.all { it.id in budgeted }) T("Every spending category has a budget.", style = Tally.type.caption, color = Tally.colors.ink3)
        }
    }
}

/**
 * The month's plan for spending: the overall budget read as a pace and a projection, each
 * category's envelope furthest over pace first, what was spent without a budget, and budgets
 * suggested from three months of averages.
 */
@Composable
fun BudgetsScreen(s: PlanState, edit: (Long) -> Unit, applySuggestions: () -> Unit, add: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val b = s.budgets
    SpacePage(
        "Budgets",
        context = Dates.period(s.period, s.today) + " · " + Copy.plural(s.daysLeft, "day") + " left",
        actions = { IconKey("plus", add, label = "Budget a category") },
    ) {
        if (!s.loaded) return@SpacePage
        val overall = b.overall
        Slab(Modifier.fillMaxWidth(), onClick = { edit(0) }, spacing = 8.dp, padding = PaddingValues(16.dp)) {
            if (overall != null) {
                val reached = limitReached(overall)
                Eyebrow(if (reached) "Limit reached" else "Monthly budget", color = if (reached) c.heldText else c.ink3)
                Row(verticalAlignment = Alignment.Bottom) {
                    T(fmt.formatWhole(if (reached) overall.spent else overall.remaining), Modifier.weight(1f), Tally.type.hero, if (reached) c.heldText else c.ink, maxLines = 1)
                    T((if (reached) "spent of " else "left of ") + fmt.formatWhole(overall.budget), Modifier.padding(bottom = 6.dp), Tally.type.ui, c.ink3)
                }
                PaceMeter(overall.spentFraction, overall.paceFraction, height = 8.dp)
                T(
                    if (reached) budgetWords(overall, fmt) + " · " + Copy.plural(s.daysLeft, "day") + " to go" else Copy.daysLine(overall) + " · " + Copy.marginLine(overall, fmt),
                    style = Tally.type.caption,
                    color = paceTone(overall),
                )
                b.projected?.let { T("At this pace the month ends at " + fmt.formatWhole(it), style = Tally.type.caption, color = if (it > overall.budget) c.waiting else c.ink3) }
            } else {
                Eyebrow("Monthly budget")
                T("One number for the whole month, and Tally reads every day against it.", style = Tally.type.ui, color = c.ink2)
                Key("Set a monthly budget", { edit(0) }, kind = KeyKind.Primary, glyph = "pie", compact = true)
            }
        }

        StatGrid(
            listOf(
                { m -> Stat("Spent", fmt.formatWhole(b.spent), m, glyph = "money-out", sub = Copy.versusLastLine(b.spent, b.lastPeriodSpent, fmt)) },
                { m -> Stat("In envelopes", fmt.formatWhole(b.budgeted), m, glyph = "pie", sub = Copy.plural(b.envelopes.size, "category", "categories")) },
                { m -> Stat("Without a budget", fmt.formatWhole(b.unbudgetedSpent), m, glyph = "tag", sub = Copy.plural(b.unbudgeted.size, "category", "categories")) },
                { m -> Stat("Projected", b.projected?.let { fmt.formatWhole(it) } ?: "–", m, glyph = "trend", sub = "by the month's end") },
            ),
        )

        if (b.suggestions.isNotEmpty()) {
            Panel("Suggested from your averages", glyph = "sparkle", action = "Set all " + fmt.formatWhole(b.suggestedTotal), onAction = applySuggestions) {
                b.suggestions.forEach { sug ->
                    ListRow(onClick = { edit(sug.categoryId) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                        CategoryBadge(sug.icon, sug.color)
                        Column(Modifier.weight(1f)) {
                            T(sug.name, style = Tally.type.ui, color = c.ink, maxLines = 1)
                            T("Average of " + Copy.plural(sug.months, "month"), style = Tally.type.caption, color = c.ink3, maxLines = 1)
                        }
                        T(fmt.formatWhole(sug.amount), style = Tally.type.amount, color = c.ink2)
                    }
                }
            }
        }

        if (b.envelopes.isNotEmpty()) {
            Panel("Envelopes", glyph = "pie", subtitle = "Furthest over pace first") {
                b.envelopes.forEach { e -> BudgetRow(e.name, e.icon, e.color, e.reading, onClick = { edit(e.categoryId) }) }
            }
        } else if (b.suggestions.isEmpty()) {
            Slab { Empty("pie", "No category budgets yet", "Give a category its own monthly limit, and its pace shows here and on Overview.") { Key("Budget a category", add, glyph = "plus") } }
        }

        if (b.unbudgeted.isNotEmpty()) {
            Panel("Spent without a budget", glyph = "tag") {
                StackBar(b.unbudgeted.map { categoryColor(it.color) to it.spent }, Modifier.fillMaxWidth())
                b.unbudgeted.forEach { u ->
                    ListRow(onClick = { edit(u.categoryId) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                        CategoryBadge(u.icon, u.color)
                        Column(Modifier.weight(1f)) {
                            T(u.name, style = Tally.type.ui, color = c.ink, maxLines = 1)
                            T(Copy.plural(u.count, "entry", "entries") + " · tap to give it a budget", style = Tally.type.caption, color = c.ink3, maxLines = 1)
                        }
                        T(fmt.formatWhole(u.spent), style = Tally.type.amount, color = c.ink2)
                    }
                }
            }
        }
    }
}

// ── Bills ────────────────────────────────────────────────────────────────────

@Composable
fun BillsRoute(nav: AppNav) {
    val vm: PlanViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    BillsScreen(state, edit = { nav.billEdit(it) }, add = { nav.billEdit() })
}

/** Bills and subscriptions that post themselves: what they cost a month, what is due soon, each one. */
@Composable
fun BillsScreen(s: PlanState, edit: (Long) -> Unit, add: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val r = s.bills
    SpacePage(
        "Bills",
        context = if (r.active.isEmpty()) "Rent, salary, subscriptions: they post on their day" else fmt.formatWhole(r.monthlyCost) + " a month out · " + fmt.formatWhole(r.monthlyIncome) + " in",
        actions = { IconKey("plus", add, label = "New bill") },
    ) {
        if (!s.loaded) return@SpacePage
        if (r.active.isEmpty() && r.paused.isEmpty()) {
            Slab { Empty("repeat", "No bills yet", "A bill posts itself on its due date, or reminds you to, so rent and subscriptions are never logged by hand.") { Key("Add a bill", add, glyph = "plus") } }
            return@SpacePage
        }
        StatGrid(
            listOf(
                { m -> Stat("Due this week", fmt.formatWhole(r.week.total), m, glyph = "calendar", sub = Copy.plural(r.week.payments, "payment")) },
                { m -> Stat("Due this month", fmt.formatWhole(r.month.total), m, glyph = "receipt", sub = Copy.plural(r.month.payments, "payment")) },
            ),
        )
        if (r.costShares.isNotEmpty()) {
            Panel("Where the monthly cost goes", glyph = "pie") {
                StackBar(r.costShares.map { categoryColor(it.color) to it.perMonth }, Modifier.fillMaxWidth())
                Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    r.costShares.take(5).forEach { share ->
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            com.tally.app.ui.kit.Dot(categoryColor(share.color), 8.dp)
                            T(share.name, Modifier.weight(1f), Tally.type.caption, c.ink2, maxLines = 1)
                            T(fmt.formatWhole(share.perMonth) + " a month", style = Tally.type.mono, color = c.ink3)
                        }
                    }
                }
            }
        }
        Panel("Coming up", glyph = "repeat", subtitle = if (r.reminders > 0) Copy.plural(r.reminders, "reminder") + " to log by hand" else null) {
            r.active.forEach { bill ->
                BillRow(
                    bill.name, bill.type, bill.amount, bill.icon, bill.color,
                    line = billLineText(bill.nextDate, bill.daysUntil, everyLabel(bill.frequency, bill.interval), s.today) + " · " + bill.accountName,
                    onClick = { edit(bill.id) },
                    due = bill.daysUntil,
                    autoPost = bill.autoPost,
                )
            }
        }
        if (r.paused.isNotEmpty()) {
            Panel("Paused", glyph = "pause") {
                r.paused.forEach { bill ->
                    BillRow(bill.name, bill.type, bill.amount, bill.icon, bill.color, line = "Paused · " + everyLabel(bill.frequency, bill.interval), onClick = { edit(bill.id) })
                }
            }
        }
    }
}

// ── Goals ────────────────────────────────────────────────────────────────────

@Composable
fun GoalsRoute(nav: AppNav) {
    val vm: PlanViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    GoalsScreen(state, open = { nav.goal(it) }, add = { nav.goalEdit() })
}

/** What the owner is saving or investing toward, each with what it needs a month. */
@Composable
fun GoalsScreen(s: PlanState, open: (Long) -> Unit, add: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val g = s.goals
    SpacePage(
        "Goals",
        context = when {
            g.goals.isEmpty() -> "Save toward something, reach a balance, invest every month"
            g.behind > 0 -> Copy.plural(g.behind, "goal") + " behind pace · " + Copy.plural(g.onTrack, "on track", "on track")
            else -> Copy.plural(g.goals.size, "goal") + " · " + Copy.plural(g.reached, "reached", "reached")
        },
        actions = { IconKey("plus", add, label = "New goal") },
    ) {
        if (!s.loaded) return@SpacePage
        if (g.goals.isEmpty()) {
            Slab { Empty("target", "No goals yet", "A trip, a cushion, a card paid off, a share of every pay invested: Tally keeps the pace it needs.") { Key("Add a goal", add, glyph = "plus") } }
            return@SpacePage
        }
        if (g.pots > 0) {
            StatGrid(
                listOf(
                    { m -> Stat("Saved in pots", fmt.formatWhole(g.saved), m, glyph = "wallet", sub = "of " + fmt.formatWhole(g.target)) },
                    { m -> Stat("Next date", g.nextDate?.let { Dates.short(it, s.today) } ?: "–", m, glyph = "calendar", sub = g.nextDateName) },
                ),
            )
        }
        Panel("Goals", glyph = "target") {
            g.goals.forEach { goal -> GoalRow(goal, s.today, onClick = { open(goal.id) }) }
        }
        if (g.reached > 0) T(Copy.plural(g.reached, "goal") + " reached.", style = Tally.type.caption, color = c.liveText)
    }
}
