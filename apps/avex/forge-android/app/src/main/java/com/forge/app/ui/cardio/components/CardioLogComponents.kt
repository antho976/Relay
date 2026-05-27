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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun CardioLogHeroItem(dateHeader: String, muted: Color, onBg: Color, outline: Color) {
    Column(
        modifier = Modifier
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 4.dp)
    ) {
        Text(dateHeader, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text("Today I ", style = MaterialTheme.typography.displayMedium, color = onBg)
            Text("moved.", style = MaterialTheme.typography.displayMedium, color = onBg, fontStyle = FontStyle.Italic)
        }
        Spacer(Modifier.height(16.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
    }
}

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
        Spacer(Modifier.height(20.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(label, style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
            if (optional) {
                Text("OPTIONAL", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f), fontSize = 9.sp, letterSpacing = 1.sp)
            }
        }
        Spacer(Modifier.height(12.dp))
        content()
        Spacer(Modifier.height(16.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.2f))
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
            Box(modifier = Modifier.width(72.dp)) {
                BasicTextField(
                    value = value,
                    onValueChange = onValueChange,
                    textStyle = MaterialTheme.typography.headlineMedium.copy(color = onBg),
                    singleLine = true,
                    cursorBrush = SolidColor(accent),
                    keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
                    decorationBox = { inner ->
                        Box {
                            if (value.isEmpty()) {
                                Text(placeholder, style = MaterialTheme.typography.headlineMedium, color = muted.copy(alpha = 0.35f))
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
