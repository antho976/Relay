package com.tally.app.ui.activity

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.AccountBalanceWallet
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.LocalOffer
import androidx.compose.material.icons.rounded.Receipt
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.SearchOff
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.SwapHoriz
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
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.FAB_CLEARANCE
import com.tally.core.Copy
import com.tally.core.TxType
import java.time.LocalDate

/**
 * The Activity tab. The search text lives here, in saveable UI state, so typing never waits on a
 * flow round trip; the ViewModel receives it and settles it before searching.
 */
@Composable
fun ActivityTab(nav: AppNav) {
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
            onQuery = { text = it },
            previousPeriod = viewModel::previousPeriod,
            nextPeriod = viewModel::nextPeriod,
            onFilter = viewModel::setFilter,
            openEntry = { nav.entry(id = it) },
        ),
    )
}

/** Everything Activity can do, as plain lambdas, so the screen renders in a test with no graph. */
data class ActivityActions(
    val onQuery: (String) -> Unit = {},
    val previousPeriod: () -> Unit = {},
    val nextPeriod: () -> Unit = {},
    val onFilter: (ActivityFilter) -> Unit = {},
    val openEntry: (Long) -> Unit = {},
)

/**
 * The ledger browser: the period's money out and in under the lit panel, its largest and average
 * entry, then the filled search and the type lens, and the entries day by day, each day on its
 * own panel. A typed query searches every month.
 */
@Composable
fun ActivityScreen(state: ActivityState, query: String, actions: ActivityActions) {
    val periodName = remember(state.period, state.today) { Dates.period(state.period, state.today) }
    val options = remember { ActivityFilter.entries.map { it.label } }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = FAB_CLEARANCE),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = null) {
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
                    "Activity",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = context,
                )
            }
        }
        item(key = "hero") { ActivityHero(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "tiles") { SummaryTiles(state.summary, state.today, Modifier.padding(horizontal = GUTTER)) }
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
                        Icons.Rounded.SearchOff,
                        "No entries match \"${state.query}\"",
                        Modifier.padding(horizontal = GUTTER),
                        action = "clear search",
                        onAction = { actions.onQuery("") },
                    )
                } else {
                    EmptyPanel(
                        Icons.AutoMirrored.Rounded.ReceiptLong,
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

@Composable
private fun ActivityHero(state: ActivityState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val s = state.summary
    val meta = when {
        state.searching -> "ALL TIME"
        state.isCurrentPeriod -> "DAY ${state.period.elapsedDays(state.today)} OF ${state.period.days}"
        else -> Copy.plural(state.period.days, "day").uppercase()
    }
    val chips = buildList {
        add(HeroChip(Icons.Rounded.CalendarToday, "Active on " + Copy.plural(s.activeDays, "day")))
        val perDay = state.perDay
        if (perDay != null && (state.filter == ActivityFilter.ALL || state.filter == ActivityFilter.SPENT)) {
            add(HeroChip(Icons.Rounded.Speed, money.formatWhole(perDay) + " a day out"))
        }
        if (s.moved > 0) add(HeroChip(Icons.Rounded.SwapHoriz, money.formatWhole(s.moved) + " moved"))
    }
    FlowHero(
        icon = if (state.searching) Icons.Rounded.Search else Icons.Rounded.AccountBalanceWallet,
        // The period already names itself twice above (the stepper and the title's line).
        label = if (state.searching) "Matches" else "In and out",
        meta = meta,
        spent = s.spent,
        income = s.income,
        chips = chips,
        modifier = modifier,
    )
}

/** The largest and the average entry of what is shown, of spending when there is any. */
@Composable
private fun SummaryTiles(s: LedgerSummary, today: LocalDate, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val noun = basisNoun(s.basis)
    val isIn = s.basis == TxType.INCOME
    val tint = if (isIn) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary
    val valueColor = if (isIn) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onBackground
    val largest = s.largest
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                Icons.Rounded.LocalOffer,
                "Largest $noun",
                money.format(largest?.amount ?: 0L),
                m,
                detail = largest?.let { entryTitle(it) + " · " + Dates.short(it.date, today) } ?: "Nothing logged",
                tint = tint,
                valueColor = valueColor,
            )
        },
        second = { m ->
            StatTile(
                Icons.Rounded.Receipt,
                "Average $noun",
                money.format(s.average),
                m,
                detail = if (s.basisCount > 0) "Across " + Copy.plural(s.basisCount, "entry", "entries") else "Nothing logged",
                tint = tint,
                valueColor = valueColor,
            )
        },
    )
}
