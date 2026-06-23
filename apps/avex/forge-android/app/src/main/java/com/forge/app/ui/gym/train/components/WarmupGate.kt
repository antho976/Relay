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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
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
        // Header: section label + live "done / total" progress so the user sees the gate filling up.
        val total = warmupItems.size
        val done = warmupItems.indices.count { checks.getOrElse(it) { false } }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "WARMUP",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                letterSpacing = 1.5.sp
            )
            Text(
                "$done / $total",
                style = MaterialTheme.typography.labelSmall,
                color = if (total > 0 && done == total) onBg else muted
            )
        }
        Spacer(Modifier.height(4.dp))
        Text(
            "Loosen up — check each off before you start.",
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

        Spacer(Modifier.height(20.dp))

        // Skip just this session — a subtle outlined pill on its own centred row. Pulled out of the
        // old single SpaceBetween row, where it collided with the two persistent-skip links on a phone.
        Box(
            modifier = Modifier
                .align(Alignment.CenterHorizontally)
                .clip(RoundedCornerShape(50))
                .border(1.dp, outline, RoundedCornerShape(50))
                .clickable(onClick = onSkip)
                .padding(horizontal = 22.dp, vertical = 9.dp)
        ) {
            Text(
                "Skip warmup",
                style = MaterialTheme.typography.labelMedium,
                color = onBg
            )
        }

        Spacer(Modifier.height(14.dp))

        // Persistent skips — turn warmups off for the rest of today / the rest of this week. Centred
        // with a separator dot so the two labels breathe instead of running into each other.
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "No warmup today",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 11.sp,
                modifier = Modifier
                    .clickable(onClick = onDisableToday)
                    .padding(vertical = 6.dp, horizontal = 6.dp)
            )
            Text(
                "·",
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.5f),
                fontSize = 11.sp
            )
            Text(
                "No warmup this week",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 11.sp,
                modifier = Modifier
                    .clickable(onClick = onDisableWeek)
                    .padding(vertical = 6.dp, horizontal = 6.dp)
            )
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
            .semantics(mergeDescendants = true) {
                role = Role.Checkbox
                stateDescription = if (checked) "Checked" else "Unchecked"
                contentDescription = "$label, ${if (checked) "checked" else "unchecked"}"
            }
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
