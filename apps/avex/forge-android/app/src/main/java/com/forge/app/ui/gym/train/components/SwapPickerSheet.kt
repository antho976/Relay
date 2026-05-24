package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExercisePlan
import com.forge.app.program.Swap
import com.forge.app.program.Swaps

/**
 * Modal bottom sheet showing swap candidates for [forExercise]'s muscle group.
 * Each variant has "Just Today" (session-only) and "Make Default" (persistent) pill buttons.
 * [currentSwapName] marks which swap is currently active with a CURRENT badge.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SwapPickerSheet(
    forExercise: ExercisePlan,
    hasPersistentSwap: Boolean,
    currentSwapName: String? = null,
    onPickForSession: (Swap) -> Unit,
    onPickPersistent: (Swap) -> Unit,
    onClearPersistent: () -> Unit,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val swaps = Swaps.forMuscle(forExercise.muscle)

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp)
        ) {
            // ── Editorial header ─────────────────────────────────────────────
            Text(
                "SWAP — ${forExercise.muscle.displayName.uppercase()} · ${swaps.size} VARIANTS",
                style = MaterialTheme.typography.labelSmall,
                color = muted
            )
            Spacer(Modifier.height(6.dp))
            Text(
                "A different way in.",
                style = MaterialTheme.typography.headlineLarge,
                color = onBg,
                fontStyle = FontStyle.Italic
            )
            Spacer(Modifier.height(6.dp))
            Text(
                "\"Just today\" changes this session only. \"Make default\" replaces the exercise in every future workout.",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )

            Spacer(Modifier.height(24.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(20.dp))

            // ── Variants ─────────────────────────────────────────────────────
            swaps.forEachIndexed { index, swap ->
                if (index > 0) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Spacer(Modifier.height(20.dp))
                }

                val isCurrent = currentSwapName == swap.name
                val letter = ('a' + index).toString()

                // Letter + name block
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Text(
                        "$letter.",
                        style = MaterialTheme.typography.headlineLarge,
                        color = muted,
                        fontStyle = FontStyle.Italic
                    )
                    Column(modifier = Modifier.weight(1f)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Text(
                                swap.name,
                                style = MaterialTheme.typography.bodyLarge,
                                color = onBg,
                                fontWeight = FontWeight.SemiBold
                            )
                            if (isCurrent) {
                                Text(
                                    "· CURRENT",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = accent,
                                    fontWeight = FontWeight.SemiBold
                                )
                            }
                        }
                        Text(
                            swap.muscleTarget,
                            style = MaterialTheme.typography.bodySmall,
                            color = muted,
                            fontStyle = FontStyle.Italic
                        )
                    }
                }

                Spacer(Modifier.height(10.dp))
                Text(
                    swap.why,
                    style = MaterialTheme.typography.bodyMedium,
                    color = muted
                )

                Spacer(Modifier.height(8.dp))
                Row(
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        "WHEN",
                        style = MaterialTheme.typography.labelSmall,
                        color = accent,
                        fontWeight = FontWeight.SemiBold
                    )
                    Text(
                        swap.whenToUse,
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontStyle = FontStyle.Italic
                    )
                }

                Spacer(Modifier.height(14.dp))

                // Pill buttons
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    OutlinedButton(
                        onClick = { onPickForSession(swap) },
                        shape = RoundedCornerShape(50),
                        modifier = Modifier.weight(1f)
                    ) {
                        Text("JUST TODAY", style = MaterialTheme.typography.labelSmall)
                    }
                    Button(
                        onClick = { onPickPersistent(swap) },
                        shape = RoundedCornerShape(50),
                        modifier = Modifier.weight(1f)
                    ) {
                        Text("MAKE DEFAULT", style = MaterialTheme.typography.labelSmall)
                    }
                }

                Spacer(Modifier.height(20.dp))
            }
        }
    }
}
