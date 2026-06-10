package com.forge.app.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.VacationPeriod
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter

private val DISPLAY_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMM d, yyyy")

@Composable
internal fun VacationPage(vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val vacations by vm.vacations.collectAsStateWithLifecycle()
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    var showAdd by remember { mutableStateOf(false) }

    if (showAdd) {
        AddVacationDialog(
            onSave = { start, end, label -> vm.addVacation(start, end, label); showAdd = false },
            onDismiss = { showAdd = false }
        )
    }

    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 56.dp)) {
        item("intro") {
            Text(
                "Mark a holiday and those days won't break your training streak or count as missed.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
            )
        }
        item("add") {
            Text(
                "+ Add holiday",
                style = MaterialTheme.typography.bodyMedium, color = accent,
                modifier = Modifier.fillMaxWidth().clickable { showAdd = true }.padding(horizontal = 24.dp, vertical = 14.dp)
            )
            SectionDivider()
        }
        if (vacations.isEmpty()) {
            item("empty") {
                Text(
                    "No holidays added.",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
                )
            }
        } else {
            items(vacations, key = { it.id }) { v ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(v.label.ifBlank { "Holiday" }, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text(formatRange(v.startDate, v.endDate), style = MaterialTheme.typography.labelSmall, color = muted)
                    }
                    Text(
                        "✕",
                        style = MaterialTheme.typography.bodyLarge, color = muted,
                        modifier = Modifier.clickable { vm.deleteVacation(v) }.padding(start = 16.dp)
                    )
                }
                HorizontalDivider(color = outline.copy(alpha = 0.12f), modifier = Modifier.padding(horizontal = 24.dp))
            }
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
        onDismissRequest = onDismiss,
        title = { Text("Add holiday") },
        text = {
            Column {
                DateRow("Start", startMs) { picking = "start" }
                Spacer(Modifier.height(8.dp))
                DateRow("End", endMs) { picking = "end" }
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = label, onValueChange = { label = it },
                    label = { Text("Label (optional)") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = startMs != null && endMs != null,
                onClick = { onSave(toKey(startMs!!), toKey(endMs!!), label) }
            ) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

@Composable
private fun DateRow(label: String, ms: Long?, onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Text(
            ms?.let { LocalDate.ofInstant(Instant.ofEpochMilli(it), ZoneOffset.UTC).format(DISPLAY_FMT) } ?: "Pick a date",
            style = MaterialTheme.typography.bodyMedium, color = accent
        )
    }
}

/** DatePicker selects a UTC midnight instant; key it as the calendar date in UTC. */
private fun toKey(ms: Long): String =
    LocalDate.ofInstant(Instant.ofEpochMilli(ms), ZoneOffset.UTC).toString()

private fun formatRange(start: String, end: String): String {
    val s = runCatching { LocalDate.parse(start).format(DISPLAY_FMT) }.getOrDefault(start)
    val e = runCatching { LocalDate.parse(end).format(DISPLAY_FMT) }.getOrDefault(end)
    return if (start == end) s else "$s – $e"
}
