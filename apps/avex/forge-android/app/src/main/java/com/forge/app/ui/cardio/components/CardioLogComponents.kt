package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled

/**
 * The compact sheet header — the entry's date as a small outlined capsule that IS the date-picker
 * trigger (interactive, so it earns its border; no separate "When?" section, no "· change" tag).
 */
@Composable
internal fun CardioLogHeroItem(dateHeader: String, muted: Color, onBg: Color, outline: Color, onPickDate: () -> Unit) {
    Row(Modifier.padding(horizontal = 24.dp).padding(top = 8.dp, bottom = 4.dp)) {
        Row(
            modifier = Modifier
                .clip(RoundedCornerShape(50))
                .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                .bounceClick(onClick = onPickDate)
                .padding(horizontal = 14.dp, vertical = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(dateHeader, style = MaterialTheme.typography.labelMedium, color = onBg, letterSpacing = 1.sp)
            Text("▾", style = MaterialTheme.typography.labelSmall, color = muted)
        }
    }
}

/**
 * The activity-type selector — shows the current pick (icon + name) and opens a dropdown of all
 * types: the built-in [CardioType]s, then the user's custom activities (GYMAP-37), then an
 * "add custom activity" row that opens the create dialog. A compact single-row control.
 */
@Composable
internal fun ActivityDropdown(
    selected: CardioActivity,
    onSelect: (CardioActivity) -> Unit,
    onAddCustom: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    var open by remember { mutableStateOf(false) }
    val customs = com.forge.app.ui.cardio.LocalCardioTypes.current
    val accent = MaterialTheme.colorScheme.primary
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
                .bounceClick { open = true }
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Icon(selected.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(18.dp))
                Text(selected.displayName, style = MaterialTheme.typography.bodyLarge, color = onBg)
            }
            Text(
                "▾",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                modifier = Modifier.semantics { contentDescription = "Choose activity" }
            )
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
            HorizontalDivider(color = outline.copy(alpha = 0.25f))
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
 * A quiet, compact form group — a small uppercase label over its content, separated from its
 * neighbours by air alone (form pages carry no dividers, §3).
 */
@Composable
internal fun FormSection(
    label: String,
    optional: Boolean,
    muted: Color,
    onBg: Color,
    outline: Color,
    content: @Composable () -> Unit
) {
    Column(Modifier.padding(horizontal = 24.dp)) {
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
            if (optional) {
                Text("OPTIONAL", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 8.sp, letterSpacing = 1.sp)
            }
        }
        Spacer(Modifier.height(8.dp))
        content()
    }
}

/**
 * A clickable section header that expands/collapses its body — used to tuck the optional
 * effort / HR-zone / interval inputs out of the default (short) form.
 */
@Composable
internal fun ExpanderHeader(
    label: String,
    expanded: Boolean,
    muted: Color,
    onBg: Color,
    outline: Color,
    onToggle: () -> Unit
) {
    Column(Modifier.padding(horizontal = 24.dp)) {
        Spacer(Modifier.height(18.dp))
        Row(
            modifier = Modifier.fillMaxWidth()
                .clickableLabeled(if (expanded) "Hide extra fields" else "Show extra fields", onClick = onToggle),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
            Text(if (expanded) "−" else "+", style = MaterialTheme.typography.bodyLarge, color = onBg)
        }
        Spacer(Modifier.height(if (expanded) 10.dp else 14.dp))
    }
}

/**
 * A narrow, labelled number field for the side-by-side duration/distance row — small uppercase
 * caption over an inline value+unit with an underline. Two of these sit in a weighted Row.
 */
@Composable
internal fun CompactNumberField(
    caption: String,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    unit: String,
    keyboardType: KeyboardType,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier) {
        Text(caption.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(modifier = Modifier.width(52.dp)) {
                BasicTextField(
                    value = value,
                    onValueChange = onValueChange,
                    textStyle = MaterialTheme.typography.titleLarge.copy(color = onBg),
                    singleLine = true,
                    cursorBrush = SolidColor(accent),
                    keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
                    decorationBox = { inner ->
                        Box {
                            if (value.isEmpty()) {
                                Text(placeholder, style = MaterialTheme.typography.titleLarge, color = muted.copy(alpha = 0.35f))
                            }
                            inner()
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
            Text(unit, style = MaterialTheme.typography.labelSmall, color = muted, modifier = Modifier.padding(bottom = 3.dp))
        }
        Spacer(Modifier.height(4.dp))
        HorizontalDivider(thickness = 1.dp, color = outline.copy(alpha = 0.35f))
    }
}

@Composable
internal fun NumberInputRow(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    unit: String,
    keyboardType: KeyboardType,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Column {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Box(modifier = Modifier.width(64.dp)) {
                BasicTextField(
                    value = value,
                    onValueChange = onValueChange,
                    textStyle = MaterialTheme.typography.titleLarge.copy(color = onBg),
                    singleLine = true,
                    cursorBrush = SolidColor(accent),
                    keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
                    decorationBox = { inner ->
                        Box {
                            if (value.isEmpty()) {
                                Text(placeholder, style = MaterialTheme.typography.titleLarge, color = muted.copy(alpha = 0.35f))
                            }
                            inner()
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
            Text(unit, style = MaterialTheme.typography.bodyMedium, color = muted)
        }
        Spacer(Modifier.height(4.dp))
        HorizontalDivider(thickness = 1.dp, color = outline.copy(alpha = 0.35f), modifier = Modifier.width(72.dp))
    }
}

@Composable
internal fun PillChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    onBg: Color,
    bg: Color,
    muted: Color,
    outline: Color
) {
    // Selected = the §5 accent ladder (accent border over a primaryContainer wash), never a white fill.
    val accent = MaterialTheme.colorScheme.primary
    val bgColor = if (selected) accent.copy(alpha = 0.15f) else Color.Transparent
    val textColor = if (selected) onBg else muted.copy(alpha = 0.65f)
    val borderColor = if (selected) accent else outline.copy(alpha = 0.35f)
    Box(
        modifier = Modifier
            .border(1.dp, borderColor, RoundedCornerShape(4.dp))
            .background(bgColor, RoundedCornerShape(4.dp))
            .bounceClick(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = textColor, letterSpacing = 1.sp)
    }
}

/** A bare number for seeding an editable field — drops a whole value's trailing ".0" (e.g. incline). */
internal fun plainDecimalInput(v: Double): String =
    if (v % 1.0 == 0.0) v.toInt().toString() else v.toString()

internal fun sanitizeDecimal(input: String): String {
    val filtered = input.filter { it.isDigit() || it == '.' }
    val firstDot = filtered.indexOf('.')
    val collapsed = if (firstDot == -1) filtered
    else filtered.substring(0, firstDot + 1) + filtered.substring(firstDot + 1).replace(".", "")
    return collapsed.take(6)
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
