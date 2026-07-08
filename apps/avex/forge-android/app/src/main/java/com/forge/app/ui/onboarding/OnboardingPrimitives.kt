package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.ForgeMotion

/**
 * Onboarding building blocks. Settings/form archetype (DESIGN §3): mono eyebrow + serif question
 * title per step, quiet captions, and every selectable rendered as an interactive tile — border
 * `outline`@0.35 unselected, accent border + accent@0.15 wash selected (the SegmentPill formula at
 * card size). Alphas only from the §5 ladder; buttons are the §8 capsule levels.
 */

/** Mono chapter eyebrow above the step title ("ABOUT YOU", "YOUR GYM"). */
@Composable
internal fun StepEyebrow(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.sp
    )
}

/** The step's question — the page title voice (serif, no terminal period). */
@Composable
internal fun StepTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onBackground
    )
}

/** One quiet caption line under the title (~12 words, §4.3). */
@Composable
internal fun StepCaption(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    )
}

/** Selected/unselected tile colors — one formula for every selectable in the flow. */
@Composable
private fun selectableColors(selected: Boolean): Pair<Color, Color> {
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
    return border to fill
}

/**
 * A full-width option card: optional leading glyph, label + one-line description, optional mono
 * right meta, and an optional slot above the text row (the plan-mode vignettes).
 */
@Composable
internal fun OptionCard(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    description: String? = null,
    icon: ImageVector? = null,
    meta: String? = null,
    topContent: (@Composable () -> Unit)? = null
) {
    val (border, fill) = selectableColors(selected)
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, border, RoundedCornerShape(12.dp))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        topContent?.invoke()
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (icon != null) {
                Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(22.dp))
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

/** A capsule choice chip (plates, cadence, problem areas, sex). */
@Composable
internal fun ChoiceChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val (border, fill) = selectableColors(selected)
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .border(1.dp, border, RoundedCornerShape(50))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (selected) MaterialTheme.colorScheme.onBackground
            else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/** A two-option unit selector — equal-width capsule cells (lb | kg, mi | km). */
@Composable
internal fun UnitSegment(
    first: String,
    second: String,
    secondSelected: Boolean,
    onSelect: (Boolean) -> Unit
) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        ChoiceChip(first, !secondSelected, { onSelect(false) }, Modifier.weight(1f))
        ChoiceChip(second, secondSelected, { onSelect(true) }, Modifier.weight(1f))
    }
}

/** A square-ish icon tile for the equipment fine-tune grid. */
@Composable
internal fun EquipmentTile(
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
            .semantics { this.selected = selected }
            .padding(horizontal = 6.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Icon(
            icon, contentDescription = null,
            tint = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(26.dp)
        )
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            minLines = 2,
            maxLines = 2
        )
    }
}

/** A preset tile: glyph, name, and a mono piece-count meta line. */
@Composable
internal fun PresetTile(
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
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Icon(
            icon, contentDescription = null,
            tint = if (selected) MaterialTheme.colorScheme.onBackground else muted,
            modifier = Modifier.size(24.dp)
        )
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                label,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onBackground,
                minLines = 2,
                maxLines = 2
            )
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = if (selected) MaterialTheme.colorScheme.primary else muted
            )
        }
    }
}

/** The 40dp round day-count chip. */
@Composable
internal fun DayChip(n: Int, selected: Boolean, onClick: () -> Unit) {
    val (border, fill) = selectableColors(selected)
    Box(
        modifier = Modifier
            .size(40.dp)
            .clip(RoundedCornerShape(50))
            .border(1.dp, border, RoundedCornerShape(50))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected },
        contentAlignment = Alignment.Center
    ) {
        Text(
            "$n",
            style = MaterialTheme.typography.titleMedium,
            color = if (selected) MaterialTheme.colorScheme.onBackground
            else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/** The slim animated step-progress rail: accent fill on an outline track. */
@Composable
internal fun ProgressRail(fraction: Float, modifier: Modifier = Modifier) {
    val animated by animateFloatAsState(fraction, ForgeMotion.standardTween(), label = "onb_progress")
    Box(
        modifier
            .height(4.dp)
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.25f))
    ) {
        Box(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth(animated.coerceIn(0f, 1f))
                .clip(RoundedCornerShape(50))
                .background(MaterialTheme.colorScheme.primary)
        )
    }
}

/** §8 level ① — the filled light do-it-now capsule (Continue / Let's go). */
@Composable
internal fun PrimaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(onBg.copy(alpha = if (enabled) 1f else 0.35f))
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.background)
    }
}

/** §8 level ② — the outlined sidekick capsule (Re-roll). */
@Composable
internal fun OutlineCapsule(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .bounceClick(onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
    }
}

/** §8 level ③ — the mono accent navigation link (skip →). */
@Composable
internal fun SkipLink(onClick: () -> Unit) {
    Text(
        "skip →",
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier
            .clickableLabeled("Skip setup", onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 12.dp)
    )
}

/** The quiet italic brand aside on the welcome step. */
@Composable
internal fun BrandAside(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        fontStyle = FontStyle.Italic,
        color = MaterialTheme.colorScheme.onSurfaceVariant
    )
}

/** Small mono section anchor inside a step (PRESETS / FINE-TUNE / WEIGHT / DISTANCE). */
@Composable
internal fun StepSectionLabel(text: String, meta: String? = null) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        if (meta != null) {
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
