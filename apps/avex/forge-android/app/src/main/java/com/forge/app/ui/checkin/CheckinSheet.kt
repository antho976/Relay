package com.forge.app.ui.checkin

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
 * The morning check-in, hosted once at the app root: the [CheckinPromptBanner] offers it, the
 * [CheckinSheet] answers it. Nothing here opens itself — the sheet used to, and arriving unasked
 * over whatever the app was opened for is what made a four-tap question feel like a toll gate.
 */
@Composable
fun CheckinHost(viewModel: CheckinViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    CheckinPromptBanner(
        visible = state.prompting,
        onOpen = viewModel::open,
        onDismiss = viewModel::skip
    )
    CheckinSheet(state, viewModel)
}

/**
 * The morning check-in sheet (Coach v3 B1) — the coach's only daily question, and the capture point
 * for illness and per-muscle soreness.
 *
 * Modal archetype (§3): surface fill, large top corners, one decision per row, nothing hidden
 * behind a tap. Four 1–5 scales in the app's existing pill vocabulary, and every one of them
 * optional — a partial answer beats an abandoned form, and "Skip" is always one tap away.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CheckinSheet(state: CheckinViewModel.UiState, viewModel: CheckinViewModel) {
    if (!state.visible) return

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    ModalBottomSheet(
        onDismissRequest = viewModel::skip,
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
                "This morning",
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
                    label = { Text("Weight") },
                    placeholder = { Text("optional") },
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
                    "Skip today",
                    style = MaterialTheme.typography.bodyMedium,
                    color = muted,
                    modifier = Modifier
                        .clickableLabeled("Skip today's check-in") { viewModel.skip() }
                        .padding(vertical = 8.dp, horizontal = 4.dp)
                )
                ForgePrimaryCapsule(if (state.answeredToday) "Update" else "Save", onClick = viewModel::save)
            }
        }
    }
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
