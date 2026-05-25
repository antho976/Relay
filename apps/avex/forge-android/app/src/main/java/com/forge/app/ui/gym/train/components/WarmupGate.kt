package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Warmup checklist shown at the top of [com.forge.app.ui.gym.train.DayScreen] until
 * every box is ticked or the user explicitly skips.
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
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 8.dp)
    ) {
        Text(
            "WARMUP",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            letterSpacing = 1.5.sp
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Tick everything off before you start.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(12.dp))

        warmupItems.forEachIndexed { index, label ->
            val checked = checks.getOrElse(index) { false }
            WarmupRow(
                label = label,
                checked = checked,
                onToggle = { onToggle(index) },
                reaction = reactions[index],
                onReaction = if (checked) { thumbsUp -> onReaction(index, thumbsUp) } else null,
                onBg = onBg,
                muted = muted,
                outline = outline,
                bg = bg
            )
            HorizontalDivider(color = outline.copy(alpha = 0.15f))
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 12.dp),
            horizontalArrangement = Arrangement.End
        ) {
            Text(
                "skip warmup →",
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.5f),
                modifier = Modifier
                    .clickable(onClick = onSkip)
                    .padding(vertical = 4.dp, horizontal = 2.dp)
            )
        }
    }
}

@Composable
private fun WarmupRow(
    label: String,
    checked: Boolean,
    onToggle: () -> Unit,
    reaction: Boolean?,
    onReaction: ((Boolean) -> Unit)?,
    onBg: Color,
    muted: Color,
    outline: Color,
    bg: Color
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onToggle)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Box(
            modifier = Modifier
                .size(20.dp)
                .clip(CircleShape)
                .background(if (checked) onBg else Color.Transparent)
                .border(1.dp, if (checked) onBg else outline.copy(alpha = 0.45f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            if (checked) {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = null,
                    tint = bg,
                    modifier = Modifier.size(12.dp)
                )
            }
        }
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (checked) muted.copy(alpha = 0.5f) else onBg,
            modifier = Modifier.weight(1f),
            textDecoration = if (checked) TextDecoration.LineThrough else TextDecoration.None
        )
        if (onReaction != null) {
            Text(
                "↑",
                style = MaterialTheme.typography.labelSmall,
                color = if (reaction == true) onBg else muted.copy(alpha = 0.3f),
                fontSize = 11.sp,
                modifier = Modifier.clickable { onReaction(true) }.padding(4.dp)
            )
            Text(
                "↓",
                style = MaterialTheme.typography.labelSmall,
                color = if (reaction == false) MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
                        else muted.copy(alpha = 0.3f),
                fontSize = 11.sp,
                modifier = Modifier.clickable { onReaction(false) }.padding(4.dp)
            )
        }
    }
}
