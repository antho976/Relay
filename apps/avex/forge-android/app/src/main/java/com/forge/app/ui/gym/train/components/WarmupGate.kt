package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.remember
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * The warmup checklist. Shown at the top of [com.forge.app.ui.gym.train.DayScreen] until
 * either every box is ticked or the user explicitly skips. Doesn't block scrolling — the
 * exercise list is hidden behind it, but the screen still accepts back-navigation.
 */
@Composable
fun WarmupGate(
    warmupItems: List<String>,
    checks: List<Boolean>,
    onToggle: (Int) -> Unit,
    onSkip: () -> Unit,
    reactions: Map<Int, Boolean> = emptyMap(),
    onReaction: (Int, Boolean) -> Unit = { _, _ -> },
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp)),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 2.dp
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "WARMUP",
                color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.Black
            )
            Text(
                text = "Tick everything off before you start. Skip if you've already warmed up.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium
            )

            warmupItems.forEachIndexed { index, label ->
                WarmupRow(
                    label = label,
                    checked = checks.getOrElse(index) { false },
                    onToggle = { onToggle(index) },
                    reaction = reactions[index],
                    onReaction = if (checks.getOrElse(index) { false }) { thumbsUp -> onReaction(index, thumbsUp) } else null
                )
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                horizontalArrangement = Arrangement.End
            ) {
                TextButton(onClick = onSkip) {
                    Text("Skip warmup")
                }
            }
        }
    }
}

@Composable
private fun WarmupRow(
    label: String,
    checked: Boolean,
    onToggle: () -> Unit,
    reaction: Boolean? = null,
    onReaction: ((Boolean) -> Unit)? = null
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .clickable { onToggle() }
            .padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Checkbox(checked = checked, onCheckedChange = { onToggle() })
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (checked) MaterialTheme.colorScheme.onSurfaceVariant
                    else MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f)
        )
        if (onReaction != null) {
            Text(
                text = "👍",
                style = MaterialTheme.typography.bodyMedium,
                color = if (reaction == true) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
                modifier = Modifier.clickable { onReaction(true) }
            )
            Text(
                text = "👎",
                style = MaterialTheme.typography.bodyMedium,
                color = if (reaction == false) MaterialTheme.colorScheme.error
                        else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
                modifier = Modifier.clickable { onReaction(false) }
            )
        }
    }
}
