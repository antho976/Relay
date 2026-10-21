package com.forge.app.ui.checkin

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import kotlin.math.roundToInt
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.toggleableLabeled
import com.forge.app.ui.goals.OpenField
import com.forge.app.ui.goals.OpenSwitchMark
import com.forge.app.ui.goals.OpenValueSlider
import com.forge.app.ui.goals.OpenWideButton
import com.forge.app.ui.theme.NoticeType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.bounceClick

/**
 * The daily check-in sheet (Coach v3 B1), opened from its notification and used to capture illness and
 * per-muscle soreness without interrupting app launch.
 *
 * Modal archetype (§3): surface fill, large top corners, one decision per row, nothing hidden
 * behind a tap. Every answer is optional — a partial answer beats an abandoned form, and closing it
 * changes nothing.
 *
 * Grouped-surface form since 2026-09-27, drawn like onboarding's "Anything else?" page: the four
 * 1–5 scales are one group of rows, each a label with a sliding segmented control at its end; the
 * sore muscles are a connected tile grid (onboarding's sore-spots shape); and the unwell flag and
 * the weigh-in are one closing group, a switch row and an inline field.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CheckinSheet(viewModel: CheckinViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    if (!state.visible) return

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = viewModel::close,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        CheckinContent(
            state = state,
            onSleep = viewModel::setSleep,
            onSoreness = viewModel::setSoreness,
            onStress = viewModel::setStress,
            onDrive = viewModel::setMotivation,
            onToggleMuscle = viewModel::toggleMuscle,
            onSick = viewModel::setSick,
            onWeight = viewModel::setWeightText,
            onClose = viewModel::close,
            onSave = viewModel::save
        )
    }
}

/**
 * The sheet's body (2026-10-21 redesign, "pg-notif ck"), stateless so it can be shot without the
 * ViewModel: a slider per question like the goal target, its answer named above it and its ends
 * named below; the muscle picker once soreness is real; unwell and the optional weigh-in; then what
 * these answers do to today, live, over Later and Save.
 */
@Composable
internal fun CheckinContent(
    state: CheckinViewModel.UiState,
    onSleep: (Int) -> Unit,
    onSoreness: (Int) -> Unit,
    onStress: (Int) -> Unit,
    onDrive: (Int) -> Unit,
    onToggleMuscle: (MuscleGroup) -> Unit,
    onSick: (Boolean) -> Unit,
    onWeight: (String) -> Unit,
    onClose: () -> Unit,
    onSave: () -> Unit
) {
    val focus = LocalFocusManager.current
    val ink = MaterialTheme.colorScheme.onBackground
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(horizontal = 24.dp)
            .navigationBarsPadding()
            .padding(top = 4.dp, bottom = 28.dp)
    ) {
        Text("Today's check-in", style = NoticeType.sheetTitle, color = ink, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(6.dp))
        OpenLabel("Answer what you know, skip the rest", style = OpenType.monoXS)
        Spacer(Modifier.height(24.dp))
        Column(verticalArrangement = Arrangement.spacedBy(22.dp)) {
            QuestionSlider(CheckinQuestion.SLEEP, state.sleepQuality, onSleep)
            QuestionSlider(CheckinQuestion.SORENESS, state.soreness, onSoreness)
            // Only asked once soreness is real: one generic tap can't gate a muscle, but a menu
            // nobody needs is worse than no gate at all.
            if (state.askWhichMuscles) {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    OpenLabel("Where · ${state.soreMuscles.size} sore", style = OpenType.monoXS)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        MuscleGroup.entries.forEach { m ->
                            MuscleChip(m.displayName, m in state.soreMuscles) { onToggleMuscle(m) }
                        }
                    }
                }
            }
            QuestionSlider(CheckinQuestion.STRESS, state.stress, onStress)
            QuestionSlider(CheckinQuestion.DRIVE, state.motivation, onDrive)
        }
        Spacer(Modifier.height(22.dp))
        Row(
            Modifier.fillMaxWidth().toggleableLabeled("Feeling unwell", state.sick) { onSick(!state.sick) },
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text("Feeling unwell", style = OpenType.rowTitle, color = ink)
                OpenLabel("Nothing is pushed until it passes", style = OpenType.monoXS)
            }
            OpenSwitchMark(state.sick)
        }
        Spacer(Modifier.height(18.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Weight", style = OpenType.rowTitle, color = ink, modifier = Modifier.weight(1f))
            // The unit rides the field because the field cannot infer it: the value goes straight
            // into the bodyweight trend, and a kg user typing 80 into a box that said only "Weight"
            // logged 80 lb.
            OpenField(
                value = state.weightText,
                onValueChange = onWeight,
                label = "Weight in ${unitLabel(state.weightUnit)}",
                placeholder = "Optional",
                suffix = unitLabel(state.weightUnit),
                isError = state.weightInvalid,
                width = 130.dp,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
            )
        }
        if (state.weightInvalid) {
            // A typed value outside the plausible range used to vanish on Save while the check-in
            // reported success (M-14). Say why, and Save holds until it is fixed.
            Spacer(Modifier.height(6.dp))
            Text(bodyweightRangeText(state.weightUnit), style = OpenType.bodyS, color = MaterialTheme.colorScheme.error)
        }
        Spacer(Modifier.height(22.dp))
        val percent = checkinEffectPercent(state.sleepQuality, state.soreness, state.stress, state.motivation, state.sick)
        Row(
            Modifier.fillMaxWidth().semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(Modifier.size(8.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
            Text(checkinEffectLine(percent, state.sick), style = OpenType.rowTitle, color = ink)
        }
        Spacer(Modifier.height(18.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenWideButton("Later", filled = false, onClick = onClose, modifier = Modifier.width(110.dp))
            OpenWideButton(if (state.answeredToday) "Update" else "Save", filled = true, onClick = onSave, modifier = Modifier.weight(1f))
        }
    }
}

/** One question: its name and named answer, the five-stop slider, and its ends. */
@Composable
private fun QuestionSlider(q: CheckinQuestion, value: Int?, onPick: (Int) -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val answer = value?.let { q.answers[(it - 1).coerceIn(0, 4)] }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text(q.label, style = OpenType.rowTitle, color = ink)
            OpenLabel(answer ?: "Not set", style = OpenType.monoXS, color = if (answer != null) ink else MaterialTheme.colorScheme.onSurfaceVariant)
        }
        OpenValueSlider(
            fraction = value?.let { (it - 1) / 4f },
            onFraction = { f -> onPick((f * 4).roundToInt() + 1) },
            description = q.label,
            stateText = answer ?: "Not answered",
            stops = 5,
            height = 24.dp,
            knob = 22.dp,
        )
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.SpaceBetween) {
            OpenLabel(q.low, style = OpenType.monoXS)
            OpenLabel(q.high, style = OpenType.monoXS)
        }
    }
}

/** A sore-muscle choice: a quiet outline, ringed red and inked once picked. */
@Composable
private fun MuscleChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(4.dp)
    Text(
        label,
        style = OpenType.bodyS,
        color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.Checkbox }
            .border(1.dp, if (selected) MaterialTheme.colorScheme.primary else OpenColors.Rule, shape)
            .padding(horizontal = 12.dp, vertical = 8.dp)
    )
}

/** The plausible range in the field's own unit, the same line the profile's weigh-in sheet shows. */
private fun bodyweightRangeText(unit: WeightUnit): String =
    if (unit == WeightUnit.ST) {
        "Enter ${formatWeight(MIN_BODYWEIGHT_LB, unit)}–${formatWeight(MAX_BODYWEIGHT_LB, unit)}."
    } else {
        val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, unit).roundToInt()
        val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, unit).roundToInt()
        "Enter $minDisp–$maxDisp ${unitLabel(unit)}."
    }
