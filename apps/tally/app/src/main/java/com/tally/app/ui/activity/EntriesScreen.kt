package com.tally.app.ui.activity

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EntryRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Fill
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.PageBar
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.SpaceList
import com.tally.app.ui.kit.T
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.core.Copy

data class EntriesActions(
    val entry: (Long) -> Unit = {},
    val delete: (Long) -> Unit = {},
    val previous: () -> Unit = {},
    val next: () -> Unit = {},
    val filter: (ActivityFilter) -> Unit = {},
    val query: (String) -> Unit = {},
    val back: () -> Unit = {},
)

@Composable
fun EntriesRoute(nav: AppNav, delete: (Long) -> Unit) {
    val vm: ActivityViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    EntriesScreen(
        state,
        EntriesActions(
            entry = { nav.entry(id = it) },
            delete = delete,
            previous = vm::previousPeriod,
            next = vm::nextPeriod,
            filter = vm::setFilter,
            query = vm::setQuery,
        ),
    )
}

/** Every entry of a month by day, month by month, narrowed by kind; tap to edit, hold to delete. */
@Composable
fun EntriesScreen(s: ActivityState, a: EntriesActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val sum = s.summary
    val context = if (!s.loaded) null else Copy.plural(sum.count, "entry", "entries") + " · " +
        fmt.formatWhole(sum.spent) + " out · " + fmt.formatWhole(sum.income) + " in"
    SpaceList(
        title = Dates.period(s.period, s.today),
        context = context,
        actions = {
            IconKey("chevron-left", a.previous, label = "Earlier month")
            IconKey("chevron-right", a.next, enabled = !s.isCurrentPeriod, label = "Later month")
        },
        header = {
            Segmented(ActivityFilter.entries.map { it to it.label }, s.filter, a.filter, Modifier.fillMaxWidth(), fill = true)
            s.perDay?.let { perDay ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Pill(fmt.formatWhole(perDay) + " a day", glyph = "calendar")
                    if (sum.activeDays > 0) Pill(Copy.plural(sum.activeDays, "day") + " with spending")
                }
            }
        },
    ) {
        days(s, a)
        if (s.loaded && s.days.isEmpty()) {
            item(key = "empty") {
                Empty("list", if (s.isCurrentPeriod) "Nothing logged this month" else "Nothing logged in this month", "Entries you add, import or that bills post land here.")
            }
        }
    }
}

/** The day groups: a header with the day's in and out, then its entries. */
internal fun LazyListScope.days(s: ActivityState, a: EntriesActions) {
    s.days.forEach { day ->
        item(key = "day-" + day.date) { DayHeader(day, s) }
        items(day.rows, key = { "tx-" + it.id }) { row ->
            EntryRow(row, onClick = { a.entry(row.id) }, onLongClick = { a.delete(row.id) }, modifier = Modifier.padding(horizontal = 0.dp))
        }
    }
    if (s.truncated) {
        item(key = "truncated") {
            T("Only the newest ${com.tally.app.data.repo.LedgerRepository.SEARCH_LIMIT} are listed. Narrow the search to see older ones.", Modifier.padding(vertical = 12.dp), Tally.type.caption, Tally.colors.ink3)
        }
    }
}

@Composable
private fun DayHeader(day: DayGroup, s: ActivityState) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 14.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        T(Dates.day(day.date, s.today), style = Tally.type.section, color = c.ink2)
        Fill()
        if (day.income > 0) T("+" + fmt.formatWhole(day.income), style = Tally.type.mono, color = c.liveText)
        if (day.income > 0 && day.spent > 0) T("  ", style = Tally.type.mono)
        if (day.spent > 0) T(fmt.formatWhole(day.spent), style = Tally.type.mono, color = c.ink3)
    }
}

@Composable
fun SearchRoute(nav: AppNav, delete: (Long) -> Unit) {
    val vm: ActivityViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    SearchScreen(
        state,
        EntriesActions(entry = { nav.entry(id = it) }, delete = delete, filter = vm::setFilter, query = vm::setQuery, back = nav::back),
    )
}

/** Every entry ever, by what was typed: a note, a payee, a category or an account. */
@Composable
fun SearchScreen(s: ActivityState, a: EntriesActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var text by rememberSaveable { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    val clear: @Composable RowScope.() -> Unit = { IconKey("close", { text = ""; a.query("") }, size = 14.dp, label = "Clear") }
    Column(Modifier.fillMaxSize().padding(0.dp)) {
        PageBar("Search", a.back)
        SpaceList(
            title = "Search",
            showTitle = false,
            bottom = 24.dp,
            header = {
                Field(
                    text,
                    { text = it; a.query(it) },
                    Modifier.fillMaxWidth().focusRequester(focus),
                    placeholder = "A note, a payee, a category",
                    leading = "search",
                    trailing = if (text.isNotEmpty()) clear else null,
                )
                Segmented(ActivityFilter.entries.map { it to it.label }, s.filter, a.filter, Modifier.fillMaxWidth(), fill = true)
                if (s.searching && s.loaded) {
                    T(
                        Copy.plural(s.summary.count, "entry", "entries") + " · " + fmt.formatWhole(s.summary.spent) + " out · " + fmt.formatWhole(s.summary.income) + " in",
                        style = Tally.type.caption,
                        color = c.ink3,
                    )
                }
            },
        ) {
            if (s.searching) days(s, a)
            if (s.searching && s.loaded && s.days.isEmpty()) {
                item(key = "none") { Empty("search", "Nothing matches", "Search reads notes, categories and accounts, across every month.") }
            }
            if (!s.searching) {
                item(key = "hint") { T("Type to search every month at once.", Modifier.padding(4.dp), Tally.type.caption, c.ink3) }
            }
        }
    }
}

/** Padding for rows inside a lazy list's column. */
internal val ROW_PADDING = PaddingValues(horizontal = 4.dp, vertical = 8.dp)
