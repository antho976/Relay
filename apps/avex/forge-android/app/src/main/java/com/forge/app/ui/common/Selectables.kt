@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.common

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeMotion

/**
 * The app's selectable family — every "pick one / pick some" control outside a lens toggle.
 *
 * Promoted out of `onboarding/OnboardingPrimitives.kt` on its third screen (onboarding · Settings →
 * Program · Settings → Units, Session, Coach and Wearable, 2026-09-25). Settings had its own
 * `PillChip` — 4dp corners, 10sp UPPERCASE mono, no 48dp target, no motion — so the same question
 * ("how many days a week?") looked finished in onboarding and like a form in Settings.
 *
 * ONE tile formula for all of them (§3 onboarding, §5 ladder): border `outline`@0.35 unselected →
 * accent border + accent@0.15 wash selected, cross-fading over [ForgeMotion.DurationFast]. Labels
 * are sentence case in the sans voice; a mono label is for a caption, not for a choice.
 */
@Composable
internal fun selectableColors(selected: Boolean, enabled: Boolean = true): Pair<Color, Color> {
    val dim = if (enabled) 1f else 0.35f
    val border by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary
        else MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "sel_border"
    )
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f) else Color.Transparent,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "sel_fill"
    )
    return border.copy(alpha = border.alpha * dim) to fill.copy(alpha = fill.alpha * dim)
}

/** The label tone that pairs with [selectableColors]: onBg when picked, muted when not. */
@Composable
private fun selectableText(selected: Boolean, enabled: Boolean = true): Color {
    val base = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
    return if (enabled) base else base.copy(alpha = 0.35f)
}

/**
 * A capsule choice chip. The target is ≥48dp from [minimumInteractiveComponentSize] even though the
 * capsule draws trimmer (§14 — touch comes from the interaction box, not from a chunky visual).
 */
@Composable
fun ForgeChoiceChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    val (border, fill) = selectableColors(selected, enabled)
    Box(
        modifier = modifier
            .minimumInteractiveComponentSize()
            .clip(RoundedCornerShape(50))
            .border(1.dp, border, RoundedCornerShape(50))
            .background(fill)
            .bounceClick(enabled = enabled, onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = selectableText(selected, enabled),
            textAlign = TextAlign.Center
        )
    }
}

/**
 * A full-width option card: optional leading glyph, label + one-line description, optional mono
 * right meta (a reading, never a state word), and an optional slot above the text row.
 */
@Composable
fun ForgeOptionCard(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    description: String? = null,
    icon: ImageVector? = null,
    meta: String? = null,
    topContent: (@Composable () -> Unit)? = null
) {
    val (border, fill) = selectableColors(selected)
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, border, RoundedCornerShape(12.dp))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        topContent?.invoke()
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (icon != null) {
                Icon(
                    icon, contentDescription = null,
                    tint = if (selected) MaterialTheme.colorScheme.onBackground else muted,
                    modifier = Modifier.size(22.dp)
                )
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                if (description != null) {
                    Text(description, style = MaterialTheme.typography.bodySmall, color = muted)
                }
            }
            if (meta != null) {
                Text(
                    meta.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (selected) MaterialTheme.colorScheme.primary else muted
                )
            }
        }
    }
}

/** A square-ish glyph tile for a gear grid: icon over a two-line label. */
@Composable
fun ForgeIconTile(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val (border, fill) = selectableColors(selected)
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, border, RoundedCornerShape(12.dp))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.Checkbox }
            .padding(horizontal = 6.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Icon(icon, contentDescription = null, tint = selectableText(selected), modifier = Modifier.size(26.dp))
        // minLines keeps a row of tiles one height; no maxLines, so a long name wraps at 200% (§14).
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = selectableText(selected),
            textAlign = TextAlign.Center,
            minLines = 2
        )
    }
}

/** A preset tile: glyph, name, and a mono meta line (a count, never a state word). */
@Composable
fun ForgePresetTile(
    icon: ImageVector,
    label: String,
    meta: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val (border, fill) = selectableColors(selected)
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, border, RoundedCornerShape(12.dp))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Icon(icon, contentDescription = null, tint = selectableText(selected), modifier = Modifier.size(24.dp))
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, minLines = 2)
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = if (selected) MaterialTheme.colorScheme.primary else muted
            )
        }
    }
}

/** The 40dp round count chip (days per week). Grows with font scale; the target stays ≥48dp. */
@Composable
fun ForgeDayChip(n: Int, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val (border, fill) = selectableColors(selected)
    Box(
        modifier = modifier
            .minimumInteractiveComponentSize()
            .sizeIn(minWidth = 40.dp, minHeight = 40.dp)
            .clip(RoundedCornerShape(50))
            .border(1.dp, border, RoundedCornerShape(50))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            "$n",
            style = MaterialTheme.typography.titleMedium,
            color = selectableText(selected),
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 6.dp)
        )
    }
}

/**
 * An equal-width segmented choice — 2 to 4 short, mutually exclusive values of ONE setting (lb · kg
 * · st, 12h · 24h, Galaxy · Pixel · Other). Each cell is a [ForgeChoiceChip], so a segment and a
 * chip can never disagree about what "selected" looks like. Not for switching views: that is
 * [SegmentPill]'s job (§2③).
 */
@Composable
fun ForgeSegmentedChoice(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    // Equal cells split the width by the option count, so at a large font scale a 3–4-way choice
    // broke its words mid-label ("Ligh / t"). Past 1.3× the cells size to their labels and wrap
    // instead (§14: wrap rather than clip); the selectable drawing is the same either way.
    if (LocalDensity.current.fontScale > 1.3f && options.size > 2) {
        FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEachIndexed { i, label -> ForgeChoiceChip(label, i == selectedIndex, { onSelect(i) }) }
        }
    } else {
        Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEachIndexed { i, label ->
                ForgeChoiceChip(label, i == selectedIndex, { onSelect(i) }, Modifier.weight(1f))
            }
        }
    }
}
