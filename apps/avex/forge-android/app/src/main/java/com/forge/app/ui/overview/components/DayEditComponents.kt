package com.forge.app.ui.overview.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.program.ExercisePlan

@Composable
internal fun NameSection(
    defaultName: String,
    customName: String?,
    onSave: (String) -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    var text by rememberSaveable(customName) { mutableStateOf(customName ?: "") }
    var isEditing by remember { mutableStateOf(false) }
    val focusRequester = remember { FocusRequester() }

    LaunchedEffect(isEditing) { if (isEditing) focusRequester.requestFocus() }

    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("NAME", style = MaterialTheme.typography.labelSmall, letterSpacing = 1.5.sp, color = muted, fontSize = 10.sp)
            if (isEditing) {
                BasicTextField(
                    value = text,
                    onValueChange = { text = it },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
                    cursorBrush = SolidColor(onBg),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onSave(text); isEditing = false }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focusRequester)
                        .onFocusChanged {
                            if (!it.isFocused) { isEditing = false; if (text != (customName ?: "")) onSave(text) }
                        },
                    decorationBox = { inner ->
                        if (text.isEmpty()) Text(defaultName, style = MaterialTheme.typography.bodyMedium, color = muted.copy(alpha = 0.4f))
                        inner()
                    }
                )
            } else {
                Text(if (text.isEmpty()) defaultName else text,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (text.isEmpty()) muted.copy(alpha = 0.4f) else onBg,
                    modifier = Modifier.fillMaxWidth().clickable { isEditing = true })
            }
        }
        if (customName != null) {
            Text("reset", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f), fontSize = 9.sp,
                modifier = Modifier.clickable { text = ""; isEditing = false; onSave("") })
        }
    }
}

@Composable
internal fun ExerciseEditRow(
    plan: ExercisePlan,
    customization: ProgramCustomization?,
    onSetsChange: (Int) -> Unit,
    onRepsChange: (String) -> Unit,
    onToggleRemoved: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    val removed = customization?.removed ?: false
    val currentSets = if (customization != null && customization.setsOverride > 0) customization.setsOverride else plan.sets
    var repsText by rememberSaveable(customization?.repRangeOverride, plan.reps) {
        mutableStateOf(customization?.repRangeOverride ?: plan.reps)
    }
    val textAlpha = if (removed) 0.3f else 1f

    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text(plan.name, style = MaterialTheme.typography.bodyMedium, color = onBg.copy(alpha = textAlpha), modifier = Modifier.weight(1f))
            Text(if (removed) "restore" else "remove", style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.55f), fontSize = 9.sp, modifier = Modifier.clickable { onToggleRemoved() })
        }
        if (!removed) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                Text("−", style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.clickable { onSetsChange((currentSets - 1).coerceAtLeast(1)) }.padding(end = 8.dp, top = 2.dp, bottom = 2.dp))
                Text("$currentSets", style = MaterialTheme.typography.bodySmall, color = onBg)
                Text("+", style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.clickable { onSetsChange(currentSets + 1) }.padding(start = 8.dp, top = 2.dp, bottom = 2.dp))
                Text(" sets  ·  ", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.4f))
                BasicTextField(
                    value = repsText,
                    onValueChange = { repsText = it },
                    singleLine = true,
                    textStyle = TextStyle(color = muted, fontSize = MaterialTheme.typography.bodySmall.fontSize),
                    cursorBrush = SolidColor(onBg),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onRepsChange(repsText) }),
                    modifier = Modifier.size(width = 56.dp, height = 20.dp).onFocusChanged { if (!it.isFocused) onRepsChange(repsText) }
                )
                Text(" reps", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.4f))
            }
        }
    }
}

@Composable
internal fun AddWarmupRow(onAdd: (String) -> Unit, muted: Color, outline: Color, onBg: Color) {
    var text by remember { mutableStateOf("") }
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        BasicTextField(
            value = text,
            onValueChange = { text = it },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodySmall.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (text.isNotBlank()) { onAdd(text); text = "" } }),
            modifier = Modifier.weight(1f)
                .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.3f)), RoundedCornerShape(4.dp))
                .padding(horizontal = 10.dp, vertical = 8.dp),
            decorationBox = { inner ->
                if (text.isEmpty()) Text("Add warmup item…", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.35f))
                inner()
            }
        )
        Text("+", style = MaterialTheme.typography.bodyLarge,
            color = if (text.isNotBlank()) onBg else muted.copy(alpha = 0.3f),
            modifier = Modifier.clickable { if (text.isNotBlank()) { onAdd(text); text = "" } })
    }
}
