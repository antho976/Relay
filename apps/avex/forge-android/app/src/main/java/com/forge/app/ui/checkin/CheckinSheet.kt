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
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.clickableLabeled

/**
 * The daily check-in sheet (Coach v3 B1), opened from its notification and used to capture illness and
 * per-muscle soreness without interrupting app launch.
 *
 * Modal archetype (§3): surface fill, large top corners, one decision per row, nothing hidden
 * behind a tap. Four 1–5 scales in the app's existing pill vocabulary, and every one of them
 * optional — a partial answer beats an abandoned form, and closing it changes nothing.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CheckinSheet(viewModel: CheckinViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    if (!state.visible) return

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    ModalBottomSheet(
        onDismissRequest = viewModel::close,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .navigationBarsPadding()
                .padding(bottom = 16.dp)
        ) {
            Text(
                "Today's check-in",
                style = MaterialTheme.typography.headlineSmall,
                color = onBg
            )
            Spacer(Modifier.height(2.dp))
            Text(
                "Shapes today's targets. Answer what you know, skip the rest.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
            Spacer(Modifier.height(18.dp))

            ScaleRow("Sleep", state.sleepQuality, viewModel::setSleep, onBg, muted, accent, outline)
            ScaleRow("Soreness", state.soreness, viewModel::setSoreness, onBg, muted, accent, outline)
            ScaleRow("Stress", state.stress, viewModel::setStress, onBg, muted, accent, outline)
            ScaleRow("Drive", state.motivation, viewModel::setMotivation, onBg, muted, accent, outline)

            // Only asked once soreness is real — one generic tap can't gate a muscle, but a menu
            // nobody needs is worse than no gate at all.
            if (state.askWhichMuscles) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "WHERE",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp,
                    letterSpacing = 1.5.sp
                )
                Spacer(Modifier.height(8.dp))
                androidx.compose.foundation.layout.FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    MuscleGroup.entries.forEach { m ->
                        SegmentPill(
                            text = m.displayName,
                            selected = m in state.soreMuscles,
                            onClick = { viewModel.toggleMuscle(m) },
                            accent = accent, onBg = onBg, muted = muted, outline = outline
                        )
                    }
                }
            }

            Spacer(Modifier.height(14.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                SegmentPill(
                    text = "Feeling unwell",
                    selected = state.sick,
                    onClick = { viewModel.setSick(!state.sick) },
                    accent = accent, onBg = onBg, muted = muted, outline = outline
                )
                Spacer(Modifier.width(12.dp))
                OutlinedTextField(
                    value = state.weightText,
                    onValueChange = viewModel::setWeightText,
                    // The unit is in the label because the field cannot infer it: the value goes
                    // straight into the bodyweight trend, and a kg user typing 80 into a box that
                    // said only "Weight" logged 80 lb.
                    label = { Text("Weight (${unitLabel(state.weightUnit)})") },
                    placeholder = { Text("optional") },
                    // A typed value outside the plausible range used to vanish on Save while the
                    // check-in reported success (M-14). Say why, and Save holds until it is fixed.
                    isError = state.weightInvalid,
                    supportingText = if (state.weightInvalid) {
                        { Text(bodyweightRangeText(state.weightUnit)) }
                    } else null,
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.weight(1f)
                )
            }

            Spacer(Modifier.height(18.dp))
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "Close",
                    style = MaterialTheme.typography.bodyMedium,
                    color = muted,
                    modifier = Modifier
                        .clickableLabeled("Close check-in") { viewModel.close() }
                        .padding(vertical = 8.dp, horizontal = 4.dp)
                )
                ForgePrimaryCapsule(if (state.answeredToday) "Update" else "Save", onClick = viewModel::save)
            }
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

/** One 1–5 question: its mono label, then five pills. The pills ARE the input (§13). */
@Composable
private fun ScaleRow(
    label: String,
    value: Int?,
    onPick: (Int) -> Unit,
    onBg: androidx.compose.ui.graphics.Color,
    muted: androidx.compose.ui.graphics.Color,
    accent: androidx.compose.ui.graphics.Color,
    outline: androidx.compose.ui.graphics.Color
) {
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            letterSpacing = 1.5.sp
        )
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            (1..5).forEach { n ->
                SegmentPill(
                    text = n.toString(),
                    selected = value == n,
                    onClick = { onPick(n) },
                    accent = accent, onBg = onBg, muted = muted, outline = outline
                )
            }
        }
    }
}
