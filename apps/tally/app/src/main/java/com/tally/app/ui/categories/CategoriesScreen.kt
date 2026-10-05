package com.tally.app.ui.categories

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Inventory2
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.accounts.EmptyNote
import com.tally.app.ui.accounts.HeroHead
import com.tally.app.ui.accounts.HeroNumber
import com.tally.app.ui.accounts.TilePair
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LegendDot
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StackedBar
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.CategoryKind
import com.tally.core.Copy

private val LENSES = listOf("Spending", "Income")

@Composable
fun CategoriesRoute(nav: AppNav) {
    val viewModel: CategoriesViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    var lens by rememberSaveable { mutableIntStateOf(0) }
    val kind = if (lens == 1) CategoryKind.INCOME else CategoryKind.EXPENSE
    val actions = remember(nav) {
        CategoriesActions(
            back = nav::back,
            add = { k -> nav.categoryEdit(0, k) },
            open = { id, k -> nav.categoryEdit(id, k) },
        )
    }
    CategoriesScreen(state, kind, actions, onKind = { lens = if (it == CategoryKind.INCOME) 1 else 0 })
}

/** Everything the Categories list can do, as plain lambdas, so it renders in a test with no graph. */
data class CategoriesActions(
    val back: () -> Unit = {},
    /** A new category of the lens's kind. */
    val add: (CategoryKind) -> Unit = {},
    val open: (Long, CategoryKind) -> Unit = { _, _ -> },
)

/**
 * Categories: one kind at a time. This period's money by category under the light, the most used
 * and the never-used counts, then every active category with its entries and period total, and
 * the archived ones.
 */
@Composable
fun CategoriesScreen(
    state: CategoriesState,
    kind: CategoryKind,
    actions: CategoriesActions,
    onKind: (CategoryKind) -> Unit = {},
) {
    val summary = state.of(kind)
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = actions.back) { ChromeButton(Icons.Rounded.Add, "Add category", { actions.add(kind) }) }
                PageTitle(
                    "Categories",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = "${state.spending.active.size} for spending · ${state.income.active.size} for income",
                )
            }
        }
        item(key = "lens") {
            SlidingSegments(
                LENSES,
                if (kind == CategoryKind.INCOME) 1 else 0,
                { onKind(if (it == 1) CategoryKind.INCOME else CategoryKind.EXPENSE) },
                Modifier.padding(horizontal = GUTTER).fillMaxWidth(),
            )
        }
        if (state.loaded) {
            item(key = "hero") { KindHero(state, kind, summary, Modifier.padding(horizontal = GUTTER)) }
            item(key = "tiles") { KindTiles(summary, Modifier.padding(horizontal = GUTTER)) }
            item(key = "active") { ActivePanel(state, kind, summary, actions, Modifier.padding(horizontal = GUTTER)) }
            if (summary.archived.isNotEmpty()) {
                item(key = "archived") { ArchivedPanel(state, summary, actions, Modifier.padding(horizontal = GUTTER)) }
            }
            if (summary.active.isEmpty()) {
                item(key = "add") {
                    HeroAction(
                        if (kind == CategoryKind.INCOME) "Add an income category" else "Add a spending category",
                        { actions.add(kind) },
                        Modifier.padding(horizontal = GUTTER).fillMaxWidth(),
                    )
                }
            }
        }
    }
}

/**
 * This period's total for the kind as THE figure, split by category in the categories' own hues,
 * with the largest named beside their shares and chips for the counts behind them.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KindHero(state: CategoriesState, kind: CategoryKind, s: KindSummary, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val periodName = Dates.period(state.period, state.today)
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val bar = remember(s.segments, muted, track) {
        if (s.segments.isEmpty()) listOf(1f to track)
        else s.segments.map { it.share to (if (it.color == null) muted else categoryColor(it.color)) }
    }
    val income = kind == CategoryKind.INCOME
    val description = if (s.segments.isEmpty()) "Nothing logged in $periodName yet"
    else "$periodName by category: " + s.segments.joinToString(", ") { "${it.name} ${it.percent}%" }
    HeroPanel(modifier) {
        HeroHead(
            if (income) "Earned by category" else "Spent by category",
            end = periodName.uppercase(),
            tint = if (income) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(14.dp))
        HeroNumber(
            money.formatWhole(s.periodTotal),
            description = (if (income) "Earned " else "Spent ") + money.formatWhole(s.periodTotal) + " in " + periodName,
        )
        Text(
            if (s.periodTotal == 0L) "Nothing logged in $periodName yet"
            else "across ${s.usedThisPeriod} of ${Copy.plural(s.active.size, "category", "categories")} in $periodName",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        StackedBar(bar, description, height = 12.dp)
        if (s.segments.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            FlowRow(
                Modifier.fillMaxWidth().clearAndSetSemantics { },
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                s.segments.take(LEGEND_MAX).forEach { seg ->
                    LegendDot(if (seg.color == null) muted else categoryColor(seg.color), "${seg.name} ${seg.percent}%")
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(s.periodEntries, "entry", "entries") + " in " + periodName)
            if (s.periodEntries > 0) {
                StatChip(Icons.Rounded.Speed, money.formatWhole(s.averageEntry) + " average entry")
            }
            StatChip(Icons.AutoMirrored.Rounded.ReceiptLong, Copy.plural(s.entries, "entry", "entries") + " filed in all")
            if (s.archived.isNotEmpty()) StatChip(Icons.Rounded.Inventory2, "${s.archived.size} archived")
        }
    }
}

private const val LEGEND_MAX = 4

/** Two different readings: the category doing the most work, and the ones doing none. */
@Composable
private fun KindTiles(s: KindSummary, modifier: Modifier = Modifier) {
    val top = s.mostUsed
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Most used",
                (top?.entries ?: 0).toString(),
                m,
                detail = if (top == null) "No entries filed yet" else "entries in " + top.name,
            )
        },
        second = { m ->
            StatTile(
                "Unused",
                s.unused.toString(),
                m,
                detail = when {
                    s.active.isEmpty() -> "No categories yet"
                    s.unused == 0 -> "Every category has entries"
                    else -> "never used, archive to tidy the pickers"
                },
            )
        },
    )
}

@Composable
private fun ActivePanel(
    state: CategoriesState,
    kind: CategoryKind,
    s: KindSummary,
    actions: CategoriesActions,
    modifier: Modifier = Modifier,
) {
    Panel(modifier) {
        PanelHeader(
            "Active",
            meta = if (s.active.isEmpty()) null else Dates.period(state.period, state.today).uppercase(),
        )
        if (s.active.isEmpty()) {
            Spacer(Modifier.height(6.dp))
            EmptyNote(
                if (kind == CategoryKind.INCOME) "No income categories. Add one to say where money comes from"
                else "No spending categories. Add one to say where money goes",
            )
        } else {
            Spacer(Modifier.height(4.dp))
            val periodName = Dates.period(state.period, state.today)
            s.active.forEach { line -> CategoryRow(line, periodName, muted = false) { actions.open(line.id, line.kind) } }
        }
    }
}

@Composable
private fun ArchivedPanel(state: CategoriesState, s: KindSummary, actions: CategoriesActions, modifier: Modifier = Modifier) {
    val periodName = Dates.period(state.period, state.today)
    Panel(modifier) {
        PanelHeader(
            "Archived",
            meta = Copy.plural(s.archived.size, "category", "categories").uppercase(),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(4.dp))
        s.archived.forEach { line -> CategoryRow(line, periodName, muted = true) { actions.open(line.id, line.kind) } }
    }
}

/**
 * One category, bare in its panel: badge, name, entry count and this period's money. The whole
 * row opens the editor. Past 1.5x font the amount moves under the name so neither breaks.
 */
@Composable
private fun CategoryRow(line: CategoryLine, periodName: String, muted: Boolean, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = money.formatWhole(line.periodTotal)
    val entries = Copy.plural(line.entries, "entry", "entries") +
        if (line.periodTotal > 0) " · ${line.percent}% of $periodName" else ""
    val stacked = LocalDensity.current.fontScale > 1.5f
    val strong = if (muted) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
    val amountColor: Color = if (muted || line.periodTotal == 0L) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Edit ${line.name}", onClick = onClick)
            .semantics(mergeDescendants = true) {
                contentDescription = "${line.name}, $entries, $amount in $periodName"
            }
            .heightIn(min = 64.dp)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        CategoryBadge(line.icon, line.color, Modifier.alpha(if (muted) 0.6f else 1f))
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(line.name, style = MaterialTheme.typography.bodyLarge, color = strong)
            if (stacked) Text(amount, style = MaterialTheme.typography.titleMedium, color = amountColor)
            Text(entries, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (!stacked) {
            Text(amount, style = MaterialTheme.typography.titleMedium, color = amountColor, modifier = Modifier.clearAndSetSemantics { })
        }
    }
}
