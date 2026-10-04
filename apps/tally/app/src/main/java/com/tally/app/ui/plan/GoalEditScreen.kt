package com.tally.app.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.Defaults
import java.time.LocalDate

/** The twelve category hues by name, for the swatches' spoken labels. Same order as the palette. */
private val HUE_NAMES = listOf(
    "Sage", "Teal", "Sky", "Iris", "Orchid", "Rose", "Coral", "Amber", "Olive", "Sand", "Slate", "Brick",
)

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
    val save: () -> Unit = {},
)

/**
 * The goal editor: the target under the light in the goal's own hue with the monthly pace it
 * implies, then the name, the amount, an optional date and the colour.
 */
@Composable
fun GoalEditScreen(state: GoalEditState, actions: GoalEditActions) {
    var picking by rememberSaveable { mutableStateOf(false) }
    val d = state.draft
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
                context = "A target, and a date if it has one",
            )
            TargetHero(state)
            if (state.loaded) {
                GoalFields(state, actions)
                DateGroup(state, actions, onPick = { picking = true })
                val colourRow: @Composable (Shape) -> Unit = { shape ->
                    GroupBlock(shape) { HueSwatches(d.color, actions.setColor) }
                }
                Group(rows = listOf(colourRow), title = "Colour")
                HeroAction("Save goal", actions.save, Modifier.fillMaxWidth())
                if (state.showErrors && state.problems.any) {
                    GroupFooter("Fill in what is marked above to save", isError = true)
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

/** The target as the figure, the meter of what the goal already holds, and the pace a date implies. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TargetHero(state: GoalEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val hue = categoryColor(d.color)
    val target = d.target ?: 0L
    val fraction = if (target > 0L) (state.saved.toDouble() / target).toFloat() else 0f
    HeroPanel(modifier) {
        HeroLabel(Icons.Rounded.Flag, "Target", end = money.currency.currencyCode, tint = hue)
        Spacer(Modifier.height(10.dp))
        HeroFigure(
            money.formatWhole(target),
            color = if (target > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            description = "Target, " + money.formatWhole(target),
            live = true,
        )
        Text(
            d.name.trim().ifEmpty { "Name it below" },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(14.dp))
        PaceMeter(
            fraction,
            null,
            "Holds ${money.formatWhole(state.saved)} of ${money.formatWhole(target)}",
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
}

@Composable
private fun GoalFields(state: GoalEditState, actions: GoalEditActions) {
    val money = LocalMoney.current
    val d = state.draft
    val errors = state.showErrors
    Column(Modifier.fillMaxWidth()) {
        PlanField(
            d.name,
            actions.setName,
            "What it is for (a trip, a cushion)",
            Icons.Rounded.EditNote,
            capitalization = KeyboardCapitalization.Sentences,
            isError = errors && state.problems.name != null,
        )
        val nameProblem = state.problems.name
        if (errors && nameProblem != null) GroupFooter(nameProblem, isError = true)
        Spacer(Modifier.height(10.dp))
        PlanField(
            d.targetText,
            { text -> actions.setTarget(text, money.parse(text)) },
            "Target amount",
            Icons.Rounded.Payments,
            keyboardType = KeyboardType.Decimal,
            suffix = money.currency.currencyCode,
            isError = errors && state.problems.target != null,
        )
        val targetProblem = state.problems.target
        if (errors && targetProblem != null) GroupFooter(targetProblem, isError = true)
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

/** Twelve hue circles in two rows of six; the picked one wears a ring and a check. One radio group. */
@Composable
private fun HueSwatches(selected: Int, onPick: (Int) -> Unit) {
    val rows = remember { (0 until Defaults.PALETTE_SIZE).toList().chunked(6) }
    Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        rows.forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                row.forEach { i ->
                    HueSwatch(categoryColor(i), HUE_NAMES.getOrElse(i) { "Colour ${i + 1}" }, i == selected) { onPick(i) }
                }
            }
        }
    }
}

@Composable
private fun HueSwatch(color: Color, label: String, selected: Boolean, onClick: () -> Unit) {
    val ring = MaterialTheme.colorScheme.onBackground
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .semantics(mergeDescendants = true) { contentDescription = label }
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .then(if (selected) Modifier.border(2.dp, ring, CircleShape) else Modifier)
            .padding(5.dp)
            .clip(CircleShape)
            .background(color),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Icon(
                Icons.Rounded.Check,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.background,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}
