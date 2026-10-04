package com.tally.app.ui.entry

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.Hyphens
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.tally.app.data.db.CategoryEntity
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.common.selectableColors
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

private const val NOTE_PLACEHOLDER = "Note (merchant, what it was)"

/**
 * The category grid: four tiles a row at normal size, fewer as the font grows so no name breaks
 * mid-word. One radio group; the picked tile takes the accent ring and wash.
 */
@Composable
internal fun CategoryGrid(
    categories: List<CategoryEntity>,
    pickedId: Long?,
    onPick: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    val scale = LocalDensity.current.fontScale
    val columns = when {
        scale >= 1.75f -> 2
        scale > 1.3f -> 3
        else -> 4
    }
    val rows = remember(categories, columns) { categories.chunked(columns) }
    Column(modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        rows.forEach { row ->
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                row.forEach { category ->
                    CategoryTile(
                        category = category,
                        picked = category.id == pickedId,
                        onClick = { onPick(category.id) },
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** One category: its badge over its name, the whole tile one radio target. */
@OptIn(ExperimentalTextApi::class)
@Composable
private fun CategoryTile(
    category: CategoryEntity,
    picked: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val (border, fill) = selectableColors(picked)
    val shape = RoundedCornerShape(14.dp)
    Column(
        modifier
            .clip(shape)
            .background(fill)
            .border(if (picked) 1.5.dp else 1.dp, border, shape)
            .bounceClick(role = Role.RadioButton, onClick = onClick)
            .semantics { selected = picked }
            .padding(horizontal = 3.dp, vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        CategoryBadge(category.icon, category.color, size = 40.dp)
        Text(
            category.name,
            modifier = Modifier.fillMaxWidth(),
            style = MaterialTheme.typography.bodySmall.copy(hyphens = Hyphens.Auto),
            color = if (picked) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            minLines = 2,
        )
    }
}

/**
 * The note: a filled rounded field with its glyph, the search-field look from Avex. Done closes
 * the keyboard and brings the keypad back.
 */
@Composable
internal fun NoteField(value: String, onChange: (String) -> Unit, modifier: Modifier = Modifier) {
    val focus = LocalFocusManager.current
    val shape = RoundedCornerShape(16.dp)
    BasicTextField(
        value = value,
        onValueChange = onChange,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = "Note" },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = ROW_PAD, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                GlyphBadge(Icons.Rounded.EditNote)
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) {
                        Text(
                            NOTE_PLACEHOLDER,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    inner()
                }
            }
        },
    )
}

/**
 * The Material 3 calendar on the warm ladder. The picker speaks UTC midnight; the day is read back
 * in UTC so the picked calendar day never shifts by the zone offset.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun EntryDatePicker(date: LocalDate, onPick: (LocalDate) -> Unit, onDismiss: () -> Unit) {
    val pickerState = rememberDatePickerState(
        initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
    )
    val colors = DatePickerDefaults.colors(
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        headlineContentColor = MaterialTheme.colorScheme.onBackground,
        weekdayContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
        dividerColor = MaterialTheme.colorScheme.outlineVariant,
    )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = {
                    pickerState.selectedDateMillis?.let { picked ->
                        onPick(Instant.ofEpochMilli(picked).atZone(ZoneOffset.UTC).toLocalDate())
                    }
                    onDismiss()
                },
            ) { Text("Set date") }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurfaceVariant),
            ) { Text("Cancel") }
        },
        colors = colors,
    ) {
        DatePicker(state = pickerState, colors = colors)
    }
}
