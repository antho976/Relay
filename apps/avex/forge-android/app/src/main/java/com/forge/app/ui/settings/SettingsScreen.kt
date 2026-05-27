package com.forge.app.ui.settings

import androidx.activity.compose.BackHandler
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
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

internal data class SettingsRow(val label: String, val tags: String, val page: SettingsPage)

internal val ALL_ROWS = listOf(
    SettingsRow("Appearance", "amoled dark theme accent compact logging display", SettingsPage.Appearance),
    SettingsRow("Units & format", "kg lb weight date time week timezone locale", SettingsPage.Format),
    SettingsRow("Session", "encouragement haptic feedback vibration", SettingsPage.Session),
    SettingsRow("Notifications", "quiet hours notify suppress", SettingsPage.Notifications),
    SettingsRow("Overview tiles", "tiles order visible hidden gym cardio trophies streak deload", SettingsPage.Tiles),
    SettingsRow("Equipment", "equipment available barbell dumbbell cable machine", SettingsPage.Equipment),
    SettingsRow("Privacy", "privacy mode blur screenshot", SettingsPage.Privacy)
)

internal data class SettingsItem(val name: String, val tags: String, val page: SettingsPage)

internal val ALL_ITEMS = listOf(
    SettingsItem("AMOLED mode", "amoled black dark theme display", SettingsPage.Appearance),
    SettingsItem("Compact set logging", "compact logging display density", SettingsPage.Appearance),
    SettingsItem("Accent color", "color accent theme tint", SettingsPage.Appearance),
    SettingsItem("Weight unit", "kg lb weight unit pounds kilograms", SettingsPage.Format),
    SettingsItem("Date format", "date format dd mm yyyy", SettingsPage.Format),
    SettingsItem("Time format", "time 12h 24h clock hour", SettingsPage.Format),
    SettingsItem("First day of week", "week start monday sunday", SettingsPage.Format),
    SettingsItem("Timezone", "timezone locale region", SettingsPage.Format),
    SettingsItem("Show encouragement", "encouragement motivation messages", SettingsPage.Session),
    SettingsItem("Haptic feedback", "haptic vibration strength", SettingsPage.Session),
    SettingsItem("Note templates", "notes templates prompts form energy pain focus", SettingsPage.Session),
    SettingsItem("Quiet hours", "quiet hours suppress notifications silent", SettingsPage.Notifications),
    SettingsItem("Notifications", "notifications enable disable notify", SettingsPage.Notifications),
    SettingsItem("Tile order", "tile order drag reorder overview arrange", SettingsPage.Tiles),
    SettingsItem("Tile visibility", "hide show visible tiles gym cardio trophies streak deload", SettingsPage.Tiles),
    SettingsItem("Available equipment", "equipment barbell dumbbell cable machine body weight", SettingsPage.Equipment),
    SettingsItem("Privacy mode", "privacy mode blur screenshot screen", SettingsPage.Privacy),
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

    val displayRows = ALL_ROWS

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

