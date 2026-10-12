package com.tally.app.ui.activity

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.items
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
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EntryRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Fill
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.PageBar
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpaceList
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.plan.paceTone
import com.tally.app.ui.theme.Tally
import com.tally.core.Copy
import com.tally.core.TxType

@Composable
fun TransactionsRoute(nav: AppNav, delete: (Long) -> Unit) {
    val vm: TransactionsViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    TransactionsScreen(
        s,
        back = nav::back,
        entry = { nav.entry(id = it) },
        delete = delete,
        query = vm::setQuery,
        edit = { if (s.isAccount) nav.accountEdit(s.accountId) else nav.categoryEdit(s.categoryId, s.categoryKind) },
        add = { if (s.isAccount) nav.entry(accountId = s.accountId) else nav.entry(type = if (s.categoryKind == com.tally.core.CategoryKind.INCOME) TxType.INCOME else TxType.EXPENSE, categoryId = s.categoryId) },
        budget = { nav.budgetEdit(s.categoryId) },
    )
}

/** One category's or one account's entries, with what it reads this month. */
@Composable
fun TransactionsScreen(
    s: TransactionsState,
    back: () -> Unit,
    entry: (Long) -> Unit,
    delete: (Long) -> Unit,
    query: (String) -> Unit,
    edit: () -> Unit,
    add: () -> Unit,
    budget: () -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var text by rememberSaveable { mutableStateOf("") }
    Column(Modifier.fillMaxSize().background(c.wall)) {
        PageBar(
            s.name ?: "",
            back,
            subtitle = if (s.loaded) Copy.plural(s.entryCount, "entry", "entries") else null,
            actions = {
                IconKey("plus", add, label = "New entry here")
                IconKey("edit", edit, label = "Edit")
            },
        )
        if (s.missing) {
            Empty("list", "This is gone", "It was deleted, here or on another of your devices.")
            return@Column
        }
        SpaceList(
            title = s.name ?: "",
            showTitle = false,
            bottom = 24.dp,
            header = {
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 6.dp) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        if (s.isAccount) AccountBadge(s.accountType, size = 40.dp) else CategoryBadge(s.icon, s.color, size = 40.dp)
                        Column(Modifier.weight(1f)) {
                            Eyebrow(if (s.isAccount) "Balance" else Dates.period(s.period, s.today))
                            T(
                                fmt.format(if (s.isAccount) s.balance else s.periodTotal),
                                style = Tally.type.hero.copy(fontSize = androidx.compose.ui.unit.TextUnit(32f, androidx.compose.ui.unit.TextUnitType.Sp)),
                                color = if (s.isAccount && s.balance < 0) c.heldText else c.ink,
                                maxLines = 1,
                            )
                        }
                    }
                    val r = s.reading
                    if (!s.isAccount && r != null) {
                        PaceMeter(r.spentFraction, r.paceFraction, height = 8.dp)
                        T(Copy.ofBudget(r.spent, r.budget, fmt) + " · " + Copy.daysLine(r), style = Tally.type.caption, color = paceTone(r))
                    } else if (!s.isAccount && s.categoryKind == com.tally.core.CategoryKind.EXPENSE) {
                        T("No budget · tap to set one", Modifier.padding(top = 2.dp).then(Modifier), Tally.type.caption, c.ink3)
                        com.tally.app.ui.kit.Key("Set a budget", budget, compact = true, glyph = "pie")
                    }
                }
                val stats: List<@Composable (Modifier) -> Unit> = if (s.isAccount) listOf(
                        { m -> Stat("In this month", fmt.formatWhole(s.flows.moneyIn), m, glyph = "money-in", sub = Copy.plural(s.flows.inCount, "entry", "entries")) },
                        { m -> Stat("Out this month", fmt.formatWhole(s.flows.moneyOut), m, glyph = "money-out", sub = Copy.plural(s.flows.outCount, "entry", "entries")) },
                    ) else listOf(
                        { m -> Stat("This month", fmt.formatWhole(s.periodTotal), m, glyph = "calendar", sub = Copy.plural(s.periodCount, "entry", "entries")) },
                        { m -> Stat("Last month", fmt.formatWhole(s.lastPeriodTotal), m, glyph = "history", sub = Copy.versusLastLine(s.periodTotal, s.lastPeriodSameDay, fmt)) },
                    )
                StatGrid(stats)
                Field(text, { text = it; query(it) }, Modifier.fillMaxWidth(), placeholder = "Search these entries", leading = "search")
            },
        ) {
            s.days.forEach { day ->
                item(key = "day-" + day.date) {
                    Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 14.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        T(Dates.day(day.date, s.today), style = Tally.type.section, color = c.ink2)
                        Fill()
                        T(fmt.formatWhole(day.spent + day.income), style = Tally.type.mono, color = c.ink3)
                    }
                }
                items(day.rows, key = { "tx-" + it.id }) { row -> EntryRow(row, onClick = { entry(row.id) }, onLongClick = { delete(row.id) }) }
            }
            if (s.loaded && s.days.isEmpty()) {
                item(key = "empty") { Empty("list", if (s.searching) "Nothing matches" else "No entries yet", null) }
            }
        }
    }
}
