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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.clickableLabeled

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

/**
 * The shared Settings section anchor — mono 13sp ([EditorialHeader]) carrying the app's air rhythm
 * (DESIGN §7/§8), NO hairline beneath it. Used by the main list and the sub-pages so all of Settings
 * speaks one section-header voice (replaces the older 10sp [SectionLabel]/GroupHeader as pages migrate).
 */
@Composable
internal fun SettingsSectionHeader(label: String, top: Dp = 26.dp) {
    EditorialHeader(
        label = label,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = top, bottom = 4.dp)
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
internal fun SettingsNavRow(
    label: String,
    subtitle: String,
    icon: ImageVector? = null,
    onClick: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Quiet leading glyph (nav-bar family) — wayfinding, muted so it never competes with the accent.
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(22.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Text("→", style = MaterialTheme.typography.bodyMedium, color = muted)
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
        ForgeSwitch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            checkedTrackColor = onBg.copy(alpha = 0.85f),
            checkedThumbColor = bg,
            checkedBorderColor = Color.Transparent,
            uncheckedTrackColor = Color.Transparent,
            uncheckedThumbColor = outline,
            uncheckedBorderColor = outline.copy(alpha = 0.5f)
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
            .padding(horizontal = 10.dp, vertical = 5.dp),
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

/**
 * Settings "do-it-now" action button (DESIGN §8 ①) — a filled light capsule, theme-aware (onBackground
 * fill / background text) so it survives a monochrome accent. Gutter-less: place inside a padded
 * row/[ChipFlow], and group the action buttons at the END of the page — never mid-scroll.
 */
@Composable
internal fun SettingsPrimaryAction(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    Box(
        modifier = Modifier
            .background(onBg.copy(alpha = if (enabled) 1f else 0.35f), RoundedCornerShape(50))
            .then(if (enabled) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = bg, letterSpacing = 0.3.sp)
    }
}

/** The outlined sidekick capsule (DESIGN §8 ②) that sits beside a [SettingsPrimaryAction]. */
@Composable
internal fun SettingsOutlineAction(label: String, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = Modifier
            .border(1.dp, outline.copy(alpha = 0.5f), RoundedCornerShape(50))
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = onBg, letterSpacing = 0.3.sp)
    }
}

/** A quiet mono accent navigation link ("action →") — jumping to another screen (DESIGN §8 ③).
 *  Self-contained: bakes the 24dp gutter + a tappable inset, so call it directly and stack. */
@Composable
internal fun SettingsActionLink(label: String, onClick: () -> Unit) {
    Text(
        label,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        letterSpacing = 0.3.sp,
        modifier = Modifier.clickableLabeled(label, onClick = onClick).padding(horizontal = 24.dp, vertical = 8.dp)
    )
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
