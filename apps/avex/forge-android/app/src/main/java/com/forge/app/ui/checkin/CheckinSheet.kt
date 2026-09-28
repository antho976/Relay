package com.forge.app.ui.checkin

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import kotlin.math.roundToInt
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
import androidx.compose.ui.platform.LocalDensity
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
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgeChoiceRow
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeSwitchRow
import com.forge.app.ui.common.ForgeTileGrid
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

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

/** The sheet's body, stateless so it can be previewed and shot without the ViewModel. */
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
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(horizontal = 24.dp)
            .navigationBarsPadding()
            .padding(bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                "Today's check-in",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onBackground
            )
            ForgeGroupCaption("Shapes today's targets. Answer what you know, skip the rest.")
        }

        ForgeRowGroup(
            { ScaleRow("Sleep", state.sleepQuality, onSleep) },
            { ScaleRow("Soreness", state.soreness, onSoreness) },
            { ScaleRow("Stress", state.stress, onStress) },
            { ScaleRow("Drive", state.motivation, onDrive) }
        )

        // Only asked once soreness is real — one generic tap can't gate a muscle, but a menu
        // nobody needs is worse than no gate at all.
        if (state.askWhichMuscles) {
            ForgeGroupSection("Where", meta = "${state.soreMuscles.size} sore") {
                // Two across at a large font scale, so a long name wraps in a wide cell rather than a narrow one.
                ForgeTileGrid(MuscleGroup.entries, cols = if (LocalDensity.current.fontScale > 1.3f) 2 else 3) { m, corners, modifier ->
                    MuscleTile(
                        label = m.displayName,
                        selected = m in state.soreMuscles,
                        corners = corners,
                        onClick = { onToggleMuscle(m) },
                        modifier = modifier
                    )
                }
            }
        }

        ForgeGroupSection(
            label = null,
            footer = if (state.weightInvalid) {
                // A typed value outside the plausible range used to vanish on Save while the
                // check-in reported success (M-14). Say why, and Save holds until it is fixed.
                {
                    Text(
                        bodyweightRangeText(state.weightUnit),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error
                    )
                }
            } else null
        ) {
            ForgeRowGroup(
                {
                    ForgeSwitchRow(
                        label = "Feeling unwell",
                        description = "Nothing is pushed until it passes.",
                        checked = state.sick,
                        onToggle = onSick
                    )
                },
                {
                    // The unit rides the row because the field cannot infer it: the value goes
                    // straight into the bodyweight trend, and a kg user typing 80 into a box that
                    // said only "Weight" logged 80 lb.
                    ForgeFieldRow(
                        label = "Weight",
                        value = state.weightText,
                        onValueChange = onWeight,
                        placeholder = "Optional",
                        suffix = unitLabel(state.weightUnit),
                        isError = state.weightInvalid,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
                    )
                }
            )
        }

        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            ForgeSecondaryCapsule("Close", onClick = onClose, modifier = Modifier.weight(1f))
            ForgePrimaryCapsule(
                if (state.answeredToday) "Update" else "Save",
                onClick = onSave,
                modifier = Modifier.weight(1f)
            )
        }
    }
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

/** One 1–5 question: its label, and a sliding 1–5 control at the row's end. The control IS the
 *  input (§13); no thumb is drawn until it is answered. */
@Composable
private fun ScaleRow(label: String, value: Int?, onPick: (Int) -> Unit) {
    ForgeChoiceRow(
        label = label,
        options = SCALE,
        selectedIndex = value?.let { it - 1 } ?: -1,
        onSelect = { onPick(it + 1) }
    )
}

private val SCALE = (1..5).map(Int::toString)

/**
 * A sore-muscle tile: a name on one member of the connected grid. Pick-many, so it keeps the
 * group's shape when lit (the ring and wash say it is on) rather than rounding out of it.
 */
@Composable
private fun MuscleTile(
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected = false)
    Text(
        label,
        style = MaterialTheme.typography.bodyMedium,
        color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.Checkbox }
            .padding(horizontal = 14.dp, vertical = 16.dp)
    )
}
