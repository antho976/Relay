package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.BeachAccess
import androidx.compose.material.icons.rounded.CalendarMonth
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.DatePicker
import com.forge.app.ui.common.window.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.clickableLabeled
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter

private val DISPLAY_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMM d, yyyy")

/** Holidays: marked days that neither break the streak nor count as missed. */
@Composable
internal fun VacationPage(vm: SettingsViewModel, onBack: () -> Unit) {
    val vacations by vm.vacations.collectAsStateWithLifecycle()
    var showAdd by remember { mutableStateOf(false) }

    if (showAdd) {
        AddVacationDialog(
            onSave = { start, end, label -> vm.addVacation(start, end, label); showAdd = false },
            onDismiss = { showAdd = false }
        )
    }

    SettingsScaffold("Holidays", onBack) {
        SettingsGroup(footer = "Days inside a holiday don't break your training streak or count as missed.") {
            if (vacations.isEmpty()) {
                SettingsEmptyBlock(Icons.Rounded.BeachAccess, "No holidays yet", "Add one before you go and your streak waits for you.")
            } else {
                vacations.forEach { v ->
                    SettingsRowContainer {
                        SettingsIconTile(Icons.Rounded.BeachAccess)
                        SettingsRowText(v.label.ifBlank { "Holiday" }, formatRange(v.startDate, v.endDate))
                        IconButton(onClick = { vm.deleteVacation(v) }) {
                            Icon(Icons.Outlined.DeleteOutline, contentDescription = "Delete ${v.label.ifBlank { "holiday" }}", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
            SettingsActionRow("Add a holiday", icon = Icons.Rounded.Add, tone = TileTone.Accent) { showAdd = true }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddVacationDialog(onSave: (String, String, String) -> Unit, onDismiss: () -> Unit) {
    var startMs by remember { mutableStateOf<Long?>(null) }
    var endMs by remember { mutableStateOf<Long?>(null) }
    var label by remember { mutableStateOf("") }
    var picking by remember { mutableStateOf<String?>(null) } // "start" | "end" | null
    val backwards = startMs != null && endMs != null && endMs!! < startMs!!

    if (picking != null) {
        val state = rememberDatePickerState(initialSelectedDateMillis = if (picking == "start") startMs else endMs)
        DatePickerDialog(
            onDismissRequest = { picking = null },
            confirmButton = {
                TextButton(onClick = {
                    val sel = state.selectedDateMillis
                    if (picking == "start") startMs = sel else endMs = sel
                    picking = null
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { picking = null }) { Text("Cancel") } }
        ) { DatePicker(state = state) }
    }

    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        onDismissRequest = onDismiss,
        title = { Text("Add holiday") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                DateRow("Starts", startMs) { picking = "start" }
                DateRow("Ends", endMs) { picking = "end" }
                Spacer(Modifier.height(4.dp))
                OutlinedTextField(
                    value = label, onValueChange = { label = it },
                    label = { Text("Name, optional") }, singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = MaterialTheme.colorScheme.primary,
                        unfocusedBorderColor = MaterialTheme.colorScheme.outline,
                        cursorColor = MaterialTheme.colorScheme.primary,
                        focusedLabelColor = MaterialTheme.colorScheme.onSurface
                    ),
                    modifier = Modifier.fillMaxWidth()
                )
                if (backwards) {
                    Text("The end is before the start.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = startMs != null && endMs != null && !backwards,
                onClick = { onSave(toKey(startMs!!), toKey(endMs!!), label) }
            ) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

/** A date field: the calendar glyph, what it is, and the picked day (or a prompt to pick one). */
@Composable
private fun DateRow(label: String, ms: Long?, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .clickableLabeled("Pick the $label date", onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(Icons.Rounded.CalendarMonth, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(22.dp))
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(
                ms?.let { Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate().format(DISPLAY_FMT) } ?: "Pick a date",
                style = MaterialTheme.typography.bodyLarge,
                color = if (ms == null) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface
            )
        }
    }
}

/** DatePicker selects a UTC midnight instant; key it as the calendar date in UTC. */
private fun toKey(ms: Long): String =
    Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate().toString()

private fun formatRange(start: String, end: String): String {
    val s = runCatching { LocalDate.parse(start).format(DISPLAY_FMT) }.getOrDefault(start)
    val e = runCatching { LocalDate.parse(end).format(DISPLAY_FMT) }.getOrDefault(end)
    return if (start == end) s else "$s – $e"
}
