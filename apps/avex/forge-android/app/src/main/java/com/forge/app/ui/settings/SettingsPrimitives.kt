@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled

/**
 * The shared Settings section anchor — mono 13sp ([EditorialHeader]) carrying the app's air rhythm
 * (DESIGN §7/§8), NO hairline beneath it. Used by the main list and the sub-pages so all of Settings
 * speaks one section-header voice.
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
            checkedTrackColor = onBg,
            checkedThumbColor = bg,
            checkedBorderColor = Color.Transparent,
            uncheckedTrackColor = Color.Transparent,
            uncheckedThumbColor = outline,
            uncheckedBorderColor = outline.copy(alpha = 0.35f)
        )
    }
}

@Composable
internal fun PillChip(label: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val alpha = if (enabled) 1f else 0.3f
    // §5 ladder: selected = accent border + accent@0.15 wash (one tile formula with
    // onboarding's selectables) — never a white fill, which read as the do-it-now capsule.
    Box(
        modifier = Modifier
            .border(1.dp, (if (selected) accent else outline.copy(alpha = 0.35f)).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .background((if (selected) accent.copy(alpha = 0.15f) else Color.Transparent).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
            .padding(horizontal = 10.dp, vertical = 5.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = (if (selected) onBg else muted.copy(alpha = 0.65f)).copy(alpha = alpha),
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
    // The gutter Row bounds the capsule to the page width (§7). Without it the Box sized to the
    // label's intrinsic width, which fits at 100% and runs off BOTH edges at 200% font scale —
    // found by RecipeScreenshotTest's 200% golden, invisible to every static check and to the eye
    // at normal scale (§14).
    Row(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Box(
            modifier = Modifier
                .background(onBg.copy(alpha = if (enabled) 1f else 0.35f), RoundedCornerShape(50))
                .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
                .padding(horizontal = 16.dp, vertical = 9.dp),
            contentAlignment = Alignment.Center
        ) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = bg, letterSpacing = 0.3.sp)
        }
    }
}

/** The outlined sidekick capsule (DESIGN §8 ②) that sits beside a [SettingsPrimaryAction].
 *  Disabled = dimmed and inert, so it never looks tappable while doing nothing (§4.5). */
@Composable
internal fun SettingsOutlineAction(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    val alpha = if (enabled) 1f else 0.35f
    // Same gutter fix as SettingsPrimaryAction above (§14, 200% overflow).
    Row(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Box(
            modifier = Modifier
                .border(1.dp, outline.copy(alpha = 0.35f * alpha), RoundedCornerShape(50))
                .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
                .padding(horizontal = 14.dp, vertical = 9.dp),
            contentAlignment = Alignment.Center
        ) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = onBg.copy(alpha = alpha), letterSpacing = 0.3.sp)
        }
    }
}

/**
 * The §12 status dot shared by the settings feed / connection rows — a solid accent disc when the
 * feed is live, a clearly-drawn muted ring (1.5dp @0.55, legible on near-black) when silent. One
 * drawing so Coach and Recovery can't disagree on how connected-vs-silent reads.
 */
@Composable
internal fun StatusDot(active: Boolean, size: Dp = 8.dp) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    if (active) Box(Modifier.size(size).background(accent, CircleShape))
    else Box(Modifier.size(size).border(1.5.dp, muted.copy(alpha = 0.55f), CircleShape))
}

/**
 * The compact OUTLINED "Connect" pill (§8 ② weight — border only, onBg label, sentence case) drawn
 * inside a whole-row tap target. NOT independently clickable — the row owns the tap — so it never
 * nests a second click. Shared by the Coach feed glance and the Recovery integration rows.
 */
@Composable
internal fun ConnectPill(label: String = "Connect") {
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = Modifier
            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .padding(horizontal = 16.dp, vertical = 8.dp),
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
