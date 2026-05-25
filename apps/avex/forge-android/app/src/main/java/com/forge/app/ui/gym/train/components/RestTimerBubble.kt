package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.domain.timer.RestTimerState

/**
 * Floating timer circle. Tap = open controls, long-press = +30s quick-extend (#96).
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun RestTimerBubble(
    state: RestTimerState,
    onOpenControls: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    val isPaused = state.isPaused && !state.isFinished

    Box(
        modifier = modifier
            .size(60.dp)
            .clip(CircleShape)
            .then(
                if (isPaused) {
                    Modifier.drawBehind {
                        val effect = PathEffect.dashPathEffect(floatArrayOf(6f, 5f), 0f)
                        drawCircle(
                            color = onBg.copy(alpha = 0.5f),
                            style = Stroke(
                                width = 1.dp.toPx(),
                                pathEffect = effect
                            ),
                            radius = size.minDimension / 2f - 0.5.dp.toPx()
                        )
                    }
                } else {
                    Modifier.background(onBg)
                }
            )
            .combinedClickable(
                onClick = onOpenControls,
                onLongClick = onLongClick
            ),
        contentAlignment = Alignment.Center
    ) {
        if (state.isFinished) {
            Text("✓", style = MaterialTheme.typography.titleLarge, color = bg)
        } else {
            Text(
                formatTime(state.secondsRemaining),
                style = MaterialTheme.typography.titleMedium,
                color = if (!isPaused) bg else onBg.copy(alpha = 0.7f),
                fontWeight = FontWeight.SemiBold
            )
        }
    }
}

@Composable
fun RestTimerControlsDialog(
    state: RestTimerState,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onReset: () -> Unit,
    onSkip: () -> Unit,
    onAddSeconds: (Int) -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(bg, RoundedCornerShape(8.dp))
                .padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                "REST TIMER",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                letterSpacing = 1.5.sp
            )

            Row(verticalAlignment = Alignment.Bottom) {
                Text(
                    formatTime(state.secondsRemaining),
                    style = MaterialTheme.typography.displayLarge,
                    color = onBg
                )
                val suffix = when {
                    state.isFinished -> " done."
                    state.isPaused  -> " paused."
                    else            -> " of ${formatTime(state.totalSeconds)}"
                }
                Text(
                    suffix,
                    style = MaterialTheme.typography.headlineSmall,
                    color = muted,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(start = 4.dp, bottom = 6.dp)
                )
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(60 to "+1 min", 120 to "+2 min", 300 to "+5 min").forEach { (s, label) ->
                    Box(
                        modifier = Modifier
                            .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
                            .clickable { onAddSeconds(s) }
                            .padding(horizontal = 10.dp, vertical = 6.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            label,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            letterSpacing = 0.5.sp
                        )
                    }
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "reset",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onReset).padding(4.dp)
                )
                Text(
                    "skip",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onSkip).padding(4.dp)
                )
                Text(
                    if (state.isPaused) "resume →" else "pause →",
                    style = MaterialTheme.typography.bodyMedium,
                    color = onBg,
                    modifier = Modifier
                        .clickable { if (state.isPaused) onResume() else onPause() }
                        .padding(4.dp)
                )
            }
        }
    }
}

private fun formatTime(totalSeconds: Int): String {
    val s = totalSeconds.coerceAtLeast(0)
    return "%d:%02d".format(s / 60, s % 60)
}
