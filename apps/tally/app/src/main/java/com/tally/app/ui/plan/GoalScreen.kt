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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Event
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.db.ContributionEntity
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.GroupFooter
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_FOCUS_OUTSET
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.GoalKind
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/** Which money sheet is open: none, adding, or withdrawing. */
private const val SHEET_NONE = 0
private const val SHEET_ADD = 1
private const val SHEET_WITHDRAW = -1

@Composable
fun GoalRoute(nav: AppNav) {
    val viewModel: GoalViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(viewModel, nav) {
        GoalActions(
            back = nav::back,
            edit = { id -> nav.goalEdit(id) },
            contribute = viewModel::contribute,
            deleteContribution = viewModel::deleteContribution,
        )
    }
    GoalScreen(state, actions)
}

/** Everything the goal screen can do, as plain lambdas. */
data class GoalActions(
    val back: () -> Unit = {},
    val edit: (Long) -> Unit = {},
    /** Signed: positive adds, negative withdraws. */
    val contribute: (Long, String) -> Unit = { _, _ -> },
    val deleteContribution: (Long) -> Unit = {},
)

/**
 * One goal: what is saved under the light against the target, add or withdraw side by side, how
 * fast it is filling and when that lands, then every contribution. A tap on a contribution
 * removes it, with Undo.
 */
@Composable
fun GoalScreen(state: GoalState, actions: GoalActions) {
    var sheet by rememberSaveable { mutableIntStateOf(SHEET_NONE) }
    val goal = state.goal
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            val money = LocalMoney.current
            Column {
                TopBar(onBack = actions.back) {
                    if (goal != null) ChromeButton(Icons.Rounded.Edit, "Edit goal", { actions.edit(goal.id) })
                }
                if (state.loaded) {
                    val context = goal?.let { g ->
                        when {
                            g.monthly && g.percent > 0 -> "${g.percent}% of what comes in, every month"
                            g.monthly -> money.formatWhole(g.target) + " every month"
                            else -> "Target " + money.formatWhole(g.target) + (g.targetDate?.let { " by " + Dates.short(it, state.today) } ?: "")
                        }
                    }
                    PageTitle(
                        goal?.name ?: "Goal",
                        Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                        context = context,
                    )
                }
            }
        }
        if (state.loaded && goal == null) {
            item(key = "gone") {
                Panel(Modifier.padding(horizontal = GUTTER)) {
                    EmptyLine("This goal was deleted", action = "back", onAction = actions.back)
                }
            }
        }
        if (goal != null) {
            when (goal.kind) {
                GoalKind.SAVINGS -> {
                    item(key = "hero") { GoalHero(state, goal, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "acts") {
                        GoalActs(
                            canWithdraw = goal.saved > 0L,
                            onAdd = { sheet = SHEET_ADD },
                            onWithdraw = { sheet = SHEET_WITHDRAW },
                            modifier = Modifier.padding(horizontal = GUTTER),
                        )
                    }
                    item(key = "pace") { PaceTiles(state, goal, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "flow") { FlowTiles(state, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "list") { ContributionsPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
                }
                GoalKind.BALANCE -> {
                    item(key = "hero") { BalanceHero(state, goal, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "pace") { PaceTiles(state, goal, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "note") { ReadsNote(goal, Modifier.padding(horizontal = GUTTER)) }
                }
                GoalKind.INVEST, GoalKind.SAVE -> {
                    item(key = "hero") { MonthlyHero(state, goal, Modifier.padding(horizontal = GUTTER)) }
                    item(key = "months") { MonthsPanel(state, goal, Modifier.padding(horizontal = GUTTER)) }
                }
            }
        }
    }
    if (goal != null && sheet != SHEET_NONE) {
        MoneySheet(
            withdraw = sheet == SHEET_WITHDRAW,
            saved = goal.saved,
            onConfirm = { amount, note -> actions.contribute(amount, note) },
            onDismiss = { sheet = SHEET_NONE },
        )
    }
}

/**
 * Add and withdraw, side by side at matching heights. Past 1.3x font a half-width button cannot
 * hold "Withdraw" whole, so the two stack at full width instead.
 */
@Composable
private fun GoalActs(canWithdraw: Boolean, onAdd: () -> Unit, onWithdraw: () -> Unit, modifier: Modifier = Modifier) {
    if (LocalDensity.current.fontScale > 1.3f) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            HeroAction("Add money", onAdd, Modifier.fillMaxWidth())
            SecondaryAction("Withdraw", onWithdraw, Modifier.fillMaxWidth(), enabled = canWithdraw)
        }
    } else {
        Row(
            modifier.fillMaxWidth().height(IntrinsicSize.Min),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            HeroAction("Add money", onAdd, Modifier.weight(1f).fillMaxHeight())
            SecondaryAction("Withdraw", onWithdraw, Modifier.weight(1f).fillMaxHeight(), enabled = canWithdraw)
        }
    }
}

/** Saved as the figure, the meter in the goal's own hue, and what is left in figures and chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GoalHero(state: GoalState, goal: GoalLine, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val hue = categoryColor(goal.color)
    val percent = (goal.fraction * 100f).roundToInt()
    val line = Copy.goalLine(goal.saved, goal.target, goal.monthsLeft, money)
    HeroPanel(modifier) {
        HeroLabel(
            if (goal.reached) "Reached" else "Saved",
            end = "$percent%",
            tint = hue,
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(goal.saved))
        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(16.dp))
        // The tick stands where an even saving from the first contribution to the date would be today.
        PaceMeter(
            goal.fraction,
            goal.paceFraction,
            "Saved ${money.formatWhole(goal.saved)} of ${money.formatWhole(goal.target)}, " +
                goalMeterWords(goal.fraction, goal.paceFraction),
            height = 14.dp,
            fill = hue,
        )
        Spacer(Modifier.height(16.dp))
        FigureRow(
            listOf(
                FigureCell("To go", money.formatWhole(goal.left)),
                FigureCell("Target", money.formatWhole(goal.target)),
                FigureCell("Entries", state.contributions.size.toString()),
            )
        )
        val date = goal.targetDate
        val months = goal.monthsLeft
        if (date != null && months != null) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                StatChip(Icons.Rounded.CalendarToday, if (months <= 0) "Date passed" else Copy.plural(months, "month") + " left")
                StatChip(Icons.Rounded.Flag, "By " + Dates.short(date, state.today))
            }
        }
    }
}

/** How fast it is filling, and when that lands against the date. */
@Composable
private fun PaceTiles(state: GoalState, goal: GoalLine, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val pace = state.pace
    val reachBy = pace?.reachBy
    val date = goal.targetDate
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Your pace",
                money.formatWhole(pace?.perMonth ?: 0L),
                m,
                detail = when {
                    pace == null && goal.kind == GoalKind.BALANCE -> "Shows once it has moved"
                    pace == null -> "Add money to see your pace"
                    goal.kind == GoalKind.BALANCE -> "A month toward it over " + Copy.plural(pace.months, "month")
                    else -> "A month on average over " + Copy.plural(pace.months, "month")
                },
            )
        },
        second = { m ->
            StatTile(
                "At this pace",
                when {
                    goal.reached -> "Reached"
                    reachBy != null -> Dates.short(reachBy, state.today)
                    else -> "Not yet"
                },
                m,
                detail = when {
                    goal.reached -> "Target met"
                    reachBy != null && date != null ->
                        if (reachBy.isAfter(date)) "After your date of " + Dates.short(date, state.today) else "On or before your date"
                    reachBy != null -> "When the target is met"
                    goal.kind == GoalKind.BALANCE -> "Not moving toward it yet"
                    else -> "Needs money coming in"
                },
            )
        },
    )
}

/** Money in and money out over the goal's life. */
@Composable
private fun FlowTiles(state: GoalState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val ins = state.addedCount
    val outs = state.withdrawnCount
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Added",
                money.formatWhole(state.added),
                m,
                detail = if (ins == 0) "Nothing added yet" else Copy.plural(ins, "time"),
            )
        },
        second = { m ->
            StatTile(
                "Withdrawn",
                money.formatWhole(state.withdrawn),
                m,
                detail = if (outs == 0) "Nothing taken out" else Copy.plural(outs, "time"),
            )
        },
    )
}

/**
 * A balance goal under the light: what its account (or everything you own) reads now, the meter
 * from where it stood when the goal was set to the target, and the even pace to the date as the tick.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BalanceHero(state: GoalState, goal: GoalLine, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val hue = categoryColor(goal.color)
    val percent = (goal.fraction * 100f).roundToInt()
    HeroPanel(modifier) {
        HeroLabel(
            if (goal.reached) "Reached" else (goal.accountName ?: "Everything you own"),
            end = "$percent%",
            tint = hue,
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(goal.saved))
        Text(goalRowLine(goal, money, state.today), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            goal.fraction,
            goal.paceFraction,
            "Reads ${money.formatWhole(goal.saved)}, from ${money.formatWhole(goal.start)} toward ${money.formatWhole(goal.target)}, " +
                goalMeterWords(goal.fraction, goal.paceFraction),
            height = 14.dp,
            fill = hue,
        )
        Spacer(Modifier.height(16.dp))
        FigureRow(
            listOf(
                FigureCell("Started at", money.formatWhole(goal.start)),
                FigureCell("Target", money.formatWhole(goal.target)),
                FigureCell("To go", money.formatWhole(goal.left)),
            )
        )
        val date = goal.targetDate
        val months = goal.monthsLeft
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val since = state.startDate
            if (since != null) StatChip(Icons.Rounded.Event, "Set " + Dates.short(since, state.today))
            if (date != null) StatChip(Icons.Rounded.Flag, "By " + Dates.short(date, state.today))
            if (date != null && months != null) {
                StatChip(Icons.Rounded.CalendarToday, if (months <= 0) "Date passed" else Copy.plural(months, "month") + " left")
            }
        }
    }
}

/** Where a balance goal's number comes from, said once under it. */
@Composable
private fun ReadsNote(goal: GoalLine, modifier: Modifier = Modifier) {
    Panel(modifier) {
        EmptyLine(
            if (goal.accountName != null) {
                "It moves with ${goal.accountName}: every entry in it, and its value when you update one"
            } else {
                "It moves with every open account: what you hold less what you owe"
            },
        )
    }
}

/**
 * A monthly goal under the light: this month's money moved (or kept), the meter against this
 * month's ask with the month's even pace as the tick, and the figures that make the ask.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MonthlyHero(state: GoalState, goal: GoalLine, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val hue = categoryColor(goal.color)
    val invest = goal.kind == GoalKind.INVEST
    HeroPanel(modifier) {
        HeroLabel(
            if (invest) "Invested this month" else "Kept this month",
            end = goal.achievedPercent?.let { "$it% OF INCOME" },
            tint = hue,
        )
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(goal.saved))
        Text(goalRowLine(goal, money, state.today), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            goal.fraction,
            goal.paceFraction,
            "${money.formatWhole(goal.saved)} of ${money.formatWhole(goal.target)} asked this month, " +
                goalMeterWords(goal.fraction, goal.paceFraction),
            height = 14.dp,
            fill = hue,
        )
        Spacer(Modifier.height(16.dp))
        FigureRow(
            listOf(
                FigureCell("Asked", money.formatWhole(goal.target)),
                FigureCell("Came in", money.formatWhole(goal.income), MaterialTheme.colorScheme.tertiary),
                FigureCell(if (goal.percent > 0) "Share asked" else "To go", if (goal.percent > 0) "${goal.percent}%" else money.formatWhole(goal.left)),
            )
        )
        if (goal.streak > 0 || goal.accountName != null) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (goal.streak > 0) StatChip(Icons.Rounded.EmojiEvents, Copy.plural(goal.streak, "month") + " met in a row")
                goal.accountName?.let { StatChip(Icons.Rounded.AccountBalance, it) }
            }
        }
    }
}

/** The months before this one: what each asked, what happened, met or not. */
@Composable
private fun MonthsPanel(state: GoalState, goal: GoalLine, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val months = goal.history.asReversed()
    Panel(modifier) {
        PanelHeader(
            "Earlier months",
            meta = if (months.isEmpty()) null else "${months.count { it.met }} OF ${months.size} MET",
        )
        if (months.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine("Your first full month shows here")
        } else {
            Spacer(Modifier.height(4.dp))
            months.forEach { m ->
                val name = Dates.month(m.start) + (if (m.start.year != state.today.year) " ${m.start.year}" else "")
                val detail = when {
                    m.income <= 0L && goal.percent > 0 -> "Nothing came in"
                    m.met -> "Met · " + money.formatWhole(m.achieved) + " of " + money.formatWhole(m.asked)
                    else -> money.formatWhole(m.achieved) + " of " + money.formatWhole(m.asked) + " asked"
                }
                Row(
                    Modifier
                        .fillMaxWidth()
                        .semantics(mergeDescendants = true) { contentDescription = "$name, $detail" }
                        .heightIn(min = 56.dp)
                        .padding(vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    GlyphBadge(
                        if (m.met) Icons.Rounded.Check else Icons.Rounded.Remove,
                        tint = if (m.met) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
                        size = 36.dp,
                    )
                    Column(Modifier.weight(1f).clearAndSetSemantics { }) {
                        Text(name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                        Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
private fun ContributionsPanel(state: GoalState, actions: GoalActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader(
            "Contributions",
            meta = if (state.contributions.isEmpty()) null else state.contributions.size.toString(),
        )
        if (state.contributions.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            EmptyLine("No money set aside yet")
        } else {
            Spacer(Modifier.height(4.dp))
            Text(
                "Tap one to remove it. Undo puts it back",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(4.dp))
            state.contributions.forEach { c -> ContributionRow(c, state) { actions.deleteContribution(c.id) } }
        }
    }
}

@Composable
private fun ContributionRow(c: ContributionEntity, state: GoalState, onClick: () -> Unit) {
    val money = LocalMoney.current
    val added = c.amount >= 0L
    val amount = if (added) money.formatSigned(c.amount) else money.format(c.amount)
    val title = c.note.ifBlank { if (added) "Added" else "Withdrawn" }
    val day = Dates.day(c.date, state.today)
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(
                label = "Remove this ${if (added) "contribution" else "withdrawal"}",
                focusOutset = ROW_FOCUS_OUTSET,
                onClick = onClick,
            )
            .semantics(mergeDescendants = true) { contentDescription = "$title, $amount, $day" }
            .heightIn(min = 60.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        GlyphBadge(
            if (added) Icons.Rounded.ArrowDownward else Icons.Rounded.ArrowUpward,
            tint = if (added) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
            size = 40.dp,
        )
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    Text(day, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = {
                Text(
                    amount,
                    style = MaterialTheme.typography.titleMedium,
                    color = if (added) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onBackground,
                    textAlign = TextAlign.End,
                )
            },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
            gap = 14.dp,
        )
    }
}

/**
 * The add or withdraw sheet: an amount, an optional note, and the one commit. A withdrawal cannot
 * take out more than the goal holds; the reason reads under the field.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MoneySheet(withdraw: Boolean, saved: Long, onConfirm: (Long, String) -> Unit, onDismiss: () -> Unit) {
    val money = LocalMoney.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    var text by rememberSaveable { mutableStateOf("") }
    var note by rememberSaveable { mutableStateOf("") }
    var tried by rememberSaveable { mutableStateOf(false) }
    // One commit per visit: the sheet stays on screen, and tappable, while it slides away.
    var committed by rememberSaveable { mutableStateOf(false) }
    // A rotation mid-slide brings the sheet back after its commit went through: close it again,
    // rather than leave a sheet whose action does nothing.
    LaunchedEffect(Unit) { if (committed) onDismiss() }
    val value = money.parse(text) ?: 0L
    val problem = when {
        value <= 0L -> "Enter an amount above zero"
        withdraw && value > saved -> "This goal holds " + money.format(saved)
        else -> null
    }
    val close: () -> Unit = {
        scope.launch { sheetState.hide() }.invokeOnCompletion { onDismiss() }
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .imePadding()
                .navigationBarsPadding()
                .padding(start = GUTTER, end = GUTTER, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                if (withdraw) "Withdraw" else "Add money",
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.semantics { heading() },
            )
            Text(
                if (withdraw) "Take money out of this goal. It holds " + money.format(saved)
                else "Set money aside for this goal. It holds " + money.format(saved),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Column(Modifier.fillMaxWidth()) {
                PlanField(
                    text,
                    { text = it },
                    "Amount",
                    Icons.Rounded.Payments,
                    keyboardType = KeyboardType.Decimal,
                    suffix = money.currency.currencyCode,
                    isError = tried && problem != null,
                )
                if (tried && problem != null) GroupFooter(problem, isError = true)
            }
            PlanField(
                note,
                { note = it },
                "Note (optional)",
                Icons.Rounded.EditNote,
                capitalization = KeyboardCapitalization.Sentences,
            )
            HeroAction(
                if (withdraw) "Withdraw" else "Add money",
                {
                    if (!committed) {
                        if (problem != null) {
                            tried = true
                        } else {
                            committed = true
                            onConfirm(if (withdraw) -value else value, note)
                            close()
                        }
                    }
                },
                Modifier.fillMaxWidth(),
            )
        }
    }
}
