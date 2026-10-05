package com.tally.app.ui.plan

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.DayBars
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy

@Composable
fun BudgetEditRoute(nav: AppNav) {
    val viewModel: BudgetEditViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        BudgetEditActions(
            back = nav::back,
            remove = viewModel::remove,
            press = viewModel::press,
            fill = viewModel::fill,
            save = viewModel::save,
        )
    }
    BudgetEditScreen(state, actions)
}

/** Everything the budget editor can do, as plain lambdas. */
data class BudgetEditActions(
    val back: () -> Unit = {},
    val remove: () -> Unit = {},
    val press: (PadKey) -> Unit = {},
    val fill: (Long) -> Unit = {},
    val save: () -> Unit = {},
)

/**
 * The budget editor: the limit as the one lit figure with this month read against it, the last
 * four months as bars under the limit's dashed line, quick fills from that history, then the
 * keypad and the save under the thumb.
 */
@Composable
fun BudgetEditScreen(state: BudgetEditState, actions: BudgetEditActions) {
    val money = LocalMoney.current
    Column(Modifier.fillMaxSize().navigationBarsPadding()) {
        TopBar(onBack = actions.back) {
            if (state.existing != null) ChromeButton(Icons.Rounded.DeleteOutline, "Remove budget", actions.remove)
        }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = GUTTER, end = GUTTER, top = 4.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
        ) {
            PageTitle(state.name ?: "Monthly budget", context = resetLine(state.startDay))
            LimitHero(state)
            if (state.loaded) {
                HistoryPanel(state)
                QuickFill(state, actions)
            }
        }
        Column(
            Modifier
                .fillMaxWidth()
                .padding(start = GUTTER, end = GUTTER, top = 10.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            PlanKeypad(showDecimal = money.fractionDigits > 0, separator = money.decimalSeparator, onKey = actions.press)
            HeroAction("Save budget", actions.save, Modifier.fillMaxWidth(), enabled = state.loaded && state.input.minor > 0L)
        }
    }
}

/**
 * The limit being typed, under the accent's light, with this month so far read against it: the
 * meter and its pace tick show at once what the new number means today.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LimitHero(state: BudgetEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val minor = state.input.minor
    val r = state.reading
    val formatted = money.format(minor)
    val spent = money.formatWhole(state.thisMonth)
    val line = when {
        minor <= 0L -> "$spent spent so far this month, with no limit to read it against"
        state.thisMonth > minor -> money.formatWhole(state.thisMonth - minor) + " over this limit already this month"
        else -> money.formatWhole(minor - state.thisMonth) + " left this month at this limit"
    }
    HeroPanel(modifier) {
        if (state.isOverall) {
            HeroLabel("Monthly limit", end = money.currency.currencyCode)
        } else {
            HeroLabel(
                "Monthly limit",
                end = money.currency.currencyCode,
                tint = categoryColor(state.color),
            )
        }
        Spacer(Modifier.height(10.dp))
        HeroFigure(
            formatted,
            color = if (minor > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            description = "Budget, $formatted",
            live = true,
        )
        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(14.dp))
        PaceMeter(
            r.spentFraction,
            if (minor > 0L) r.paceFraction else null,
            if (minor > 0L) {
                "Spent $spent of ${money.formatWhole(minor)}. An even pace would be ${money.formatWhole(r.expected)} by today."
            } else {
                "No limit typed yet. Spent $spent this month."
            },
            height = 14.dp,
        )
        Spacer(Modifier.height(14.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(r.daysLeft, "day") + " left")
            if (minor > 0L && r.dailyAllowance > 0L) {
                StatChip(Icons.Rounded.Speed, money.formatWhole(r.dailyAllowance) + " a day from here")
            }
            val existing = state.existing
            if (existing != null && existing != minor) {
                StatChip(Icons.Rounded.History, "Was " + money.formatWhole(existing))
            }
        }
    }
}

/** The current month and the three before it as bars, the typed limit as the dashed line. */
@Composable
private fun HistoryPanel(state: BudgetEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val labels = remember(state.historyStarts) { state.historyStarts.map { Dates.monthShort(it) } }
    val limit = state.input.minor.takeIf { it > 0L } ?: state.existing
    val description = "Spent by month: " + state.history.mapIndexed { i, v ->
        (labels.getOrNull(i) ?: "") + " " + money.formatWhole(v)
    }.joinToString(", ") + (limit?.let { ". Limit " + money.formatWhole(it) } ?: "")
    val average = state.average
    Panel(modifier) {
        PanelHeader("Spending history", meta = "LAST " + state.history.size + " MONTHS")
        Spacer(Modifier.height(6.dp))
        Text(
            when {
                average == null -> "No earlier months on record yet"
                state.monthsOnRecord == 1 -> "One earlier month on record"
                else -> Copy.plural(state.monthsOnRecord, "earlier month") + " on record"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(14.dp))
        DayBars(
            state.history,
            labels,
            highlight = state.history.lastIndex,
            description = description,
            allowance = limit,
        )
        Spacer(Modifier.height(14.dp))
        FigureRow(
            listOf(
                FigureCell("This month", money.formatWhole(state.thisMonth)),
                FigureCell("Last month", money.formatWhole(state.lastMonth)),
                FigureCell("3-month average", money.formatWhole(average ?: 0L)),
            )
        )
    }
}

/** Fills from the history: last month rounded up, the average, or what was set before. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun QuickFill(state: BudgetEditState, actions: BudgetEditActions) {
    val money = LocalMoney.current
    val minor = state.input.minor
    val last = state.suggestLast
    val average = state.suggestAverage
    val existing = state.existing
    Column(Modifier.fillMaxWidth()) {
        Text(
            "Fill from your history",
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(4.dp))
        if (last == null && average == null && (existing == null || existing == minor)) {
            Spacer(Modifier.height(6.dp))
            EmptyLine("Nothing on record to suggest from yet")
        } else {
            FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (last != null) {
                    ChoiceChip("Last month, rounded · " + money.formatWhole(last), minor == last) { actions.fill(last) }
                }
                if (average != null && average != last) {
                    ChoiceChip("3-month average · " + money.formatWhole(average), minor == average) { actions.fill(average) }
                }
                if (existing != null && existing != last && existing != average) {
                    ChoiceChip("Current · " + money.formatWhole(existing), minor == existing) { actions.fill(existing) }
                }
            }
        }
    }
}
