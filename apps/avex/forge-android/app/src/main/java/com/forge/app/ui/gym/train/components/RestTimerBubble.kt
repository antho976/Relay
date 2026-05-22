package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.timer.RestTimerState

/**
 * Floating timer button. Phase 3c uses a stock Material 3 FloatingActionButton so the
 * Scaffold's FAB slot lays it out reliably — the earlier custom Canvas circle didn't
 * render in that slot for reasons that weren't worth root-causing. The progress ring
 * is a polish task for later.
 */
@Composable
fun RestTimerBubble(
    state: RestTimerState,
    onOpenControls: () -> Unit,
    modifier: Modifier = Modifier
) {
    val finishedColor = Color(0xFF22C55E)
    val containerColor = when {
        state.isFinished -> finishedColor
        state.isPaused -> MaterialTheme.colorScheme.surfaceVariant
        else -> MaterialTheme.colorScheme.primary
    }
    val contentColor = if (state.isPaused && !state.isFinished) {
        MaterialTheme.colorScheme.onSurface
    } else {
        MaterialTheme.colorScheme.onPrimary
    }

    FloatingActionButton(
        onClick = onOpenControls,
        modifier = modifier,
        containerColor = containerColor,
        contentColor = contentColor,
        shape = CircleShape
    ) {
        Text(
            text = formatTime(state.secondsRemaining),
            fontWeight = FontWeight.SemiBold,
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(horizontal = 8.dp)
        )
    }
}

@Composable
fun RestTimerControlsDialog(
    state: RestTimerState,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onReset: () -> Unit,
    onSkip: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Rest timer") },
        text = {
            Text(
                buildString {
                    append(formatTime(state.secondsRemaining))
                    append(" of ")
                    append(formatTime(state.totalSeconds))
                    if (state.isPaused) append(" — paused")
                },
                style = MaterialTheme.typography.bodyLarge
            )
        },
        confirmButton = {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)
            ) {
                TextButton(onClick = onReset) { Text("Reset") }
                TextButton(onClick = onSkip) { Text("Skip") }
                Button(
                    onClick = { if (state.isPaused) onResume() else onPause() }
                ) {
                    Text(if (state.isPaused) "Resume" else "Pause")
                }
            }
        }
    )
}

private fun formatTime(totalSeconds: Int): String {
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    return "%d:%02d".format(minutes, seconds)
}
