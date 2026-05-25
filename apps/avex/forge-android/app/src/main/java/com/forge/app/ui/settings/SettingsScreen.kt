@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle

enum class SettingsPage(val title: String) {
    Appearance("Appearance"),
    Format("Units & format"),
    Session("Session"),
    Notifications("Notifications"),
    Tiles("Overview tiles"),
    Equipment("Equipment"),
    Privacy("Privacy")
}

private data class SettingsRow(val label: String, val tags: String, val page: SettingsPage)

private val ALL_ROWS = listOf(
    SettingsRow("Appearance", "amoled dark theme accent compact logging display", SettingsPage.Appearance),
    SettingsRow("Units & format", "kg lb weight date time week timezone locale", SettingsPage.Format),
    SettingsRow("Session", "encouragement haptic feedback vibration", SettingsPage.Session),
    SettingsRow("Notifications", "quiet hours notify suppress", SettingsPage.Notifications),
    SettingsRow("Overview tiles", "tiles order visible hidden gym cardio trophies streak deload", SettingsPage.Tiles),
    SettingsRow("Equipment", "equipment available barbell dumbbell cable machine", SettingsPage.Equipment),
    SettingsRow("Privacy", "privacy mode blur screenshot", SettingsPage.Privacy)
)

enum class ResetTarget(val label: String, val message: String) {
    SESSIONS("Reset session data", "Deletes all sessions, sets, and exercises logged. Cannot be undone."),
    TROPHIES("Reset trophies", "Clears all earned trophies. Cannot be undone."),
    CARDIO("Reset cardio", "Deletes all cardio entries. Cannot be undone."),
    SETTINGS("Reset app settings", "Restores all settings to defaults. Does not delete your data."),
    FACTORY("Factory reset", "Deletes ALL data and resets all settings. This cannot be undone.")
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val exportPath by viewModel.exportPath.collectAsStateWithLifecycle()

    var currentPage by remember { mutableStateOf<SettingsPage?>(null) }
    var searchActive by remember { mutableStateOf(false) }
    var searchQuery by remember { mutableStateOf("") }
    var confirmReset by remember { mutableStateOf<ResetTarget?>(null) }
    var showDataDialog by remember { mutableStateOf(false) }

    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(searchActive) {
        if (searchActive) focusRequester.requestFocus()
    }

    BackHandler(enabled = currentPage != null || searchActive) {
        if (currentPage != null) currentPage = null
        else { searchActive = false; searchQuery = "" }
    }

    val displayRows = if (searchQuery.isBlank()) ALL_ROWS
    else ALL_ROWS.filter {
        searchQuery.lowercase().let { q -> q in it.label.lowercase() || q in it.tags }
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    when {
                        currentPage != null -> {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                Text("•", style = MaterialTheme.typography.bodyMedium, color = muted)
                                Text("Forge", style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                            }
                        }
                        searchActive -> {
                            BasicTextField(
                                value = searchQuery,
                                onValueChange = { searchQuery = it },
                                textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
                                cursorBrush = SolidColor(onBg),
                                singleLine = true,
                                decorationBox = { inner ->
                                    Box {
                                        if (searchQuery.isEmpty()) {
                                            Text(
                                                "Search settings…",
                                                style = MaterialTheme.typography.bodyMedium,
                                                color = muted.copy(alpha = 0.45f),
                                                fontStyle = FontStyle.Italic
                                            )
                                        }
                                        inner()
                                    }
                                },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .focusRequester(focusRequester)
                            )
                        }
                        else -> {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                Text("•", style = MaterialTheme.typography.bodyMedium, color = muted)
                                Text("Forge", style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                            }
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = {
                        when {
                            currentPage != null -> currentPage = null
                            searchActive -> { searchActive = false; searchQuery = "" }
                            else -> onBack()
                        }
                    }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    when {
                        currentPage != null -> {
                            Text(
                                currentPage!!.title.uppercase(),
                                style = MaterialTheme.typography.labelSmall,
                                letterSpacing = 1.5.sp,
                                color = muted,
                                modifier = Modifier.padding(end = 16.dp)
                            )
                        }
                        searchActive -> {
                            if (searchQuery.isNotEmpty()) {
                                TextButton(onClick = { searchQuery = "" }) {
                                    Text("×", style = MaterialTheme.typography.bodyLarge, color = muted)
                                }
                            }
                        }
                        else -> {
                            IconButton(onClick = { searchActive = true }) {
                                Icon(Icons.Filled.Search, contentDescription = "Search", tint = muted)
                            }
                            Text(
                                "SETTINGS",
                                style = MaterialTheme.typography.labelSmall,
                                letterSpacing = 2.sp,
                                color = muted,
                                modifier = Modifier.padding(end = 16.dp)
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when (val page = currentPage) {
            null -> MainList(
                state = state,
                displayRows = displayRows,
                searchQuery = searchQuery,
                modifier = Modifier.fillMaxSize().padding(inner),
                onOpenPage = { currentPage = it },
                onOpenDataDialog = { showDataDialog = true },
                onResetTarget = { confirmReset = it }
            )
            SettingsPage.Appearance -> AppearancePage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Format -> FormatPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Session -> SessionPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Notifications -> NotificationsPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Tiles -> TilesPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Equipment -> EquipmentPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Privacy -> PrivacyPage(state, viewModel, Modifier.padding(inner))
        }
    }

    if (showDataDialog) {
        DataExportDialog(viewModel = viewModel, onDismiss = { showDataDialog = false })
    }

    exportPath?.let { path ->
        AlertDialog(
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

// ─── Main list ────────────────────────────────────────────────────────────────

@Composable
private fun MainList(
    state: SettingsUiState,
    displayRows: List<SettingsRow>,
    searchQuery: String,
    modifier: Modifier,
    onOpenPage: (SettingsPage) -> Unit,
    onOpenDataDialog: () -> Unit,
    onResetTarget: (ResetTarget) -> Unit
) {
    LazyColumn(modifier = modifier, contentPadding = PaddingValues(bottom = 56.dp)) {
        displayRows.forEach { row ->
            item(row.page.name) {
                SettingsNavRow(row.label, rowSubtitle(row.page, state)) { onOpenPage(row.page) }
                SectionDivider()
            }
        }

        if (searchQuery.isBlank()) {
            item("vacation") {
                SettingsNavRow("Holiday / Vacation", "Streak & deload counter pause") { /* future */ }
                SectionDivider()
            }
            item("data") {
                SectionLabel("DATA")
                SettingsNavRow("Export data", "Sessions · weekly · full backup · PDF") { onOpenDataDialog() }
                SectionDivider()
            }
            item("reset") {
                SectionLabel("RESET")
                ResetTarget.entries.forEach { target ->
                    DestructiveRow(target.label, isFactory = target == ResetTarget.FACTORY) { onResetTarget(target) }
                }
                Spacer(Modifier.height(8.dp))
            }
        }
    }
}

// ─── Subtitle helpers ─────────────────────────────────────────────────────────

private fun rowSubtitle(page: SettingsPage, s: SettingsUiState): String = when (page) {
    SettingsPage.Appearance -> "AMOLED ${if (s.amoledMode) "on" else "off"} · compact ${if (s.compactSetLogging) "on" else "off"}"
    SettingsPage.Format -> "${if (s.useKg) "kg" else "lb"} · ${dateShort(s.dateFormat)} · ${if (s.timeFormat24h) "24h" else "12h"} · ${tzShort(s.timezone)}"
    SettingsPage.Session -> "Haptic: ${s.hapticStrength}"
    SettingsPage.Notifications -> if (s.quietHoursEnabled)
        "Quiet ${s.quietHoursStart.toString().padStart(2, '0')}:00–${s.quietHoursEnd.toString().padStart(2, '0')}:00"
    else "Off"
    SettingsPage.Tiles -> "${s.overviewTileOrder.count { it !in s.hiddenOverviewTiles }} of ${s.overviewTileOrder.size} visible"
    SettingsPage.Equipment -> if (s.availableEquipment.isEmpty()) "All equipment" else "${s.availableEquipment.size} selected"
    SettingsPage.Privacy -> if (s.privacyMode) "Screen blur on" else "Screen blur off"
}

private fun dateShort(f: String) = when (f) {
    "dd/MM/yyyy" -> "05/01"
    "MM/dd/yyyy" -> "01/05"
    else -> "Jan 5"
}

private fun tzShort(id: String) = when (id) {
    "America/Los_Angeles" -> "PST"
    "America/Denver" -> "MST"
    "America/Chicago" -> "CST"
    "America/New_York" -> "EST"
    "America/Sao_Paulo" -> "BRT"
    "UTC" -> "UTC"
    "Europe/London" -> "GMT"
    "Europe/Paris" -> "CET"
    "Europe/Moscow" -> "MSK"
    "Asia/Kolkata" -> "IST"
    "Asia/Tokyo" -> "JST"
    "Australia/Sydney" -> "AEST"
    else -> id.substringAfterLast("/")
}

// ─── Data export dialog ───────────────────────────────────────────────────────

private data class ExportOption(val label: String, val format: String, val action: () -> Unit)

@Composable
private fun DataExportDialog(viewModel: SettingsViewModel, onDismiss: () -> Unit) {
    var selectedIdx by remember { mutableStateOf<Int?>(null) }
    var selectedFormat by remember { mutableStateOf<String?>(null) }

    val options = remember(viewModel) {
        listOf(
            ExportOption("Sessions", "CSV") { viewModel.exportSessionsCsv() },
            ExportOption("Weekly summary", "JSON") { viewModel.exportWeeklyJson() },
            ExportOption("Full backup", "JSON") { viewModel.exportFullBackup() },
            ExportOption("Last session", "PDF") { viewModel.exportLastSessionPdf() }
        )
    }

    val validFormat = selectedIdx?.let { options[it].format }
    val canExport = selectedIdx != null && selectedFormat == validFormat

    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(bg, RoundedCornerShape(8.dp))
                .padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                "EXPORT DATA",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                letterSpacing = 1.5.sp
            )

            // What?
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("What?", style = MaterialTheme.typography.bodyMedium, color = onBg)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    options.forEachIndexed { idx, opt ->
                        PillChip("${opt.label}\n${opt.format}", selected = selectedIdx == idx) {
                            selectedIdx = idx
                            selectedFormat = opt.format
                        }
                    }
                }
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            // Format?
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Format?", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("CSV", "JSON", "PDF").forEach { fmt ->
                        val isValid = validFormat == fmt
                        PillChip(fmt, selected = selectedFormat == fmt, enabled = isValid) {
                            if (isValid) selectedFormat = fmt
                        }
                    }
                }
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "cancel",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onDismiss).padding(4.dp)
                )
                Text(
                    "Export →",
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (canExport) onBg else onBg.copy(alpha = 0.3f),
                    modifier = if (canExport) {
                        Modifier.clickable {
                            options[selectedIdx!!].action()
                            onDismiss()
                        }.padding(4.dp)
                    } else {
                        Modifier.padding(4.dp)
                    }
                )
            }
        }
    }
}

// ─── Reset confirm dialog ─────────────────────────────────────────────────────

@Composable
private fun ResetConfirmDialog(target: ResetTarget, onConfirm: () -> Unit, onDismiss: () -> Unit) {
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
