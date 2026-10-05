package com.tally.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Autorenew
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.Today
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.CurrencyPlan
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChoiceRow
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.nav.AppNav
import com.tally.core.BudgetPeriod

@Composable
fun FormatRoute(nav: AppNav) {
    val viewModel: FormatViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(viewModel, nav) {
        FormatActions(
            back = nav::back,
            setCurrency = viewModel::setCurrency,
            setMonthStartDay = viewModel::setMonthStartDay,
            setWeekStartsMonday = viewModel::setWeekStartsMonday,
            confirmCurrency = viewModel::confirmCurrency,
            cancelCurrency = viewModel::cancelCurrency,
        )
    }
    FormatScreen(state, actions)
}

/**
 * Everything Currency & dates can change, as plain lambdas. Each write lands at once, except a
 * currency switch that would round stored amounts: that one waits for [confirmCurrency].
 */
data class FormatActions(
    val back: () -> Unit = {},
    val setCurrency: (String) -> Unit = {},
    val setMonthStartDay: (Int) -> Unit = {},
    val setWeekStartsMonday: (Boolean) -> Unit = {},
    val confirmCurrency: () -> Unit = {},
    val cancelCurrency: () -> Unit = {},
)

/**
 * Currency & dates: how an amount reads in the picked currency under the one lit panel, with the
 * month and week it is counted in, then the budget month and the currency list.
 */
@Composable
fun FormatScreen(state: FormatState, actions: FormatActions) {
    var pickingDay by rememberSaveable { mutableStateOf(false) }
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            PageHead(
                "Currency & dates",
                onBack = actions.back,
                context = "How amounts read, and when your month turns over",
            )
        }
        item(key = "hero") { FormatHero(state, Modifier.padding(horizontal = GUTTER)) }
        // The short group first, so the month and week never sit under the whole currency list.
        item(key = "month") {
            MonthGroup(state, actions, onPickDay = { pickingDay = true }, modifier = Modifier.padding(horizontal = GUTTER).padding(top = 10.dp))
        }
        item(key = "currency") { CurrencyGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
    }
    if (pickingDay) {
        MonthStartDialog(
            selected = state.monthStartDay,
            onPick = { day ->
                actions.setMonthStartDay(day)
                pickingDay = false
            },
            onDismiss = { pickingDay = false },
        )
    }
    state.pendingCurrency?.let { plan ->
        RoundingDialog(plan, onConfirm = actions.confirmCurrency, onDismiss = actions.cancelCurrency)
    }
}

/**
 * The one currency switch that asks first: fewer decimals round amounts the ledger holds, and
 * switching back cannot bring the dropped digits back, so there is no undo to offer instead.
 */
@Composable
private fun RoundingDialog(plan: CurrencyPlan, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    val locale = LocalMoney.current.locale
    val option = remember(plan.code, locale) { currencyOption(plan.code, locale) }
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text("Switch and round", color = MaterialTheme.colorScheme.primary)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = MaterialTheme.colorScheme.onBackground)
            }
        },
        title = { Text("Switch to ${option.name}?", style = MaterialTheme.typography.headlineSmall) },
        text = { Text(roundingPrompt(option.name, plan.toDigits, plan.rounded), style = MaterialTheme.typography.bodyLarge) },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        titleContentColor = MaterialTheme.colorScheme.onBackground,
        textContentColor = MaterialTheme.colorScheme.onBackground,
    )
}

/** A sample amount in the picked currency as the figure, and the calendar it is counted on as chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FormatHero(state: FormatState, modifier: Modifier = Modifier) {
    val locale = LocalMoney.current.locale
    val option = remember(state.currency, locale) { currencyOption(state.currency, locale) }
    HeroPanel(modifier) {
        HeroHead("Currency", end = option.code)
        Spacer(Modifier.height(12.dp))
        HeroNumber(option.sample, description = "Amounts read like ${option.sample}")
        Text(
            option.name + " · " + option.decimalsLine.replaceFirstChar { it.lowercase() },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarMonth, "Month from " + monthStartLabel(state.monthStartDay))
            StatChip(Icons.Rounded.Today, "Week from " + weekStartLabel(state.weekStartsMonday))
            StatChip(Icons.Rounded.Autorenew, "Resets " + Dates.short(state.period.endExclusive, state.today))
        }
    }
}

@Composable
private fun CurrencyGroup(state: FormatState, actions: FormatActions, modifier: Modifier = Modifier) {
    val locale = LocalMoney.current.locale
    val options = remember(state.currency, locale) { currencyCodes(state.currency).map { currencyOption(it, locale) } }
    val rows = options.map { option ->
        val row: @Composable (Shape) -> Unit = { shape ->
            CurrencyRow(option, selected = option.code == state.currency, shape = shape) { actions.setCurrency(option.code) }
        }
        row
    }
    Group(
        rows = rows,
        modifier = modifier,
        title = "Currency",
        trailing = state.currency,
        footer = "Changing the currency relabels amounts; it does not convert them, and it asks before rounding to fewer decimals",
    )
}

@Composable
private fun MonthGroup(state: FormatState, actions: FormatActions, onPickDay: () -> Unit, modifier: Modifier = Modifier) {
    val start: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Month starts on",
            shape,
            subtitle = "This period runs " + Dates.short(state.period.start, state.today) + " to " +
                Dates.short(state.period.lastDay, state.today),
            leading = { RowBadge(Icons.Rounded.CalendarMonth) },
            trailing = {
                Text(
                    monthStartLabel(state.monthStartDay),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            onClick = onPickDay,
        )
    }
    val week: @Composable (Shape) -> Unit = { shape ->
        ChoiceRow(
            "Week starts on",
            options = listOf("Monday", "Sunday"),
            selectedIndex = if (state.weekStartsMonday) 0 else 1,
            onSelect = { actions.setWeekStartsMonday(it == 0) },
            shape = shape,
        )
    }
    Group(
        rows = listOf(start, week),
        modifier = modifier,
        title = "Budget month",
        footer = "Pick your payday to budget paycheque to paycheque",
    )
}

/** The start day as chips 1 to 28: every month has each of them, so no cycle ever skips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MonthStartDialog(selected: Int, onPick: (Int) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("Close", color = MaterialTheme.colorScheme.primary)
            }
        },
        title = { Text("Month starts on", style = MaterialTheme.typography.headlineSmall) },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                Text(
                    "Budgets and the pace tick start over on this day each month.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(12.dp))
                FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    (1..BudgetPeriod.MAX_START_DAY).forEach { day ->
                        ChoiceChip(day.toString(), day == selected) { onPick(day) }
                    }
                }
            }
        },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        titleContentColor = MaterialTheme.colorScheme.onBackground,
        textContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
