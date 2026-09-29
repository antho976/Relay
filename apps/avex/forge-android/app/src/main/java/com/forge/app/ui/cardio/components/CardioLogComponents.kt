@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import com.forge.app.ui.common.window.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.common.ForgeGlyphBadge
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.GROUP_OUTER
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape
import com.forge.app.domain.units.filterDecimalInput

/**
 * The compact sheet header — the entry's date and start time as small filled capsules that ARE the
 * date / time-picker triggers (no separate "When?" section, no "· change" tag). The time capsule
 * (GYMAP-33) is hidden on rest days, which have no start time.
 */
@Composable
internal fun CardioLogHeroItem(
    dateHeader: String,
    timeHeader: String,
    showTime: Boolean,
    onPickDate: () -> Unit,
    onPickTime: () -> Unit
) {
    // Wraps, so at a large font scale the time drops under the date rather than squeezing.
    FlowRow(
        Modifier.padding(horizontal = 24.dp).padding(top = 4.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        HeroPickerCapsule(text = dateHeader, label = "Pick date", onClick = onPickDate)
        if (showTime) {
            HeroPickerCapsule(text = timeHeader, label = "Pick start time", onClick = onPickTime)
        }
    }
}

@Composable
private fun HeroPickerCapsule(text: String, label: String, onClick: () -> Unit) {
    ForgeChromeButton(onClick = onClick, label = label) {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onBackground,
                letterSpacing = 1.sp
            )
            DropGlyph()
        }
    }
}

/** The drawn "opens a menu" caret on a picker capsule or row. Decorative: the row speaks. */
@Composable
private fun DropGlyph() {
    Icon(
        Icons.Filled.KeyboardArrowDown,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.size(18.dp)
    )
}

/**
 * The activity-type selector — one filled row showing the current pick on its glyph badge, which
 * opens a dropdown of all types: the built-in [CardioType]s, then the user's custom activities
 * (GYMAP-37), then an "add custom activity" row that opens the create dialog.
 */
@Composable
internal fun ActivityDropdown(
    selected: CardioActivity,
    onSelect: (CardioActivity) -> Unit,
    onAddCustom: () -> Unit
) {
    var open by remember { mutableStateOf(false) }
    val customs = com.forge.app.ui.cardio.LocalCardioTypes.current
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(GROUP_OUTER))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .clickableLabeled("Choose activity") { open = true }
                .padding(horizontal = ROW_H, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            ForgeGlyphBadge(selected.icon, selected = true)
            Text(
                selected.displayName,
                style = MaterialTheme.typography.bodyLarge,
                color = onBg,
                modifier = Modifier.weight(1f)
            )
            DropGlyph()
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            val builtins = CardioType.entries.map { CardioActivity.Builtin(it) }
            val customActivities = customs.map { CardioActivity.Custom(it) }
            (builtins + customActivities).forEach { activity ->
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(activity.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(18.dp))
                            Text(activity.displayName, color = onBg)
                        }
                    },
                    onClick = { onSelect(activity); open = false }
                )
            }
            // Air, not a rule, between the list and its one action.
            Spacer(Modifier.height(6.dp))
            DropdownMenuItem(
                text = {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("+", style = MaterialTheme.typography.titleMedium, color = accent)
                        Text("Add custom activity", color = accent)
                    }
                },
                onClick = { open = false; onAddCustom() }
            )
        }
    }
}

/**
 * A form group on the sheet's 24dp gutter: the mono anchor (with OPTIONAL as its reading) over one
 * group of filled members, and an optional footnote under it.
 */
@Composable
internal fun FormSection(
    label: String,
    optional: Boolean,
    footer: (@Composable () -> Unit)? = null,
    content: @Composable () -> Unit
) {
    Column(Modifier.padding(horizontal = 24.dp).padding(top = 20.dp)) {
        ForgeGroupSection(label, meta = if (optional) "Optional" else null, footer = footer, content = content)
    }
}

/**
 * A clickable section header that expands/collapses its body — used to tuck the optional
 * effort / HR-zone / interval inputs out of the default (short) form. Drawn as a filled capsule so
 * it reads as the button it is.
 */
@Composable
internal fun ExpanderHeader(label: String, expanded: Boolean, onToggle: () -> Unit) {
    Row(Modifier.padding(horizontal = 24.dp).padding(top = 20.dp)) {
        ForgeChromeButton(onClick = onToggle, label = if (expanded) "Hide extra fields" else "Show extra fields") {
            Row(
                Modifier.padding(horizontal = 18.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
                Text(
                    if (expanded) "−" else "+",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

/**
 * A pick-many text tile on one member of a connected grid (the weather tags). Pick-many, so a lit
 * tile keeps the group's shape and says it is on with the ring and wash alone.
 */
@Composable
internal fun CardioTextTile(
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected = false)
    Text(
        label,
        style = MaterialTheme.typography.bodyMedium,
        color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.Checkbox }
            .padding(horizontal = 8.dp, vertical = 16.dp)
    )
}

/** A pick-one glyph tile (the custom-activity icon grid): the glyph alone, on its own member. */
@Composable
internal fun CardioGlyphTile(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected)
    Box(
        modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton; contentDescription = label }
            .padding(vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(22.dp)
        )
    }
}

/**
 * The note: the one multi-line field, so it stays a box, but a FILLED one (the group surface at the
 * group's outer radius) rather than an outline or an underline.
 */
@Composable
internal fun FilledNoteField(value: String, onValueChange: (String) -> Unit, placeholder: String) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        minLines = 3,
        maxLines = 6,
        decorationBox = { inner ->
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(GROUP_OUTER))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .padding(horizontal = ROW_H, vertical = 16.dp)
            ) {
                // A placeholder may dim below the muted floor: a ghost affordance, not content.
                if (value.isEmpty()) {
                    Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = muted.copy(alpha = 0.6f))
                }
                inner()
            }
        },
        modifier = Modifier.fillMaxWidth().semantics { contentDescription = "Note" }
    )
}

/** A bare number for seeding an editable field — drops a whole value's trailing ".0" (e.g. incline). */
internal fun plainDecimalInput(v: Double): String =
    if (v % 1.0 == 0.0) v.toInt().toString() else v.toString()

internal fun sanitizeDecimal(input: String): String = filterDecimalInput(input).take(6)

/**
 * The duration field's text, held to one day. The field allows five characters, so a typo like
 * "99999" (or "99:99") would otherwise be stored as-is and inflate the week's totals, the streak,
 * the records and the cardio goals. The bound is the importers' (a longer single entry is a slip).
 */
internal fun capDurationText(input: String): String =
    if (parseDurationMin(input) > com.forge.app.data.importer.ImportBounds.MAX_CARDIO_MINUTES) {
        com.forge.app.data.importer.ImportBounds.MAX_CARDIO_MINUTES.toString()
    } else input

/**
 * The distance field's text (in the display unit), held to the importers' single-entry bound so the
 * field shows what will be saved rather than a "1500" that is silently stored as 1000 km. The miles
 * bound is rounded DOWN to one decimal so the capped text itself never parses past the bound.
 */
internal fun capDistanceText(input: String, useMiles: Boolean): String {
    val maxKm = com.forge.app.data.importer.ImportBounds.MAX_CARDIO_DISTANCE_KM
    val km = com.forge.app.domain.units.parseToKm(input, useMiles) ?: return input
    if (km <= maxKm) return input
    val maxDisplay = kotlin.math.floor(com.forge.app.domain.units.toDisplayDistance(maxKm, useMiles) * 10.0) / 10.0
    return plainDecimalInput(maxDisplay)
}

/** Keep digits and a single colon, capped at "HH:MM" width, for the duration field (GYMAP-41). */
internal fun sanitizeDuration(input: String): String {
    val filtered = input.filter { it.isDigit() || it == ':' }
    val firstColon = filtered.indexOf(':')
    val collapsed = if (firstColon == -1) filtered
    else filtered.substring(0, firstColon + 1) + filtered.substring(firstColon + 1).replace(":", "")
    return collapsed.take(5)
}

/**
 * Parse the duration field into whole minutes (GYMAP-41). Accepts either a plain minute count
 * ("90" -> 90) or an H:MM clock value ("1:30" -> 90); a lone number stays minutes so existing
 * plain-number entry is unchanged. The store has no seconds column, so no sub-minute component is
 * read; malformed parts count as 0.
 */
internal fun parseDurationMin(input: String): Int {
    val t = input.trim()
    if (t.isEmpty()) return 0
    val colon = t.indexOf(':')
    if (colon < 0) return t.toIntOrNull() ?: 0
    val hours = t.substring(0, colon).toIntOrNull() ?: 0
    val minutes = t.substring(colon + 1).toIntOrNull() ?: 0
    return hours * 60 + minutes
}
