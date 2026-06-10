package com.forge.app.ui.settings

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.ui.platform.LocalContext
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
    Equipment("Equipment"),
    Privacy("Privacy"),
    Program("Program"),
    ExercisePrefs("Exercise likes"),
    Vacation("Holiday / Vacation")
}

internal data class SettingsRow(val label: String, val tags: String, val page: SettingsPage)

internal val ALL_ROWS = listOf(
    SettingsRow("Appearance", "amoled dark theme accent compact logging display", SettingsPage.Appearance),
    SettingsRow("Units & format", "kg lb weight date time week timezone locale", SettingsPage.Format),
    SettingsRow("Session", "haptic feedback vibration notes templates", SettingsPage.Session),
    SettingsRow("Notifications", "quiet hours notify suppress", SettingsPage.Notifications),
    SettingsRow("Equipment", "equipment available barbell dumbbell cable machine", SettingsPage.Equipment),
    SettingsRow("Privacy", "privacy mode blur screenshot", SettingsPage.Privacy),
    SettingsRow("Program", "program generate auto split days routine rotate trainings workouts", SettingsPage.Program),
    SettingsRow("Exercise likes", "like dislike favourite exclude exercises preferences movements heart", SettingsPage.ExercisePrefs)
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
    SettingsItem("Haptic feedback", "haptic vibration strength", SettingsPage.Session),
    SettingsItem("Note templates", "notes templates prompts form energy pain focus", SettingsPage.Session),
    SettingsItem("Quiet hours", "quiet hours suppress notifications silent", SettingsPage.Notifications),
    SettingsItem("Notifications", "notifications enable disable notify", SettingsPage.Notifications),
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

    // Complete DB backup & restore via the system file picker (survives uninstall).
    val context = LocalContext.current
    val statusMessage by viewModel.statusMessage.collectAsStateWithLifecycle()
    val restoreSucceeded by viewModel.restoreSucceeded.collectAsStateWithLifecycle()
    var pendingRestoreUri by remember { mutableStateOf<Uri?>(null) }
    val dateStamp = remember {
        java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.getDefault()).format(java.util.Date())
    }
    val backupLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/octet-stream")
    ) { uri -> uri?.let { viewModel.backupDatabase(it) } }
    val restoreLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri -> uri?.let { pendingRestoreUri = it } }
    val crashLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/zip")
    ) { uri -> uri?.let { viewModel.exportCrashLogs(it) } }
    LaunchedEffect(restoreSucceeded) { if (restoreSucceeded) restartApp(context) }

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
            SettingsPage.Equipment -> EquipmentPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Privacy -> PrivacyPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Program -> ProgramPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.ExercisePrefs -> ExercisePrefsPage(state, viewModel, Modifier.padding(inner))
            SettingsPage.Vacation -> VacationPage(viewModel, Modifier.padding(inner))
        }
    }

    if (showDataDialog) {
        DataExportDialog(
            viewModel = viewModel,
            onBackup = { backupLauncher.launch("forge_backup_$dateStamp.db") },
            onRestore = { restoreLauncher.launch(arrayOf("*/*")) },
            onExportCrashLogs = { crashLauncher.launch("forge_crash_logs_$dateStamp.zip") },
            onDismiss = { showDataDialog = false }
        )
    }

    statusMessage?.let { msg ->
        AlertDialog(
            onDismissRequest = viewModel::clearStatusMessage,
            title = { Text("Backup & restore") },
            text = { Text(msg) },
            confirmButton = { TextButton(onClick = viewModel::clearStatusMessage) { Text("OK") } }
        )
    }

    pendingRestoreUri?.let { uri ->
        AlertDialog(
            onDismissRequest = { pendingRestoreUri = null },
            title = { Text("Restore from backup?") },
            text = { Text("This replaces ALL current data with the chosen backup, then restarts the app. It can't be undone — back up first if you're unsure.") },
            confirmButton = {
                TextButton(onClick = { viewModel.restoreDatabase(uri); pendingRestoreUri = null }) {
                    Text("Restore & restart", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = { TextButton(onClick = { pendingRestoreUri = null }) { Text("Cancel") } }
        )
    }

    // When an export finishes, open the system share sheet so it can be saved as a real file
    // (Save to Files / Downloads / Drive…) instead of being stranded in app storage.
    exportPath?.let { path ->
        LaunchedEffect(path) {
            val file = java.io.File(path)
            runCatching {
                val uri = androidx.core.content.FileProvider.getUriForFile(
                    context, "${context.packageName}.fileprovider", file
                )
                val mime = when (file.extension.lowercase()) {
                    "json" -> "application/json"
                    "csv" -> "text/csv"
                    "pdf" -> "application/pdf"
                    else -> "*/*"
                }
                val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                    type = mime
                    putExtra(android.content.Intent.EXTRA_STREAM, uri)
                    addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                context.startActivity(android.content.Intent.createChooser(send, "Save or share export"))
            }
            viewModel.clearExportPath()
        }
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

/** Relaunch the app — used after a restore, since the database file was swapped underneath Room. */
private fun restartApp(context: android.content.Context) {
    val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)
    intent?.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK)
    if (intent != null) context.startActivity(intent)
    Runtime.getRuntime().exit(0)
}

