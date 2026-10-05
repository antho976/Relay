package com.tally.app.ui.plan

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.categories.HueSwatches
import com.tally.app.ui.common.ChoiceChip
import com.tally.app.ui.common.ChoiceRow
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupBlock
import com.tally.app.ui.common.GroupFooter
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.SaveRefusal
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.refusalLine
import com.tally.app.ui.common.slab
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.GoalKind
import java.time.LocalDate

@Composable
fun GoalEditRoute(nav: AppNav) {
    val viewModel: GoalEditViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        GoalEditActions(
            back = nav::back,
            delete = viewModel::delete,
            setName = viewModel::setName,
            setTarget = viewModel::setTarget,
            setDate = viewModel::setDate,
            setColor = viewModel::setColor,
            setKind = viewModel::setKind,
            setAccount = viewModel::setAccount,
            setPercent = viewModel::setPercent,
            setByShare = viewModel::setByShare,
            save = viewModel::save,
        )
    }
    GoalEditScreen(state, actions)
}

/** Everything the goal editor can do, as plain lambdas. */
data class GoalEditActions(
    val back: () -> Unit = {},
    val delete: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setTarget: (String, Long?) -> Unit = { _, _ -> },
    val setDate: (LocalDate?) -> Unit = {},
    val setColor: (Int) -> Unit = {},
    val setKind: (GoalKind) -> Unit = {},
    val setAccount: (Long?) -> Unit = {},
    val setPercent: (Int) -> Unit = {},
    val setByShare: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
)

/** The four kinds in the order the editor offers them. */
private val KINDS = listOf(GoalKind.SAVINGS, GoalKind.BALANCE, GoalKind.INVEST, GoalKind.SAVE)

private val ASK_LABELS = listOf("Share of income", "Set amount")

internal fun goalKindIcon(kind: GoalKind): ImageVector = when (kind) {
    GoalKind.SAVINGS -> Icons.Rounded.Savings
    GoalKind.BALANCE -> Icons.Rounded.AccountBalance
    GoalKind.INVEST -> Icons.AutoMirrored.Rounded.TrendingUp
    GoalKind.SAVE -> Icons.Rounded.Payments
}

/**
 * The goal editor: what the goal aims for under the light in its own hue, then (for a new goal)
 * what kind of goal it is, its name, and the fields its kind needs: an amount and a date for a
 * pot or a balance, the account a balance reads, the share of income a monthly goal asks.
 */
@Composable
fun GoalEditScreen(state: GoalEditState, actions: GoalEditActions) {
    var picking by rememberSaveable { mutableStateOf(false) }
    var saveAttempts by rememberSaveable { mutableIntStateOf(0) }
    val d = state.draft
    val monthly = d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE
    Column(Modifier.fillMaxSize().imePadding().navigationBarsPadding()) {
        TopBar(onBack = actions.back) {
            if (!state.isNew) ChromeButton(Icons.Rounded.DeleteOutline, "Delete goal", actions.delete)
        }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = GUTTER, end = GUTTER, top = 4.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
        ) {
            PageTitle(
                if (state.isNew) "New goal" else "Edit goal",
                context = if (state.isNew) "Pick what it measures, then what it aims for" else goalKindLabel(d.kind),
            )
            TargetHero(state)
            if (state.loaded) {
                if (state.isNew) KindGroup(d.kind, actions.setKind)
                GoalFields(state, actions)
                when (d.kind) {
                    GoalKind.BALANCE -> AccountGroup(state, actions, allLabel = "Everything you own", allHint = "What you hold less what you owe")
                    GoalKind.INVEST -> AccountGroup(state, actions, allLabel = "Every investment account", allHint = "Moves into any account set as Investment")
                    else -> Unit
                }
                if (monthly) AskGroup(state, actions) else DateGroup(state, actions, onPick = { picking = true })
                // The category editor's swatches, which drop to four a row before a 48dp target
                // would shrink on a narrow phone.
                val colourRow: @Composable (Shape) -> Unit = { shape ->
                    GroupBlock(shape) { HueSwatches(d.color, actions.setColor) }
                }
                Group(rows = listOf(colourRow), title = "Colour")
                Column(Modifier.fillMaxWidth()) {
                    val p = state.problems
                    SaveRefusal(if (state.showErrors) refusalLine(listOf(p.name, p.target)) else null, saveAttempts)
                    HeroAction("Save goal", { saveAttempts++; actions.save() }, Modifier.fillMaxWidth())
                }
            }
        }
    }
    if (picking) {
        PlanDatePicker(
            d.targetDate ?: state.today.plusMonths(6),
            onPick = { actions.setDate(it) },
            onDismiss = { picking = false },
        )
    }
}

/** The goal as it will read: its aim as the figure, and the line, meter and chips its kind carries. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TargetHero(state: GoalEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val hue = categoryColor(d.color)
    val target = d.target ?: 0L
    HeroPanel(modifier) {
        when (d.kind) {
            GoalKind.SAVINGS -> {
                val fraction = if (target > 0L) (state.saved.toDouble() / target).toFloat() else 0f
                HeroLabel("Target", end = money.currency.currencyCode, tint = hue)
                Spacer(Modifier.height(10.dp))
                // Not a live region: the figure follows a text field, and TalkBack already echoes each key.
                HeroFigure(
                    money.formatWhole(target),
                    color = if (target > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                    description = "Target, " + money.formatWhole(target),
                )
                Text(
                    d.name.trim().ifEmpty { "Name it below" },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(14.dp))
                // The tick follows the date being edited; a new goal, an undated or a reached one has none.
                val pace = if (target > 0L && state.saved < target) goalPaceFraction(state.firstDate, d.targetDate, state.today) else null
                PaceMeter(
                    fraction,
                    pace,
                    "Holds ${money.formatWhole(state.saved)} of ${money.formatWhole(target)}" +
                        (if (target > 0L) ", " + goalMeterWords(fraction, pace) else ""),
                    height = 14.dp,
                    fill = hue,
                )
                Spacer(Modifier.height(14.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val date = d.targetDate
                    StatChip(Icons.Rounded.CalendarToday, if (date != null) "By " + Dates.short(date, state.today) else "No date")
                    if (target > 0L) StatChip(Icons.Rounded.Speed, Copy.goalLine(state.saved, target, state.monthsLeft, money))
                    if (!state.isNew) StatChip(Icons.Rounded.Savings, money.formatWhole(state.saved) + " saved")
                }
            }
            GoalKind.BALANCE -> {
                val now = state.balanceNow
                val what = d.accountId?.let { id -> state.accounts.firstOrNull { it.id == id }?.name } ?: "Everything you own"
                HeroLabel("Reach", end = money.currency.currencyCode, tint = hue)
                Spacer(Modifier.height(10.dp))
                HeroFigure(
                    money.formatWhole(target),
                    color = if (d.target != null) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                    description = "Reach " + money.formatWhole(target),
                )
                Text(
                    "$what reads ${money.format(now)} today",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(14.dp))
                val left = if (target >= now) target - now else now - target
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val date = d.targetDate
                    StatChip(Icons.Rounded.CalendarToday, if (date != null) "By " + Dates.short(date, state.today) else "No date")
                    if (d.target != null) {
                        val months = state.monthsLeft
                        StatChip(
                            Icons.Rounded.Speed,
                            when {
                                left == 0L -> "Already there"
                                months == null || months <= 0 -> money.formatWhole(left) + " to go"
                                else -> money.formatWhole((left + months - 1) / months) + " a month for " + Copy.plural(months, "month")
                            },
                        )
                    }
                }
            }
            GoalKind.INVEST, GoalKind.SAVE -> {
                val invest = d.kind == GoalKind.INVEST
                HeroLabel("Every month", end = if (d.byShare) "OF INCOME" else money.currency.currencyCode, tint = hue)
                Spacer(Modifier.height(10.dp))
                HeroFigure(
                    if (d.byShare) "${d.percent}%" else money.formatWhole(target),
                    description = if (d.byShare) "${d.percent} percent of what comes in" else money.formatWhole(target) + " a month",
                )
                Text(
                    when {
                        invest && d.byShare -> "of what comes in, moved to your investments"
                        invest -> "a month moved to your investments"
                        d.byShare -> "of what comes in, left after spending"
                        else -> "a month left after spending"
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(14.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatChip(Icons.Rounded.CalendarMonth, "Resets every month")
                    if (invest) {
                        StatChip(
                            Icons.AutoMirrored.Rounded.TrendingUp,
                            d.accountId?.let { id -> state.accounts.firstOrNull { it.id == id }?.name } ?: "Every investment account",
                        )
                    }
                }
            }
        }
    }
}

/** What the goal measures: one radio group of four rows. */
@Composable
private fun KindGroup(selected: GoalKind, onPick: (GoalKind) -> Unit) {
    Column(Modifier.selectableGroup()) {
        Group(rows = KINDS.map { kindRow(it, it == selected, onPick) }, title = "Kind")
    }
}

private fun kindRow(kind: GoalKind, selected: Boolean, onPick: (GoalKind) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        goalKindLabel(kind),
        shape,
        modifier = Modifier.semantics { this.selected = selected },
        subtitle = goalKindHint(kind),
        leading = { GlyphBadge(goalKindIcon(kind)) },
        trailing = {
            if (selected) Icon(Icons.Rounded.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        },
        chevron = false,
        onClick = { onPick(kind) },
    )
}

@Composable
private fun GoalFields(state: GoalEditState, actions: GoalEditActions) {
    val money = LocalMoney.current
    val d = state.draft
    val errors = state.showErrors
    val monthly = d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE
    Column(Modifier.fillMaxWidth()) {
        PlanField(
            d.name,
            actions.setName,
            when (d.kind) {
                GoalKind.SAVINGS -> "What it is for (a trip, a cushion)"
                GoalKind.BALANCE -> "Its name (a house fund, card paid off)"
                GoalKind.INVEST -> "Its name (invest for later)"
                GoalKind.SAVE -> "Its name (keep a fifth)"
            },
            Icons.Rounded.EditNote,
            capitalization = KeyboardCapitalization.Sentences,
            isError = errors && state.problems.name != null,
        )
        val nameProblem = state.problems.name
        if (errors && nameProblem != null) GroupFooter(nameProblem, isError = true)
        if (!monthly) {
            Spacer(Modifier.height(10.dp))
            PlanField(
                d.targetText,
                { text -> actions.setTarget(text, money.parse(text)) },
                if (d.kind == GoalKind.BALANCE) "Amount to reach" else "Target amount",
                Icons.Rounded.Payments,
                keyboardType = KeyboardType.Decimal,
                suffix = money.currency.currencyCode,
                isError = errors && state.problems.target != null,
            )
            val targetProblem = state.problems.target
            if (errors && targetProblem != null) GroupFooter(targetProblem, isError = true)
        }
    }
}

/** The account a balance or invest goal reads: every one (the default), or one picked. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AccountGroup(state: GoalEditState, actions: GoalEditActions, allLabel: String, allHint: String) {
    val d = state.draft
    val block: @Composable (Shape) -> Unit = { shape ->
        GroupBlock(shape) {
            FlowRow(Modifier.fillMaxWidth().selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ChoiceChip(allLabel, d.accountId == null) { actions.setAccount(null) }
                state.accounts.filter { !it.archived }.forEach { a ->
                    ChoiceChip(a.name, a.id == d.accountId) { actions.setAccount(a.id) }
                }
            }
        }
    }
    Group(rows = listOf(block), title = "Reads", footer = if (d.accountId == null) allHint else null)
}

/** A monthly goal's ask: a share of what comes in (stepped), or a set amount (typed). */
@Composable
private fun AskGroup(state: GoalEditState, actions: GoalEditActions) {
    val money = LocalMoney.current
    val d = state.draft
    val mode: @Composable (Shape) -> Unit = { shape ->
        ChoiceRow("Ask", ASK_LABELS, if (d.byShare) 0 else 1, { actions.setByShare(it == 0) }, shape)
    }
    val share: @Composable (Shape) -> Unit = { shape ->
        Row(
            Modifier
                .slab(shape)
                .heightIn(min = 56.dp)
                .padding(start = ROW_PAD, end = 10.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text("Share", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                Text("Of what comes in each month", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            StepButton(Icons.Rounded.Remove, "Less", enabled = d.percent > 1) { actions.setPercent(d.percent - if (d.percent > 5) 5 else 1) }
            Text(
                "${d.percent}%",
                modifier = Modifier.widthIn(min = 44.dp),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onBackground,
                textAlign = TextAlign.Center,
            )
            StepButton(Icons.Rounded.Add, "More", enabled = d.percent < 100) { actions.setPercent(d.percent + if (d.percent >= 5) 5 else 1) }
        }
    }
    Group(
        rows = listOf(mode) + if (d.byShare) listOf(share) else emptyList(),
        title = "Each month",
        footer = if (d.byShare) "Steps by 5 above 5 percent, by 1 below" else null,
    )
    if (!d.byShare) {
        Column(Modifier.fillMaxWidth()) {
            PlanField(
                d.targetText,
                { text -> actions.setTarget(text, money.parse(text)) },
                "Amount a month",
                Icons.Rounded.Payments,
                keyboardType = KeyboardType.Decimal,
                suffix = money.currency.currencyCode,
                isError = state.showErrors && state.problems.target != null,
            )
            val problem = state.problems.target
            if (state.showErrors && problem != null) GroupFooter(problem, isError = true)
        }
    }
}

/** The target date row, and a row that takes the date away once there is one. */
@Composable
private fun DateGroup(state: GoalEditState, actions: GoalEditActions, onPick: () -> Unit) {
    val date = state.draft.targetDate
    val dateRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Target date",
            shape,
            leading = { GlyphBadge(Icons.Rounded.CalendarMonth) },
            trailing = {
                Text(
                    if (date != null) Dates.short(date, state.today) else "None",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            onClick = onPick,
        )
    }
    val clearRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Remove the date",
            shape,
            leading = { GlyphBadge(Icons.Rounded.Close) },
            chevron = false,
            onClick = { actions.setDate(null) },
        )
    }
    Group(
        rows = if (date != null) listOf(dateRow, clearRow) else listOf(dateRow),
        title = "Date",
        footer = if (date == null) "Without a date it shows what is left, not a monthly pace" else null,
    )
}
