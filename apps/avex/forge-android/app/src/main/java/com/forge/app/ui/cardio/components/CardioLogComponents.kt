package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioType

/**
 * The compact sheet header — a small date label that doubles as the date picker trigger (so there's
 * no separate "When?" section). Tapping anywhere on it opens the picker.
 */
@Composable
internal fun CardioLogHeroItem(dateHeader: String, muted: Color, onBg: Color, outline: Color, onPickDate: () -> Unit) {
    Column(
        modifier = Modifier
            .clickable(onClick = onPickDate)
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 4.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(dateHeader, style = MaterialTheme.typography.labelMedium, color = onBg, letterSpacing = 1.sp)
            Text("· change", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Spacer(Modifier.height(12.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
    }
}

/**
 * The activity-type selector — shows the current pick (icon + name) and opens a dropdown of all
 * types. A compact single-row control in place of the old 12-pill grid.
 */
@Composable
internal fun ActivityDropdown(
    selected: CardioType,
    onSelect: (CardioType) -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(10.dp))
                .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(10.dp))
                .clickable { open = true }
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Icon(selected.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(18.dp))
                Text(selected.displayName, style = MaterialTheme.typography.bodyLarge, color = onBg)
            }
            Icon(Icons.Filled.ArrowDropDown, contentDescription = "Choose activity", tint = muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            CardioType.entries.forEach { t ->
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(t.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(18.dp))
                            Text(t.displayName, color = onBg)
                        }
                    },
                    onClick = { onSelect(t); open = false }
                )
            }
        }
    }
}

/**
 * A quiet, compact form group — a small uppercase label (vs the old big italic header) over its
 * content. Decluttered so the log sheet reads as a short form, not a stack of tall titled cards.
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
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
            if (optional) {
                Text("OPTIONAL", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.4f), fontSize = 8.sp, letterSpacing = 1.sp)
            }
        }
        Spacer(Modifier.height(8.dp))
        content()
        Spacer(Modifier.height(10.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.15f))
    }
}

/**
 * A clickable section header that expands/collapses its body — used to tuck the optional
 * effort / HR-zone / interval inputs out of the default (short) form. Chevron rotates with state.
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
        Spacer(Modifier.height(16.dp))
        Row(
            modifier = Modifier.fillMaxWidth().clickable(onClick = onToggle),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
            Text(if (expanded) "−" else "+", style = MaterialTheme.typography.bodyLarge, color = onBg)
        }
        Spacer(Modifier.height(if (expanded) 10.dp else 14.dp))
        if (!expanded) HorizontalDivider(color = outline.copy(alpha = 0.15f))
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
        HorizontalDivider(thickness = 1.dp, color = outline.copy(alpha = 0.45f))
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
        HorizontalDivider(thickness = 1.dp, color = outline.copy(alpha = 0.45f), modifier = Modifier.width(72.dp))
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
    val bgColor = if (selected) onBg else Color.Transparent
    val textColor = if (selected) bg else muted.copy(alpha = 0.65f)
    val borderColor = if (selected) onBg else outline.copy(alpha = 0.4f)
    Box(
        modifier = Modifier
            .border(1.dp, borderColor, RoundedCornerShape(4.dp))
            .background(bgColor, RoundedCornerShape(4.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = textColor, letterSpacing = 1.sp)
    }
}

internal fun sanitizeDecimal(input: String): String {
    val filtered = input.filter { it.isDigit() || it == '.' }
    val firstDot = filtered.indexOf('.')
    val collapsed = if (firstDot == -1) filtered
    else filtered.substring(0, firstDot + 1) + filtered.substring(firstDot + 1).replace(".", "")
    return collapsed.take(6)
}
