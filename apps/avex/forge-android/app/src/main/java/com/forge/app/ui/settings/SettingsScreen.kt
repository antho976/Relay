package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var confirmReset by remember { mutableStateOf<ResetTarget?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("SETTINGS", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            // ─── Units ────────────────────────────────────────────────────────
            SectionHeader("UNITS")
            SettingsChipRow(
                label = "Weight unit",
                options = listOf("lb" to "lb (default)", "kg" to "kg"),
                selected = if (state.useKg) "kg" else "lb",
                onSelect = { viewModel.setUseKg(it == "kg") }
            )

            SectionDivider()

            // ─── Appearance ───────────────────────────────────────────────────
            SectionHeader("APPEARANCE")
            SettingsToggleRow(
                label = "AMOLED pure black",
                subtitle = "Replaces dark background with true black",
                checked = state.amoledMode,
                onCheckedChange = viewModel::setAmoledMode
            )
            Text("Accent color", style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.padding(top = 4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(vertical = 4.dp)) {
                listOf("#3D4F73" to "Navy", "#8B3535" to "Red",
                       "#4D6040" to "Olive", "#7A6435" to "Gold").forEach { (hex, label) ->
                    val isDefault = state.accentColorHex.isEmpty() && hex == "#3D4F73"
                    FilterChip(
                        selected = state.accentColorHex == hex || isDefault,
                        onClick = { viewModel.setAccentColorHex(hex) },
                        label = { Text(label, style = MaterialTheme.typography.labelMedium) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                            selectedLabelColor = MaterialTheme.colorScheme.onPrimaryContainer
                        )
                    )
                }
            }

            SectionDivider()

            // ─── Locale ───────────────────────────────────────────────────────
            SectionHeader("LOCALE")
            SettingsChipRow(
                label = "Date format",
                options = listOf("MMM d, yyyy" to "Jan 5", "dd/MM/yyyy" to "05/01", "MM/dd/yyyy" to "01/05"),
                selected = state.dateFormat,
                onSelect = viewModel::setDateFormat
            )
            SettingsChipRow(
                label = "Time format",
                options = listOf("12h" to "12h", "24h" to "24h"),
                selected = if (state.timeFormat24h) "24h" else "12h",
                onSelect = { viewModel.setTimeFormat24h(it == "24h") }
            )
            SettingsChipRow(
                label = "Week starts",
                options = listOf("Mon" to "Mon", "Sun" to "Sun"),
                selected = if (state.firstDayMonday) "Mon" else "Sun",
                onSelect = { viewModel.setFirstDayMonday(it == "Mon") }
            )

            SectionDivider()

            // ─── Session UX ───────────────────────────────────────────────────
            SectionHeader("SESSION UX")
            SettingsToggleRow(
                label = "Encouragement messages",
                subtitle = "Show motivational messages during sessions",
                checked = state.showEncouragement,
                onCheckedChange = viewModel::setShowEncouragement
            )
            SettingsToggleRow(
                label = "Compact set logging",
                subtitle = "Denser set rows for experienced users",
                checked = state.compactSetLogging,
                onCheckedChange = viewModel::setCompactSetLogging
            )

            SectionDivider()

            // ─── Feel ─────────────────────────────────────────────────────────
            SectionHeader("FEEL")
            SettingsChipRow(
                label = "Haptic feedback",
                options = listOf("off" to "Off", "light" to "Light", "medium" to "Medium", "strong" to "Strong"),
                selected = state.hapticStrength,
                onSelect = viewModel::setHapticStrength
            )

            SectionDivider()

            // ─── Notifications ────────────────────────────────────────────────
            SectionHeader("NOTIFICATIONS")
            SettingsToggleRow(
                label = "Quiet hours",
                subtitle = "Suppress timer + recap notifications",
                checked = state.quietHoursEnabled,
                onCheckedChange = viewModel::setQuietHoursEnabled
            )
            if (state.quietHoursEnabled) {
                SettingsHourRow(
                    label = "From",
                    hour = state.quietHoursStart,
                    onHourChange = viewModel::setQuietHoursStart
                )
                SettingsHourRow(
                    label = "Until",
                    hour = state.quietHoursEnd,
                    onHourChange = viewModel::setQuietHoursEnd
                )
            }

            SectionDivider()

            // ─── Overview tiles (#121) ────────────────────────────────────────
            SectionHeader("OVERVIEW TILES")
            Text("Tile order", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(bottom = 4.dp))
            val tileLabels = mapOf("gym" to "Gym", "cardio" to "Cardio", "trophies" to "Trophies")
            state.overviewTileOrder.forEachIndexed { idx, tileId ->
                Row(modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically) {
                    Text(tileLabels[tileId] ?: tileId, style = MaterialTheme.typography.bodyLarge)
                    Row {
                        if (idx > 0) TextButton(onClick = {
                            val order = state.overviewTileOrder.toMutableList()
                            order.add(idx - 1, order.removeAt(idx))
                            viewModel.setOverviewTileOrder(order)
                        }) { Text("↑") }
                        if (idx < state.overviewTileOrder.lastIndex) TextButton(onClick = {
                            val order = state.overviewTileOrder.toMutableList()
                            order.add(idx + 1, order.removeAt(idx))
                            viewModel.setOverviewTileOrder(order)
                        }) { Text("↓") }
                    }
                }
            }
            SectionDivider()
            Text("Tile visibility", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(vertical = 4.dp))
            listOf("gym" to "Gym tile", "cardio" to "Cardio tile", "trophies" to "Trophies tile",
                "streak" to "Streak card", "deload" to "Deload banner").forEach { (id, label) ->
                SettingsToggleRow(
                    label = label,
                    subtitle = if (id in state.hiddenOverviewTiles) "Hidden" else "Shown",
                    checked = id !in state.hiddenOverviewTiles,
                    onCheckedChange = { shown -> viewModel.setTileHidden(id, !shown) }
                )
            }

            SectionDivider()

            // ─── Custom warmup (#120) ─────────────────────────────────────────
            SectionHeader("CUSTOM WARMUP")
            Text(
                "Replace the built-in warmup for each day. One item per line.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 4.dp)
            )
            com.forge.app.program.Program.days.forEach { day ->
                WarmupDayEditor(
                    dayName = day.defaultName,
                    defaultItems = day.warmup,
                    onSave = { items -> viewModel.setCustomWarmup(day.key, items) }
                )
            }

            SectionDivider()

            // ─── Equipment context (#44) ──────────────────────────────────────
            SectionDivider()
            SectionHeader("EQUIPMENT")
            Text(
                "Select what equipment you have available. Swap suggestions will favor exercises you can do.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            // Equipment toggles — shown as filter chips
            @OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
            androidx.compose.foundation.layout.FlowRow(
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                com.forge.app.program.Equipment.entries.forEach { equip ->
                    val selected = state.availableEquipment.contains(equip.name)
                    FilterChip(
                        selected = selected,
                        onClick = {
                            val current = state.availableEquipment.toMutableSet()
                            if (selected) current.remove(equip.name) else current.add(equip.name)
                            viewModel.setAvailableEquipment(current)
                        },
                        label = { Text(equip.display, style = MaterialTheme.typography.labelSmall) }
                    )
                }
            }

            // ─── Privacy mode (#152) ──────────────────────────────────────────
            SectionDivider()
            SectionHeader("PRIVACY")
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Privacy mode", style = MaterialTheme.typography.bodyMedium)
                    Text("Blur app content in task switcher / screenshots",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Switch(
                    checked = state.privacyMode,
                    onCheckedChange = viewModel::setPrivacyMode
                )
            }

            // ─── Vacation mode (#135) ──────────────────────────────────────────
            SectionDivider()
            SectionHeader("VACATION MODE")
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Holiday / Vacation", style = MaterialTheme.typography.bodyMedium)
                    Text("Streak & deload counter pause during marked ranges",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                TextButton(onClick = { /* vacation management — full UI in future */ }) {
                    Text("Manage")
                }
            }

            // ─── Data ─────────────────────────────────────────────────────────
            SectionDivider()
            SectionHeader("DATA")

            // Export (#5, #6, #138)
            ExportRow("Export weekly data (JSON)", viewModel::exportWeeklyJson)
            ExportRow("Export sessions (CSV)", viewModel::exportSessionsCsv)
            ExportRow("Full backup (JSON)", viewModel::exportFullBackup)
            ExportRow("Load sample data (8 weeks)", viewModel::loadSampleData)
            ExportRow("Export last session (PDF)", viewModel::exportLastSessionPdf)
            SectionDivider()

            ResetTarget.entries.forEach { target ->
                SettingsDestructiveRow(label = target.label) { confirmReset = target }
            }
        }
    }

    // Export success snackbar
    val exportPath by viewModel.exportPath.collectAsStateWithLifecycle()
    exportPath?.let { path ->
        androidx.compose.material3.AlertDialog(
            onDismissRequest = viewModel::clearExportPath,
            title = { Text("Export saved") },
            text = { Text("File saved to app storage:\n${path.substringAfterLast("/")}") },
            confirmButton = { TextButton(onClick = viewModel::clearExportPath) { Text("OK") } }
        )
    }

    confirmReset?.let { target ->
        ResetConfirmDialog(
            target = target,
            onConfirm = {
                when (target) {
                    ResetTarget.SESSIONS -> viewModel.resetSessions()
                    ResetTarget.TROPHIES -> viewModel.resetTrophies()
                    ResetTarget.CARDIO -> viewModel.resetCardio()
                    ResetTarget.SETTINGS -> viewModel.resetSettings()
                    ResetTarget.FACTORY -> viewModel.factoryReset()
                }
                confirmReset = null
            },
            onDismiss = { confirmReset = null }
        )
    }
}

enum class ResetTarget(val label: String, val message: String) {
    SESSIONS("Reset session data", "Deletes all sessions, sets, and exercises logged. Cannot be undone."),
    TROPHIES("Reset trophies", "Clears all earned trophies. Cannot be undone."),
    CARDIO("Reset cardio", "Deletes all cardio entries. Cannot be undone."),
    SETTINGS("Reset app settings", "Restores all settings to defaults. Does not delete your data."),
    FACTORY("Factory reset", "Deletes ALL data and resets all settings. This cannot be undone.")
}

// ─── Section composables ──────────────────────────────────────────────────────

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.labelLarge,
        fontWeight = FontWeight.Bold,
        color = MaterialTheme.colorScheme.primary,
        letterSpacing = androidx.compose.ui.unit.TextUnit(1.5f, androidx.compose.ui.unit.TextUnitType.Sp),
        modifier = Modifier.padding(top = 12.dp, bottom = 4.dp)
    )
}

@Composable
private fun SectionDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(vertical = 8.dp),
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f)
    )
}

@Composable
private fun SettingsToggleRow(
    label: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun SettingsInfoRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
        Text(value, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsChipRow(
    label: String,
    options: List<Pair<String, String>>,
    selected: String,
    onSelect: (String) -> Unit
) {
    Column(modifier = Modifier.padding(vertical = 4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEach { (value, display) ->
                FilterChip(
                    selected = selected == value,
                    onClick = { onSelect(value) },
                    label = { Text(display, style = MaterialTheme.typography.labelMedium) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                        selectedLabelColor = MaterialTheme.colorScheme.primary
                    )
                )
            }
        }
    }
}

@Composable
private fun SettingsHourRow(label: String, hour: Int, onHourChange: (Int) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { onHourChange((hour - 1 + 24) % 24) }) { Text("−") }
            Text(
                "${hour.toString().padStart(2, '0')}:00",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            TextButton(onClick = { onHourChange((hour + 1) % 24) }) { Text("+") }
        }
    }
}

@Composable
private fun ExportRow(label: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurface)
        TextButton(onClick = onClick) { Text("Export") }
    }
}

@Composable
private fun SettingsDestructiveRow(label: String, onClick: () -> Unit) {
    TextButton(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
            contentColor = if (label.startsWith("Factory")) MaterialTheme.colorScheme.error
            else MaterialTheme.colorScheme.onSurface
        )
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.fillMaxWidth()
        )
    }
}

@Composable
private fun WarmupDayEditor(
    dayName: String,
    defaultItems: List<String>,
    onSave: (List<String>) -> Unit
) {
    var expanded by remember { mutableStateOf(false) }
    var text by remember { mutableStateOf(defaultItems.joinToString("\n")) }

    Column(modifier = Modifier.padding(vertical = 4.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(dayName, style = MaterialTheme.typography.bodyLarge)
            TextButton(onClick = { expanded = !expanded }) {
                Text(if (expanded) "Done" else "Edit")
            }
        }
        if (expanded) {
            androidx.compose.material3.OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text("One item per line") },
                minLines = 3,
                maxLines = 8
            )
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                TextButton(onClick = { text = defaultItems.joinToString("\n") }) { Text("Reset to default") }
                TextButton(onClick = {
                    onSave(text.lines().filter { it.isNotBlank() })
                    expanded = false
                }) { Text("Save") }
            }
        }
    }
}

@Composable
private fun ResetConfirmDialog(
    target: ResetTarget,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(target.label) },
        text = { Text(target.message) },
        confirmButton = {
            Button(
                onClick = onConfirm,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
            ) { Text("Confirm") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
