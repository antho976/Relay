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
import androidx.compose.foundation.shape.RoundedCornerShape
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

@Composable
fun WarmupGate(
    warmupItems: List<String>,
    checks: List<Boolean>,
    onToggle: (Int) -> Unit,
    onSkip: () -> Unit,
    onDisableToday: () -> Unit,
    onDisableWeek: () -> Unit,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 12.dp)
    ) {
        Text(
            "WARMUP",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            letterSpacing = 1.5.sp
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Complete everything before you start.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(16.dp))

        warmupItems.forEachIndexed { index, label ->
            val checked = checks.getOrElse(index) { false }
            WarmupRow(
                label = label,
                checked = checked,
                onToggle = { onToggle(index) },
                onBg = onBg,
                muted = muted,
                outline = outline,
                bg = bg
            )
            HorizontalDivider(color = outline.copy(alpha = 0.15f))
        }

        Spacer(Modifier.height(16.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(
                    "No warmup today",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 11.sp,
                    modifier = Modifier.clickable(onClick = onDisableToday)
                )
                Text(
                    "No warmup this week",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 11.sp,
                    modifier = Modifier.clickable(onClick = onDisableWeek)
                )
            }
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .border(1.dp, muted, RoundedCornerShape(50))
                    .clickable(onClick = onSkip)
                    .padding(horizontal = 14.dp, vertical = 6.dp)
            ) {
                Text(
                    "Skip warmup",
                    style = MaterialTheme.typography.labelSmall,
                    color = onBg,
                    fontSize = 11.sp
                )
            }
        }
    }
}

@Composable
private fun WarmupRow(
    label: String,
    checked: Boolean,
    onToggle: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    bg: Color
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onToggle)
            .padding(vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(
            modifier = Modifier
                .size(24.dp)
                .clip(CircleShape)
                .background(if (checked) onBg else Color.Transparent)
                .border(1.5.dp, if (checked) onBg else muted, CircleShape),
            contentAlignment = Alignment.Center
        ) {
            if (checked) {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = null,
                    tint = bg,
                    modifier = Modifier.size(14.dp)
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
    }
}
