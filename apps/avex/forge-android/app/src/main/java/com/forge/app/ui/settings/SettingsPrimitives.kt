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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.AccentEmphasis
import com.forge.app.ui.theme.emphasized

@Composable
internal fun SectionLabel(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.labelSmall,
        color = emphasized(MaterialTheme.colorScheme.onSurfaceVariant),
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
internal fun AccentEmphasisRow(current: String, onSelect: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    Column(
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("Accent emphasis", style = MaterialTheme.typography.bodyMedium, color = onBg)
        Text(
            "Color important text — big numbers, titles, names — with your accent.",
            style = MaterialTheme.typography.bodySmall, color = muted
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            AccentEmphasis.options.forEach { (key, label) ->
                val selected = current.equals(key, ignoreCase = true) || (current.isBlank() && key == "off")
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(8.dp))
                        .border(1.dp, if (selected) accent else muted.copy(alpha = 0.4f), RoundedCornerShape(8.dp))
                        .background(if (selected) accent.copy(alpha = 0.15f) else Color.Transparent)
                        .clickable { onSelect(key) }
                        .padding(horizontal = 16.dp, vertical = 8.dp)
                ) {
                    Text(label, style = MaterialTheme.typography.bodySmall, color = if (selected) accent else muted)
                }
            }
        }
    }
}

@Composable
internal fun AccentColorRow(currentHex: String, onSelect: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accentColors = listOf(
        "#3D4F73" to "Navy", "#8B3535" to "Red", "#4D6040" to "Olive", "#7A6435" to "Gold",
        "#356B6B" to "Teal", "#5B4570" to "Purple", "#8B3556" to "Rose", "#3E5E3E" to "Forest",
        "#8B5A35" to "Copper", "#445A6B" to "Steel", "#6B4535" to "Rust", "#556B35" to "Moss"
    )
    Column(
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("Accent color", style = MaterialTheme.typography.bodyMedium, color = onBg)
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
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
        CustomHexInput(currentHex = currentHex, onSelect = onSelect, onBg = onBg, muted = muted)
        Spacer(Modifier.height(4.dp))
    }
}

/** Type any `#RRGGBB` hex for an accent colour outside the preset palette; applies live once valid. */
@Composable
private fun CustomHexInput(currentHex: String, onSelect: (String) -> Unit, onBg: Color, muted: Color) {
    // Seed from the current pref only when it's a hex that ISN'T one of the presets (a real custom pick).
    val presets = remember { setOf("#3D4F73", "#8B3535", "#4D6040", "#7A6435", "#356B6B", "#5B4570", "#8B3556", "#3E5E3E", "#8B5A35", "#445A6B", "#6B4535", "#556B35") }
    var text by remember(currentHex) {
        mutableStateOf(currentHex.takeIf { it.length == 7 && it !in presets }.orEmpty())
    }
    val valid = text.matches(Regex("#[0-9A-F]{6}"))
    val preview = if (valid) remember(text) { Color(android.graphics.Color.parseColor(text)) } else null
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text("Custom", style = MaterialTheme.typography.bodySmall, color = muted)
        BasicTextField(
            value = text,
            onValueChange = { raw ->
                // Normalize: a single leading '#', uppercase hex only, capped at #RRGGBB.
                val hex = raw.uppercase().filter { it in "0123456789ABCDEF" }.take(6)
                text = "#$hex"
                if (text.matches(Regex("#[0-9A-F]{6}"))) onSelect(text)
            },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            modifier = Modifier
                .clip(RoundedCornerShape(6.dp))
                .border(1.dp, muted.copy(alpha = 0.4f), RoundedCornerShape(6.dp))
                .padding(horizontal = 10.dp, vertical = 6.dp)
                .size(width = 96.dp, height = 20.dp)
        )
        Box(
            modifier = Modifier
                .size(28.dp)
                .clip(CircleShape)
                .background(preview ?: Color.Transparent)
                .border(1.dp, muted.copy(alpha = 0.4f), CircleShape)
        )
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
                modifier = Modifier.clickableLabeled("Earlier hour") { onHourChange((hour - 1 + 24) % 24) }.padding(4.dp)
            )
            Text("${hour.toString().padStart(2, '0')}:00", style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                "+", style = MaterialTheme.typography.bodyLarge, color = muted,
                modifier = Modifier.clickableLabeled("Later hour") { onHourChange((hour + 1) % 24) }.padding(4.dp)
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
                modifier = if (canMoveUp) Modifier.clickableLabeled("Move up", onClick = onMoveUp).padding(4.dp) else Modifier.padding(4.dp)
            )
            Text(
                "↓", style = MaterialTheme.typography.bodyMedium,
                color = if (canMoveDown) muted else muted.copy(alpha = 0.2f),
                modifier = if (canMoveDown) Modifier.clickableLabeled("Move down", onClick = onMoveDown).padding(4.dp) else Modifier.padding(4.dp)
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
