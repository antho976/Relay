package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.ForgeMotion

/**
 * Onboarding building blocks. Settings/form archetype (DESIGN §3): a serif question carries each
 * step on its own — the mono chapter eyebrow that used to sit above it is gone (2026-08-22), since
 * the segmented [StepRail] already says where you are and the question already says what it wants.
 * Every selectable shares ONE tile formula — border `outline`@0.35 unselected, accent border +
 * accent@0.15 wash selected (the SegmentPill formula at card size). Alphas only from the §5 ladder;
 * buttons are the §8 capsule levels.
 */

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

/** Small mono section anchor inside a step (UNITS / PLATE WEIGHT / RACKS & BENCHES). */
@Composable
internal fun StepSectionLabel(text: String, meta: String? = null) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
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

/** A capsule choice chip (plates, refresh cadence, sore spots, sex, watch). */
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
            // §14 — the target is 48dp even though the capsule is trimmer than that; the extra comes
            // from the interaction box, not from padding the visual out of proportion.
            .minimumInteractiveComponentSize()
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
            .minimumInteractiveComponentSize()
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

/**
 * The step rail: one cell per step of the path you are actually on, accent behind you, hollow ahead
 * (§2② — a filled / hollow rail for a set of items, some present). It replaced a single continuous
 * bar (2026-08-22) whose denominator had to *guess* the path length before the plan-mode fork; cells
 * say how many steps are left instead of implying a fraction, and committing to the short custom /
 * freestyle path visibly drops the cells that will never run.
 */
@Composable
internal fun StepRail(step: Int, total: Int, modifier: Modifier = Modifier) {
    Row(
        modifier.semantics { contentDescription = "Step ${step + 1} of $total" },
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        repeat(total) { i ->
            val color by animateColorAsState(
                if (i <= step) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.outline.copy(alpha = 0.25f),
                ForgeMotion.standardTween(),
                label = "rail_cell"
            )
            Box(
                Modifier
                    .weight(1f)
                    .height(3.dp)
                    .clip(RoundedCornerShape(50))
                    .background(color)
            )
        }
    }
}

/** §8 level ① — the filled light do-it-now capsule (Continue / Start training). Onboarding's
 *  full-width CTA is the shared [ForgePrimaryCapsule] under an onboarding-local name, so the two
 *  can't drift. */
@Composable
internal fun PrimaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) = ForgePrimaryCapsule(label = label, onClick = onClick, modifier = modifier, enabled = enabled)

/** §8 level ② — the outlined sidekick capsule (Re-roll). Delegates to the shared [ForgeOutlineCapsule]. */
@Composable
internal fun OutlineCapsule(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) =
    ForgeOutlineCapsule(label = label, onClick = onClick, modifier = modifier)

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

/** The quiet italic brand aside — the offline promise, said once, on the last page. */
@Composable
internal fun BrandAside(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        fontStyle = FontStyle.Italic,
        color = MaterialTheme.colorScheme.onSurfaceVariant
    )
}

/**
 * A boolean with its one-line explainer (§4.3) and a drawn [ForgeSwitch]. The WHOLE row is the tap
 * target and the switch is passive — never a nested tap (§14). Disabled rows render inert rather
 * than tappable-but-dead (§2③).
 */
@Composable
internal fun ToggleRow(
    label: String,
    description: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    enabled: Boolean = true
) {
    val alpha = if (enabled) 1f else 0.35f
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (enabled) Modifier.clickableLabeled(label) { onToggle(!checked) }
                else Modifier
            )
            // The switch itself is drawn, not focusable, so the row has to announce the state or
            // TalkBack reads a name with no value (§14).
            .semantics { stateDescription = if (checked && enabled) "On" else "Off" }
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                label,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha)
            )
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
            )
        }
        Box(Modifier.clearAndSetSemantics { }) {
            ForgeSwitch(checked = checked && enabled, onCheckedChange = null, enabled = enabled)
        }
    }
}
