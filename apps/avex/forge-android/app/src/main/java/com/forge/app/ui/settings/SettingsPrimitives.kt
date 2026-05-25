@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun SectionLabel(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.5.sp,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 8.dp)
    )
}

@Composable
internal fun SectionDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(horizontal = 24.dp),
        color = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)
    )
}

@Composable
internal fun SettingsNavRow(label: String, subtitle: String, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Text("→", style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}

@Composable
internal fun ToggleRow(
    label: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val bg = MaterialTheme.colorScheme.background
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedTrackColor = onBg.copy(alpha = 0.85f),
                checkedThumbColor = bg,
                checkedBorderColor = Color.Transparent,
                uncheckedTrackColor = Color.Transparent,
                uncheckedThumbColor = outline,
                uncheckedBorderColor = outline.copy(alpha = 0.5f)
            )
        )
    }
}

@Composable
internal fun PillChip(label: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val alpha = if (enabled) 1f else 0.3f
    Box(
        modifier = Modifier
            .border(1.dp, (if (selected) onBg else outline.copy(alpha = 0.4f)).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .background((if (selected) onBg else Color.Transparent).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .then(if (enabled) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = (if (selected) bg else muted.copy(alpha = 0.65f)).copy(alpha = alpha),
            letterSpacing = 0.5.sp
        )
    }
}

@Composable
internal fun SubSectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 6.dp)
    )
}

@Composable
internal fun AccentColorRow(currentHex: String, onSelect: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accentColors = listOf(
        "#3D4F73" to "Navy", "#8B3535" to "Red", "#4D6040" to "Olive", "#7A6435" to "Gold"
    )
    Column(
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("Accent color", style = MaterialTheme.typography.bodyMedium, color = onBg)
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            accentColors.forEach { (hex, label) ->
                val isSelected = currentHex == hex || (currentHex.isEmpty() && hex == "#3D4F73")
                val swatchColor = remember(hex) { Color(android.graphics.Color.parseColor(hex)) }
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.clickable { onSelect(hex) }
                ) {
                    Box(
                        modifier = Modifier
                            .size(28.dp)
                            .clip(CircleShape)
                            .background(swatchColor)
                            .border(2.dp, if (isSelected) onBg else Color.Transparent, CircleShape)
                    )
                    Text(
                        label,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = if (isSelected) 0.9f else 0.45f),
                        fontSize = 9.sp
                    )
                }
            }
        }
        Spacer(Modifier.height(4.dp))
    }
}

@Composable
internal fun HourPickerRow(label: String, hour: Int, onHourChange: (Int) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Row(
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "−", style = MaterialTheme.typography.bodyLarge, color = muted,
                modifier = Modifier.clickable { onHourChange((hour - 1 + 24) % 24) }.padding(4.dp)
            )
            Text("${hour.toString().padStart(2, '0')}:00", style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                "+", style = MaterialTheme.typography.bodyLarge, color = muted,
                modifier = Modifier.clickable { onHourChange((hour + 1) % 24) }.padding(4.dp)
            )
        }
    }
}

@Composable
internal fun TileOrderRow(
    label: String,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(
                "↑", style = MaterialTheme.typography.bodyMedium,
                color = if (canMoveUp) muted else muted.copy(alpha = 0.2f),
                modifier = if (canMoveUp) Modifier.clickable(onClick = onMoveUp).padding(4.dp) else Modifier.padding(4.dp)
            )
            Text(
                "↓", style = MaterialTheme.typography.bodyMedium,
                color = if (canMoveDown) muted else muted.copy(alpha = 0.2f),
                modifier = if (canMoveDown) Modifier.clickable(onClick = onMoveDown).padding(4.dp) else Modifier.padding(4.dp)
            )
        }
    }
}

@Composable
internal fun DestructiveRow(label: String, isFactory: Boolean = false, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val error = MaterialTheme.colorScheme.error
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = if (isFactory) error else onBg)
        Text(
            "→", style = MaterialTheme.typography.bodyMedium,
            color = if (isFactory) error.copy(alpha = 0.5f) else muted.copy(alpha = 0.4f)
        )
    }
}

// ─── Chip section helper (label + FlowRow of chips) ──────────────────────────

@Composable
internal fun ChipSection(
    label: String,
    options: List<Pair<String, String>>,
    selected: String,
    onSelect: (String) -> Unit
) {
    SubSectionLabel(label)
    FlowRow(
        modifier = Modifier.padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        options.forEach { (value, display) ->
            PillChip(display.uppercase(), selected == value) { onSelect(value) }
        }
    }
    Spacer(Modifier.height(8.dp))
}
