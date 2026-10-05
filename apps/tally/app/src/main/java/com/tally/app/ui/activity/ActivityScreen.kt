package com.tally.app.ui.activity

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.Figure
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.nav.AppNav
import com.tally.core.Copy

/**
 * History: every entry by day, Home's Recent "view all", as Avex keeps it. The search text lives
 * here, in saveable UI state, so typing never waits on a flow round trip; the ViewModel receives
 * it and settles it before searching.
 */
@Composable
fun HistoryRoute(nav: AppNav) {
    val viewModel: ActivityViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    var text by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(text) { viewModel.setQuery(text) }
    LifecycleResumeEffect(viewModel) {
        viewModel.onResume()
        onPauseOrDispose { }
    }
    ActivityScreen(
        state = state,
        query = text,
        actions = ActivityActions(
            back = nav::back,
            onQuery = { text = it },
            previousPeriod = viewModel::previousPeriod,
            nextPeriod = viewModel::nextPeriod,
            onFilter = viewModel::setFilter,
            openEntry = { nav.entry(id = it) },
        ),
    )
}

/** Everything History can do, as plain lambdas, so the screen renders in a test with no graph. */
data class ActivityActions(
    val back: () -> Unit = {},
    val onQuery: (String) -> Unit = {},
    val previousPeriod: () -> Unit = {},
    val nextPeriod: () -> Unit = {},
    val onFilter: (ActivityFilter) -> Unit = {},
    val openEntry: (Long) -> Unit = {},
)

/**
 * The ledger as Avex's History reads a log: the title and its figures (out, in, how many), the
 * filled search, the type lens, then the entries day by day, each day on its own panel. The
 * figures read the current lens, so a tap on "Income" is answered by the numbers moving. A typed
 * query searches every month.
 */
@Composable
fun ActivityScreen(state: ActivityState, query: String, actions: ActivityActions) {
    val periodName = remember(state.period, state.today) { Dates.period(state.period, state.today) }
    val options = remember { ActivityFilter.entries.map { it.label } }
    val bottom = 32.dp + WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = bottom),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = actions.back) {
                    PeriodStepper(
                        label = periodName,
                        searching = state.searching,
                        canGoNext = !state.isCurrentPeriod,
                        onPrevious = actions.previousPeriod,
                        onNext = actions.nextPeriod,
                    )
                }
                val context = when {
                    !state.loaded -> periodName
                    state.searching -> "Every month · " + Copy.plural(state.summary.count, "match", "matches")
                    else -> periodName + " · " + Copy.plural(state.summary.count, "entry", "entries")
                }
                PageTitle(
                    "History",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = context,
                )
            }
        }
        item(key = "figures") { HistoryFigures(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "controls") {
            Column(Modifier.padding(horizontal = GUTTER), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SearchField(query, actions.onQuery)
                if (state.searching) Caption("Searching every month", Modifier.padding(horizontal = 4.dp))
                SlidingSegments(
                    options = options,
                    selectedIndex = state.filter.ordinal,
                    onSelect = { actions.onFilter(ActivityFilter.entries[it]) },
                    modifier = Modifier.fillMaxWidth(),
                )
                if (state.truncated) Caption("Showing the newest ${LedgerRepository.SEARCH_LIMIT}", Modifier.padding(horizontal = 4.dp))
            }
        }
        if (state.loaded && state.days.isEmpty()) {
            item(key = "empty") {
                if (state.searching) {
                    EmptyPanel(
                        "No entries match \"${state.query}\"",
                        Modifier.padding(horizontal = GUTTER),
                        action = "clear search",
                        onAction = { actions.onQuery("") },
                    )
                } else {
                    EmptyPanel(
                        emptyLine(state.filter, periodName),
                        Modifier.padding(horizontal = GUTTER),
                    )
                }
            }
        } else {
            items(state.days, key = { "day-" + it.date.toEpochDay() }) { group ->
                DayPanel(group, state.today, actions.openEntry, Modifier.padding(horizontal = GUTTER))
            }
        }
    }
}

private fun emptyLine(filter: ActivityFilter, periodName: String): String = when (filter) {
    ActivityFilter.ALL -> "Nothing logged in $periodName"
    ActivityFilter.SPENT -> "No spending logged in $periodName"
    ActivityFilter.INCOME -> "No income logged in $periodName"
    ActivityFilter.TRANSFERS -> "No transfers in $periodName"
}

/**
 * The month stepper in the top bar's chrome: previous, the period's mono name, next. There is no
 * next past the current period, and while a query searches every month there is no month to step.
 */
@Composable
private fun PeriodStepper(
    label: String,
    searching: Boolean,
    canGoNext: Boolean,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
) {
    if (searching) {
        Text(
            "EVERY MONTH",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
        return
    }
    ChromeButton(Icons.AutoMirrored.Rounded.KeyboardArrowLeft, "Previous month", onPrevious)
    Text(
        label.uppercase(),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onBackground,
        textAlign = TextAlign.Center,
    )
    if (canGoNext) {
        ChromeButton(Icons.AutoMirrored.Rounded.KeyboardArrowRight, "Next month", onNext)
    } else {
        // Holds the label where it was, so stepping back and forth never shifts it.
        Spacer(Modifier.size(48.dp))
    }
}

/**
 * Avex's tiny hero: open serif figures on the page, no panel, honest at zero. What went out,
 * what came in, and how many entries, for what the lens shows; then the one entry that stood out.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HistoryFigures(state: ActivityState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val s = state.summary
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Figure(money.formatWhole(s.spent), "Out")
            Figure(money.formatWhole(s.income), "In", valueColor = if (s.income > 0L) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onBackground)
            Figure(s.count.toString(), if (s.count == 1) "Entry" else "Entries")
        }
        val largest = s.largest
        val line = buildList {
            if (s.spent > 0L || s.income > 0L) add(netLine(s.spent, s.income) { money.formatWhole(it) })
            if (largest != null) add("largest " + money.format(largest.amount) + ", " + entryTitle(largest) + " on " + Dates.short(largest.date, state.today))
            val perDay = state.perDay
            if (perDay != null && perDay > 0L && (state.filter == ActivityFilter.ALL || state.filter == ActivityFilter.SPENT)) {
                add(money.formatWhole(perDay) + " a day out")
            }
        }.joinToString(" · ")
        if (line.isNotEmpty()) {
            Text(line.replaceFirstChar { it.uppercase() }, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
