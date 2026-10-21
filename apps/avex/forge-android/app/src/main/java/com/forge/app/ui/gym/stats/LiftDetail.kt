package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.DirectiveRepository
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.Program
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenRangeSwitch
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.common.window.ModalBottomSheet
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import java.time.Instant
import java.time.ZoneId
import javax.inject.Inject

/** The lift a lift page shows, and the coach's next target for it. */
@HiltViewModel
class LiftDetailViewModel @Inject constructor(
    savedState: SavedStateHandle,
    directiveRepo: DirectiveRepository,
) : ViewModel() {
    val liftId: String = savedState.get<String>(com.forge.app.ui.nav.Routes.ARG_LIFT_ID).orEmpty()

    val next: StateFlow<DirectiveRepository.NextLiftTarget?> =
        flow { emit(directiveRepo.nextTargetFor(liftId)) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

/**
 * One lift's own page (Stats → Every lift → a row): its e1RM over the range with the span before
 * under it, the coach's next target, and its top set from each session, newest first.
 */
@Composable
fun LiftDetailScreen(
    onBack: () -> Unit,
    onSetGoal: (String) -> Unit = {},
    viewModel: LiftDetailViewModel = hiltViewModel(),
    statsViewModel: StatsViewModel = hiltViewModel(),
) {
    val state by statsViewModel.state.collectAsStateWithLifecycle()
    val next by viewModel.next.collectAsStateWithLifecycle()
    val unit = LocalForgeSettings.current.weightUnit
    val id = viewModel.liftId
    var range by rememberSaveable { mutableStateOf(StatsRange.W12) }
    val now = remember(state) { endOfToday(System.currentTimeMillis()) }
    val window = remember(range, state.statSets, now) { statsWindow(range, now, state.statSets.firstOrNull()?.sessionStartedAt) }
    val sessions = remember(state.statSets, id) { liftSessions(state.statSets.filter { it.exerciseId == id })[id].orEmpty() }
    val name = state.e1rmLifts.firstOrNull { it.exerciseId == id }?.exerciseName ?: Program.exerciseDisplayName(id)
    val trend = remember(sessions, window, name) { liftTrend(id, sessions, window)?.copy(name = name) }
    val ink = MaterialTheme.colorScheme.onBackground
    val unitText = unitLabel(unit)
    var menu by remember { mutableStateOf(false) }

    LazyColumn(
        Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
                Spacer(Modifier.weight(1f))
                Box {
                    OpenTopBarIcon(OpenIcons.MoreVertical, "More for $name", { menu = true })
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text("Set a goal for this lift") }, onClick = { menu = false; onSetGoal(id) })
                    }
                }
            }
        }
        item("title") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text(name, style = OpenType.titleM, color = ink)
                Spacer(Modifier.height(10.dp))
                liftMeta(id)?.let { OpenLabel(it) }
                Spacer(Modifier.height(10.dp))
                OpenRangeSwitch(StatsRange.entries, range, { it.label }, { range = it })
                Spacer(Modifier.height(22.dp))
            }
        }
        if (trend == null) {
            item("empty") {
                Text("No weighted sets of this lift yet.", style = OpenType.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            return@LazyColumn
        }
        item("figures") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OpenLabel("Est. 1RM")
                    Text(figure(w(trend.now, unit), " $unitText", OpenType.figureL, OpenType.figureMDenominator), style = OpenType.figureL, color = ink)
                    liftChangeLine(trend, range) { "${w(it, unit)} $unitText" }?.let {
                        OpenLabel(it, style = OpenType.monoSTight, color = if (trend.flatWeeks == null) ink else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OpenLabel("Next session")
                    val n = next
                    if (n == null || n.weightLb == null) {
                        Text(figure("–", "", OpenType.figureL, OpenType.figureMDenominator), style = OpenType.figureL, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        OpenLabel(if (n == null) "Not in your plan" else "${n.dayName} · ${n.repsText} reps", style = OpenType.monoSTight)
                    } else {
                        val reps = n.repsText.substringBefore("-").substringBefore("–").trim()
                        Text(figure(w(n.weightLb, unit), "×$reps", OpenType.figureL, OpenType.figureMDenominator), style = OpenType.figureL, color = ink)
                        OpenLabel("${n.dayName} · coach", style = OpenType.monoSTight)
                    }
                }
            }
        }
        item("graph") {
            Column {
                Spacer(Modifier.height(20.dp))
                LiftGraph(trend, state, window, range, unit)
                Spacer(Modifier.height(48.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    OpenLabel("Top set each session")
                    OpenLabel("Est. 1RM")
                }
                Spacer(Modifier.height(22.dp))
            }
        }
        val rows = trend.current.reversed()
        items(rows, key = { it.t }) { s ->
            TopSetRow(s, unit, Modifier.padding(bottom = 22.dp))
        }
    }
}

/** "Barbell · chest" from the library: the first piece of kit, then the muscle it trains. */
private fun liftMeta(id: String): String? {
    val ex = Program.exercise(id) ?: return null
    return listOfNotNull(ex.equipment.firstOrNull()?.display, ex.muscle.displayName.lowercase()).joinToString(" · ").ifBlank { null }
}

@Composable
private fun TopSetRow(s: LiftSession, unit: WeightUnit, modifier: Modifier = Modifier) {
    val ink = MaterialTheme.colorScheme.onBackground
    val date = Instant.ofEpochMilli(s.t).atZone(ZoneId.systemDefault()).toLocalDate()
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.width(34.dp)) {
            OpenLabel(date.dayOfWeek.getDisplayName(java.time.format.TextStyle.SHORT, com.forge.app.ui.common.currentLocale()), style = OpenType.monoXS)
            Text("${date.dayOfMonth}", style = OpenType.figureS, color = ink)
        }
        Text(
            figure(w(s.weightLb, unit), "×${s.reps}", OpenType.figureSet, OpenType.figureSetUnit),
            style = OpenType.figureSet, color = ink, modifier = Modifier.weight(1f)
        )
        Text(w(s.e1rm, unit), style = OpenType.monoM, color = if (s.pr) ink else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Which lift the Strength graph shows: every lift trained in the range, strongest first. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LiftPickerSheet(
    lifts: List<LiftTrend>,
    selectedId: String,
    unit: WeightUnit,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp),
        dragHandle = {
            Box(Modifier.padding(top = 12.dp, bottom = 10.dp).width(36.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(OpenColors.Rule))
        }
    ) {
        Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 28.dp).navigationBarsPadding()) {
            Text("Show on the graph", style = OpenType.sheetTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.padding(start = 8.dp, top = 8.dp, bottom = 14.dp))
            LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(lifts, key = { it.exerciseId }) { t ->
                    val on = t.exerciseId == selectedId
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(4.dp))
                            .then(if (on) Modifier.background(MaterialTheme.colorScheme.primaryContainer) else Modifier)
                            .bounceClick(role = Role.RadioButton) { onPick(t.exerciseId) }
                            .semantics { selected = on }
                            .padding(horizontal = 12.dp, vertical = 14.dp),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(t.name, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
                        OpenLabel("${w(t.now, unit)} ${unitLabel(unit)}", style = OpenType.monoSTight, color = if (on) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
                        if (on) Icon(OpenIcons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
                    }
                }
            }
        }
    }
}
