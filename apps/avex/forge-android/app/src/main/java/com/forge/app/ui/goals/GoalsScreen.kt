package com.forge.app.ui.goals

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.goal.HOME_PIN_SLOTS
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.GoalsColors
import com.forge.app.ui.theme.GoalsType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale

/**
 * The Goals page (2026-10-21 redesign, "pg-goals gl"): from View all on Home. Each live goal is a
 * reading with its pace (name, where it stands over its target, the meter, and when it lands at this
 * pace); the bookmark pins it to Home's three; reached goals fold below with when they were met.
 *
 * Adding and editing happen on the routed [GoalEditorScreen]; this list refreshes reactively when
 * you come back from it.
 */
@Composable
fun GoalsScreen(
    onBack: () -> Unit,
    onAddGoal: () -> Unit,
    onEditLift: (exerciseId: String) -> Unit,
    onEditCustom: (goalId: Long) -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val pinned by viewModel.pinnedGoals.collectAsStateWithLifecycle()
    val settings = LocalForgeSettings.current
    val locale = currentLocale()
    Scaffold(
        topBar = {
            OpenBackBar(onBack) { OpenTopBarIcon(OpenIcons.Plus, "Add a goal", onAddGoal) }
        },
        containerColor = Color.Transparent
    ) { inner ->
        // No spinners: the local read is instant, the list simply appears when state lands.
        if (state.loading) {
            Box(Modifier.fillMaxSize().padding(inner))
            return@Scaffold
        }
        GoalsPage(
            state = state,
            pinned = pinned,
            weightUnit = settings.weightUnit,
            useMiles = settings.useMiles,
            locale = locale,
            onAdd = onAddGoal,
            onOpen = { ref ->
                when (ref) {
                    is GoalRef.Lift -> onEditLift(ref.exerciseId)
                    is GoalRef.Custom -> onEditCustom(ref.id)
                }
            },
            onTogglePin = viewModel::togglePinKey,
            modifier = Modifier.padding(inner)
        )
    }
}

/** The page body, free of the ViewModel so the goldens render exactly what the app does. */
@Composable
internal fun GoalsPage(
    state: GoalsViewModel.UiState,
    pinned: List<String>,
    weightUnit: WeightUnit,
    useMiles: Boolean,
    locale: Locale,
    onAdd: () -> Unit,
    onOpen: (GoalRef) -> Unit,
    onTogglePin: (String) -> Unit,
    modifier: Modifier = Modifier,
    zone: ZoneId = ZoneId.systemDefault(),
) {
    val today = remember(state.todayStartMs) {
        if (state.todayStartMs > 0) Instant.ofEpochMilli(state.todayStartMs).atZone(zone).toLocalDate() else LocalDate.now(zone)
    }
    val lines = remember(state, weightUnit, useMiles, locale, today) {
        state.liftGoals.map { g ->
            liftLine(g, state.liftChange30d[g.exerciseId], state.liftReachedAtMs[g.exerciseId], weightUnit, locale, zone)
        } + state.customGoals.map { g -> customLine(g, state.weighIns, weightUnit, useMiles, today) }
    }
    val active = remember(lines) { lines.filter { !it.achieved }.sortedByDescending { it.fraction } }
    val reached = remember(lines) { lines.filter { it.achieved }.sortedByDescending { it.reachedAtMs } }
    val livePins = lines.map { it.pinKey }.toSet()
    val pinnedCount = pinned.count { it in livePins }
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    LazyColumn(
        modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 64.dp)
    ) {
        item("head") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text(
                    "Goals",
                    style = OpenType.title,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.semantics { heading() }
                )
                Spacer(Modifier.height(8.dp))
                // Wraps between its parts, never inside one ("2 reached" stays together).
                val parts = goalsMetaLine(active.size, pinnedCount, reached.size).split(" · ")
                androidx.compose.foundation.layout.FlowRow(Modifier.semantics(mergeDescendants = true) { }) {
                    parts.forEachIndexed { i, part ->
                        OpenLabel(if (i < parts.lastIndex) "$part · " else part, style = OpenType.monoXS)
                    }
                }
                Spacer(Modifier.height(36.dp))
                HeadRow("Active", if (active.isEmpty()) null else "Pace")
                Spacer(Modifier.height(18.dp))
            }
        }
        if (active.isEmpty()) {
            item("empty") {
                // An honest zero: the bare meter where a goal's would be, and what fills it.
                Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    OpenMeter(0f)
                    OpenLabel(
                        if (reached.isEmpty()) "Add a goal and its pace shows here" else "Every goal is reached. Add the next one",
                        style = OpenType.monoXS
                    )
                    OpenSmallButton("Add a goal", filled = true, onClick = onAdd, icon = OpenIcons.Plus)
                }
            }
        } else {
            items(active, key = { it.pinKey }) { line ->
                GoalReadingRow(
                    line = line,
                    pinned = line.pinKey in pinned,
                    pinsFull = pinnedCount >= HOME_PIN_SLOTS,
                    onOpen = { onOpen(line.ref) },
                    onTogglePin = { onTogglePin(line.pinKey) },
                    modifier = Modifier.padding(bottom = 26.dp)
                )
            }
        }
        if (reached.isNotEmpty()) {
            item("reached-head") {
                Column {
                    Spacer(Modifier.height(18.dp))
                    HeadRow("Reached", "${reached.size}")
                    Spacer(Modifier.height(16.dp))
                }
            }
            items(reached, key = { "done-" + it.pinKey }) { line ->
                ReachedRow(line, onOpen = { onOpen(line.ref) }, modifier = Modifier.padding(bottom = 16.dp))
            }
        }
    }
}

@Composable
private fun HeadRow(left: String, right: String?) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(left, Modifier.semantics { heading() })
        if (right != null) OpenLabel(right)
    }
}

/**
 * One live goal: its name with the Home bookmark, the reading with its pace column, and the meter.
 * The row opens the goal's editor; the bookmark is its own 48dp target and toggles the pin.
 */
@Composable
private fun GoalReadingRow(
    line: GoalLine,
    pinned: Boolean,
    pinsFull: Boolean,
    onOpen: () -> Unit,
    onTogglePin: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val spoken = listOfNotNull(
        line.title,
        "${line.figure}${line.denominator}",
        "${(line.fraction * 100).toInt()} percent",
        line.paceTop,
        line.paceBottom,
    ).joinToString(", ")
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                line.title,
                style = OpenType.rowTitle,
                color = ink,
                modifier = Modifier
                    .weight(1f)
                    .clickableLabeled("Edit ${line.title}", onClick = onOpen)
                    .semantics { contentDescription = spoken }
            )
            Box(
                Modifier
                    .touchSlop(vertical = 16.dp, horizontal = 16.dp)
                    .clickableLabeled(
                        if (pinned) "Unpin from Home" else if (pinsFull) "Pin to Home, replacing the oldest pin" else "Pin to Home",
                        role = Role.Switch,
                        onClick = onTogglePin
                    )
                    .semantics {
                        contentDescription = "Pinned to Home"
                        toggleableState = ToggleableState(pinned)
                        stateDescription = if (pinned) "On" else "Off"
                    }
                    .padding(16.dp)
            ) {
                Icon(
                    OpenIcons.Bookmark,
                    contentDescription = null,
                    tint = if (pinned) accent else OpenColors.Rule,
                    modifier = Modifier.size(16.dp)
                )
            }
        }
        Row(
            Modifier
                .fillMaxWidth()
                .clickableLabeled("Edit ${line.title}", onClick = onOpen)
                .clearAndSetSemantics { },
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            Text(
                buildAnnotatedString {
                    append(line.figure)
                    pushStyle(GoalsType.denominator.toSpanStyle().copy(color = muted))
                    append(line.denominator)
                    pop()
                },
                style = GoalsType.figure,
                color = ink,
                modifier = Modifier.weight(1f)
            )
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                OpenLabel(line.paceTop, style = OpenType.monoXS, color = if (line.paceTopMoved) ink else muted)
                line.paceBottom?.let { OpenLabel(it, style = OpenType.monoXS, color = if (line.paceBottomMoved) ink else muted) }
            }
        }
        OpenMeter(line.fraction, Modifier.clearAndSetSemantics { })
    }
}

/** A reached goal, folded: a red check, what it was, and when it was met. Still opens its editor. */
@Composable
private fun ReachedRow(line: GoalLine, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier
            .fillMaxWidth()
            .clickableLabeled("Edit ${line.reachedTitle}", onClick = onOpen)
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull("Reached", line.reachedTitle, line.reachedWhen).joinToString(", ")
            },
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(OpenIcons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(16.dp))
        Text(line.reachedTitle, style = GoalsType.reached, color = GoalsColors.Reached, modifier = Modifier.weight(1f))
        line.reachedWhen?.let { OpenLabel(it, style = OpenType.monoXS) }
    }
}
