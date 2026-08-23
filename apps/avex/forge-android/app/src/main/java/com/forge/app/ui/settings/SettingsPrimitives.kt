@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.toggleableLabeled

/**
 * The page gutter every settings row owns itself (DESIGN §7). A page Column must NOT add it again —
 * the primitives here are the only place it appears.
 */
internal val SETTINGS_GUTTER = 24.dp

/**
 * ONE vertical padding for every settings row (DESIGN §7: "ONE vertical padding for ALL of a lens's
 * rows — sibling sections never mix 4/5/6, the page reads as one rhythm"). Settings had drifted to
 * ten different values (2/4/5/6/8/9/10/11/12/14), so Recovery's signal rows sat 6dp taller than the
 * write-back toggles directly beneath them. Every row primitive below takes this; a page that wants
 * air adds a Spacer, never a fatter row.
 */
internal val SETTINGS_ROW_PAD = 12.dp

/**
 * The shared Settings section anchor — mono 15sp ([EditorialHeader]) carrying the app's air rhythm
 * (DESIGN §7/§8), NO hairline beneath it. Used by the main list and the sub-pages so all of Settings
 * speaks one section-header voice.
 */
@Composable
internal fun SettingsSectionHeader(label: String, top: Dp = 26.dp) {
    EditorialHeader(
        label = label,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, top = top, bottom = 8.dp)
    )
}

/**
 * The one-line explainer that sits under a control's label (DESIGN §3: "each control gets a ≤1-line
 * explainer"). Sans, not mono: §6 gives mono to UPPERCASE micro-labels and says never sentences, and
 * these are sentences. They were `labelSmall` — 10sp MONO — which set every explanation in Settings
 * in the label voice at the smallest size in the app. `bodySmall` is the prose rung and reads.
 */
@Composable
internal fun SettingsExplainer(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier
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
            .clickableLabeled(label, onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Quiet leading glyph (nav-bar family) — wayfinding, muted so it never competes with the accent.
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(22.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            SettingsExplainer(subtitle)
        }
        Text("→", style = MaterialTheme.typography.bodyMedium, color = muted)
    }
}

/**
 * Label + explainer + switch, with the WHOLE ROW as the tap target.
 *
 * The switch used to be the only thing that responded, and `ForgeSwitch` draws a 40×24dp track — so
 * every toggle in Settings (~20 of them) was a 24dp-tall target against §14's ≥48dp minimum. The row
 * now owns the tap and the switch is DRAWN (`onCheckedChange = null`), which also keeps §2③'s
 * one-target-per-row rule: no nested click inside a clickable row.
 */
@Composable
internal fun ToggleRow(
    label: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleableLabeled(label, checked) { onCheckedChange(!checked) }
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            SettingsExplainer(subtitle)
        }
        ForgeSwitch(
            checked = checked,
            onCheckedChange = null,     // drawn — the row is the target (§2③, no nested taps)
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
internal fun PillChip(
    label: String,
    selected: Boolean,
    enabled: Boolean = true,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val alpha = if (enabled) 1f else 0.35f
    // §5 ladder: selected = accent border + accent@0.15 wash (one tile formula with
    // onboarding's selectables) — never a white fill, which read as the do-it-now capsule.
    Box(
        modifier = modifier
            .border(1.dp, (if (selected) accent else outline.copy(alpha = 0.35f)).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .background((if (selected) accent.copy(alpha = 0.15f) else Color.Transparent).copy(alpha = alpha), RoundedCornerShape(4.dp))
            .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
            .padding(horizontal = 12.dp, vertical = 9.dp),
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
 * The ONE place a settings page puts its one-shot actions (DESIGN §8: "group page-level action
 * buttons at the END of the page, never mid-scroll"). Owns the 24dp gutter, sits the filled ① and
 * its outlined ② sidekicks side by side, and — being a FlowRow — WRAPS them to the next line at
 * large font scales instead of running off the edge.
 *
 * This exists because the two capsules below used to bake `fillMaxWidth().padding(horizontal = 24)`
 * into themselves. That made them unusable anywhere except bare at page level: eight of nine call
 * sites wrapped them in another padded Row or a ChipFlow, which double-guttered them to 48dp, and
 * inside a Row two `fillMaxWidth` children meant the second one got whatever the first left. The
 * capsules are now gutterless and size to their label; the gutter and the arrangement live here.
 */
@Composable
internal fun SettingsActionRow(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier = Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content
    )
}

/**
 * Settings "do-it-now" action button (DESIGN §8 ①) — a filled light capsule, theme-aware (onBackground
 * fill / background text) so it survives a monochrome accent. Gutterless: place it inside a
 * [SettingsActionRow], which owns the gutter and the wrapping.
 */
@Composable
internal fun SettingsPrimaryAction(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    Box(
        modifier = Modifier
            .background(onBg.copy(alpha = if (enabled) 1f else 0.35f), RoundedCornerShape(50))
            .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
            .padding(horizontal = 20.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = bg, letterSpacing = 0.3.sp)
    }
}

/** The outlined sidekick capsule (DESIGN §8 ②) that sits beside a [SettingsPrimaryAction] inside a
 *  [SettingsActionRow]. Disabled = dimmed and inert, so it never looks tappable while doing
 *  nothing (§4.5). Pass [contentColor] for the destructive variant (§8: tinted `error`, never a
 *  filled red button). */
@Composable
internal fun SettingsOutlineAction(
    label: String,
    enabled: Boolean = true,
    contentColor: Color = MaterialTheme.colorScheme.onBackground,
    onClick: () -> Unit
) {
    val outline = MaterialTheme.colorScheme.outline
    val alpha = if (enabled) 1f else 0.35f
    Box(
        modifier = Modifier
            .border(1.dp, outline.copy(alpha = 0.35f * alpha), RoundedCornerShape(50))
            .then(if (enabled) Modifier.bounceClick(onClick = onClick) else Modifier)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = contentColor.copy(alpha = alpha), letterSpacing = 0.3.sp)
    }
}

/**
 * The §12 status dot shared by the settings feed / connection rows — a solid accent disc when the
 * feed is live, a clearly-drawn muted ring (1.5dp @0.55, legible on near-black) when silent. One
 * drawing so Coach and Recovery can't disagree on how connected-vs-silent reads.
 *
 * The 0.55 ring is off the §5 ladder deliberately: §8 specifies that exact weight so the empty state
 * reads on near-black, and §14 exempts structural marks from the text floor. It stays allowlisted.
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
internal fun ConnectPill(label: String = "Connect") = ForgeRowPill(label)

/** A quiet mono accent navigation link ("action →") — jumping to another screen (DESIGN §8 ③).
 *  Self-contained: bakes the 24dp gutter + a tappable inset, so call it directly and stack. */
@Composable
internal fun SettingsActionLink(label: String, onClick: () -> Unit) {
    Text(
        label,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        letterSpacing = 0.3.sp,
        modifier = Modifier
            .clickableLabeled(label, onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
    )
}

/**
 * Label on the left, a value with −/+ steppers on the right. The steppers are [GlyphButton]s, which
 * guarantee the ≥48dp target (§14) a padded `Text("−")` never had — they were 26dp.
 */
@Composable
internal fun HourPickerRow(label: String, hour: Int, onHourChange: (Int) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            GlyphButton("−", "Earlier hour", muted, { onHourChange((hour - 1 + 24) % 24) })
            Text("${hour.toString().padStart(2, '0')}:00", style = MaterialTheme.typography.bodyMedium, color = onBg)
            GlyphButton("+", "Later hour", muted, { onHourChange((hour + 1) % 24) })
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
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // GlyphButton dims a disabled glyph to the shared §4.5 inert level and makes it inert,
            // so an un-movable arrow never looks tappable (and never sits at an off-ladder 0.2).
            GlyphButton("↑", "Move up", muted, onMoveUp, enabled = canMoveUp)
            GlyphButton("↓", "Move down", muted, onMoveDown, enabled = canMoveDown)
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
            .clickableLabeled(label, onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = if (isFactory) error else onBg)
        Text("→", style = MaterialTheme.typography.bodyMedium, color = if (isFactory) error else muted)
    }
}

/**
 * The Settings search field (DESIGN §13: "a filled rounded field (surfaceVariant — the standard
 * phone-search look, Settings + timezone picker)"). Interactive, so the fill and the rounded corners
 * are earned (§1).
 *
 * There were three of these — the main list's (filled, rounded-12, Material magnifier), the timezone
 * picker's (filled, rounded-10, no magnifier at all) and Exercise-likes' (BORDERED, rounded-8, with
 * a `Text("⌕")` standing in for the icon). Same control, three drawings, and the odd one out made
 * Exercise-likes read as a different app. One drawing now, per §2⑥.
 */
@Composable
internal fun SettingsSearchField(
    query: String,
    placeholder: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(start = 14.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(Icons.Filled.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            singleLine = true,
            modifier = Modifier.weight(1f).padding(vertical = 12.dp),
            decorationBox = { inner ->
                Box {
                    // §5's named exception: a placeholder may dim below the muted floor — it is a
                    // ghost affordance, not content.
                    if (query.isEmpty()) {
                        Text(
                            placeholder,
                            style = MaterialTheme.typography.bodyMedium,
                            color = muted.copy(alpha = 0.6f)
                        )
                    }
                    inner()
                }
            }
        )
        // Was an 18dp Icon (or a bare "×" with no padding at all) — both far under §14's 48dp.
        if (query.isNotEmpty()) {
            GlyphButton("✕", "Clear search", muted, { onQueryChange("") },
                style = MaterialTheme.typography.bodyMedium)
        }
    }
}
