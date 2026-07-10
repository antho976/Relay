package com.forge.app.ui.settings

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.graphics.Color
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle

enum class SettingsPage(val title: String) {
    Appearance("Appearance"),
    Format("Units & format"),
    Session("Session"),
    Notifications("Notifications"),
    Program("Program & equipment"),
    Coach("Coach"),
    Recovery("Recovery"),
    ExercisePrefs("Exercise likes"),
    Vacation("Holiday / Vacation"),
    About("About")
}

internal data class SettingsItem(val name: String, val tags: String, val page: SettingsPage)

/** What a non-page search hit does when tapped — fire an action (dialog/menu), not open a sub-page. */
enum class SearchAction { DATA, IMPORT, RESET, COACH }

/** A search hit that triggers an [action] instead of navigating to a page. [where] is its breadcrumb. */
internal data class SettingsActionEntry(val name: String, val where: String, val tags: String, val action: SearchAction)

/** Search-reachable actions: the export/backup dialog, the reset menu, and the coach brief. */
internal val ACTION_ENTRIES = listOf(
    SettingsActionEntry("Export data", "Data", "data export csv pdf weekly json sessions full", SearchAction.DATA),
    SettingsActionEntry("Import data", "Data", "import data strong hevy fitnotes jefit csv json migrate from another app move history", SearchAction.IMPORT),
    SettingsActionEntry("Backup & restore", "Data", "backup restore database file save load import survive uninstall", SearchAction.DATA),
    SettingsActionEntry("Reset…", "Reset", "reset delete clear wipe erase sessions trophies cardio settings data", SearchAction.RESET),
    SettingsActionEntry("Factory reset", "Reset", "factory reset erase everything wipe delete all clean slate", SearchAction.RESET),
    SettingsActionEntry("Your coach", "Coach", "coach brief tracking plan learning weekly review autopilot deload trust history record proposals undo", SearchAction.COACH),
)

/**
 * Page-level search entries — so a query like "app" surfaces the whole Appearance page, not only its
 * individual toggles. A page's own NAME is otherwise absent from the item index (its items are
 * "AMOLED mode", "Accent color", …), which is why "app"/"prog"/"recov" used to return nothing.
 */
internal data class SettingsPageEntry(val page: SettingsPage, val tags: String)

internal val PAGE_ENTRIES = listOf(
    SettingsPageEntry(SettingsPage.Appearance, "appearance amoled dark theme accent color compact logging display privacy look app icon launcher home screen"),
    SettingsPageEntry(SettingsPage.Format, "units format kg lb weight date time week timezone locale distance strength standards sex"),
    SettingsPageEntry(SettingsPage.Session, "session haptic feedback vibration notes templates rest timer between sets compound isolation"),
    SettingsPageEntry(SettingsPage.Notifications, "notifications reminders quiet hours recap timer alerts notify suppress"),
    SettingsPageEntry(SettingsPage.Program, "program equipment generate auto split days routine rotate trainings workouts barbell dumbbell cable machine plate focus goal experience emphasis priority"),
    SettingsPageEntry(SettingsPage.Coach, "coach weekly review autopilot auto-apply suggest mode on off tweaks"),
    SettingsPageEntry(SettingsPage.Recovery, "recovery health connect sleep heart rate resting samsung watch coach deload steps bodyweight"),
    SettingsPageEntry(SettingsPage.ExercisePrefs, "exercise likes dislike favourite exclude preferences movements heart hidden preferred"),
    SettingsPageEntry(SettingsPage.Vacation, "holiday vacation pause streak break away travel"),
)

internal val ALL_ITEMS = listOf(
    SettingsItem("AMOLED mode", "amoled black dark theme display", SettingsPage.Appearance),
    SettingsItem("Compact set logging", "compact logging display density", SettingsPage.Appearance),
    SettingsItem("Accent color", "color accent theme tint", SettingsPage.Appearance),
    SettingsItem("App icon", "app icon launcher home screen change alternate", SettingsPage.Appearance),
    SettingsItem("Weight unit", "kg lb weight unit pounds kilograms", SettingsPage.Format),
    SettingsItem("Distance unit", "km mi miles kilometers distance cardio pace", SettingsPage.Format),
    SettingsItem("Date format", "date format dd mm yyyy", SettingsPage.Format),
    SettingsItem("Time format", "time 12h 24h clock hour", SettingsPage.Format),
    SettingsItem("First day of week", "week start monday sunday", SettingsPage.Format),
    SettingsItem("Timezone", "timezone locale region", SettingsPage.Format),
    SettingsItem("Haptic feedback", "haptic vibration strength", SettingsPage.Session),
    SettingsItem("Rest times", "rest timer seconds between sets compound isolation default", SettingsPage.Session),
    SettingsItem("Note templates", "notes templates prompts form energy pain focus", SettingsPage.Session),
    SettingsItem("Training reminders", "reminder notify nudge daily streak train schedule engagement", SettingsPage.Notifications),
    SettingsItem("Quiet hours", "quiet hours suppress notifications silent", SettingsPage.Notifications),
    SettingsItem("Available equipment", "equipment barbell dumbbell cable machine body weight", SettingsPage.Program),
    SettingsItem("Weight per plate", "plate weight machine plates count", SettingsPage.Program),
    SettingsItem("Heaviest dumbbell", "dumbbell max heaviest adjustable ceiling", SettingsPage.Program),
    SettingsItem("Privacy mode", "privacy mode blur screenshot screen", SettingsPage.Appearance),
    SettingsItem("Strength standards", "strength standards sex male female relative bodyweight ratio elite novice", SettingsPage.Format),
    SettingsItem("Weekly recap", "weekly recap summary notification report", SettingsPage.Notifications),
    SettingsItem("Rest timer alerts", "rest timer alert notification background buzz vibrate", SettingsPage.Notifications),
    SettingsItem("Health Connect", "health connect recovery sync samsung google fit permissions wearable watch", SettingsPage.Recovery),
    SettingsItem("Sleep", "sleep recovery hours health connect rest coach deload", SettingsPage.Recovery),
    SettingsItem("Resting heart rate", "resting heart rate hr recovery health connect coach", SettingsPage.Recovery),
    SettingsItem("Steps", "steps recovery health connect cardio wearable daily", SettingsPage.Recovery),
    SettingsItem("Bodyweight sync", "bodyweight weight sync health connect log recovery", SettingsPage.Recovery),
    SettingsItem("Exercise preferences", "exercise likes dislikes preferred hidden movements favourite heart", SettingsPage.ExercisePrefs),
    SettingsItem("Ask to dislike after swap", "swap dislike prompt hide exercise default", SettingsPage.ExercisePrefs),
    SettingsItem("Holiday mode", "holiday vacation pause streak break away travel", SettingsPage.Vacation),
    SettingsItem("Coach mode", "coach mode suggest auto apply autopilot earn", SettingsPage.Coach),
) + com.forge.app.program.Equipment.entries.map { equip ->
    // Each piece of equipment is searchable by name, so a query like "kettlebell" surfaces it
    // (tagged to the Program & equipment page) instead of only the generic "Available equipment" row.
    SettingsItem(equip.display, "equipment ${equip.name.lowercase()} ${equip.display.lowercase()}", SettingsPage.Program)
}

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
    onOpenCoachBrief: () -> Unit = {},
    onOpenBuilder: () -> Unit = {},
    // When set, open straight to this sub-page (deep link, e.g. the cardio "connect a watch" banner →
    // Recovery) instead of the root list.
    initialPage: SettingsPage? = null,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val exportPath by viewModel.exportPath.collectAsStateWithLifecycle()
    val photoCount by viewModel.photoCount.collectAsStateWithLifecycle()

    // Persisted across nav (rememberSaveable) so returning from a deep screen launched here — the
    // program Builder, the Coach brief — lands back on the page you opened it from, not the root list.
    // Seeded with [initialPage] for a deep link; rememberSaveable keeps a later in-screen back/forward
    // navigation sticky rather than snapping back to the deep-linked page.
    var currentPage by rememberSaveable { mutableStateOf<SettingsPage?>(initialPage) }
    // Deep-linked arrival (the cardio watch banner → Recovery): the user never saw the root list, so
    // the FIRST back returns to the caller (the cardio tab), not "up" to a Settings root they didn't
    // come from. The flag clears the moment they navigate anywhere else inside Settings.
    var onDeepLinkedPage by rememberSaveable { mutableStateOf(initialPage != null) }
    LaunchedEffect(currentPage) { if (currentPage != initialPage) onDeepLinkedPage = false }
    // The Program page's open sub-section is hoisted here so EVERY back affordance (top-bar arrow,
    // system back) returns to the Program menu first, then to Settings — never skipping a level.
    var programSection by rememberSaveable { mutableStateOf<ProgramSection?>(null) }
    var searchQuery by remember { mutableStateOf("") }
    var confirmReset by remember { mutableStateOf<ResetTarget?>(null) }
    var showResetMenu by remember { mutableStateOf(false) }
    var showDataDialog by remember { mutableStateOf(false) }
    var showImportDialog by remember { mutableStateOf(false) }

    // Complete DB backup & restore via the system file picker (survives uninstall).
    val context = LocalContext.current
    val statusMessage by viewModel.statusMessage.collectAsStateWithLifecycle()
    val restoreSucceeded by viewModel.restoreSucceeded.collectAsStateWithLifecycle()
    val restoreImpact by viewModel.restoreImpact.collectAsStateWithLifecycle()
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
    // Import from another gym app (#GYMAP-17) — same SAF pattern as restore, but merges instead of replaces.
    val importResult by viewModel.importResult.collectAsStateWithLifecycle()
    // Manual file pick — opens the picker already pointed at Downloads (where exports land) and
    // filtered to CSV/JSON, so the right file is usually one tap away.
    val importLauncher = rememberLauncherForActivityResult(
        OpenImportDocument()
    ) { uri -> uri?.let { viewModel.importData(it) } }
    // CSV/JSON from other apps; some file pickers only tag them octet-stream, so accept broadly.
    val launchImport: () -> Unit = {
        importLauncher.launch(arrayOf("text/csv", "text/comma-separated-values", "application/json", "text/plain", "application/octet-stream", "*/*"))
    }
    // One-time folder grant (Downloads) so Import can auto-scan it for exports.
    val folderGrantLauncher = rememberLauncherForActivityResult(
        OpenDownloadsTree()
    ) { uri -> uri?.let { viewModel.grantImportFolder(it) } }
    LaunchedEffect(restoreSucceeded) { if (restoreSucceeded) restartApp(context) }

    BackHandler(enabled = currentPage != null || searchQuery.isNotBlank()) {
        when {
            currentPage == SettingsPage.Program && programSection != null -> programSection = null
            onDeepLinkedPage && currentPage != null -> onBack()
            currentPage != null -> { currentPage = null; programSection = null }
            else -> searchQuery = ""
        }
    }

    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    Scaffold(
        topBar = {
            TopAppBar(
                title = { com.forge.app.ui.common.ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = {
                        when {
                            currentPage == SettingsPage.Program && programSection != null -> programSection = null
                            onDeepLinkedPage && currentPage != null -> onBack()
                            currentPage != null -> { currentPage = null; programSection = null }
                            searchQuery.isNotBlank() -> searchQuery = ""
                            else -> onBack()
                        }
                    }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        AnimatedContent(
            targetState = currentPage,
            transitionSpec = {
                // Opening a sub-page slides in from the right; backing out slides from the left.
                val dir = if (targetState != null) 1 else -1
                (slideInHorizontally { dir * it / 4 } + fadeIn()) togetherWith
                    (slideOutHorizontally { -dir * it / 4 } + fadeOut())
            },
            label = "settings-page"
        ) { page ->
            when (page) {
                null -> MainList(
                    state = state,
                    searchQuery = searchQuery,
                    modifier = Modifier.fillMaxSize().padding(inner),
                    onSearchChange = { searchQuery = it },
                    onOpenPage = { currentPage = it; programSection = null },
                    onOpenCoachBrief = onOpenCoachBrief,
                    onOpenDataDialog = { showDataDialog = true },
                    onImportData = { showImportDialog = true },
                    onResetTarget = { confirmReset = it },
                    onOpenResetMenu = { showResetMenu = true }
                )
                SettingsPage.Appearance -> AppearancePage(state, viewModel, Modifier.padding(inner))
                SettingsPage.Format -> FormatPage(state, viewModel, Modifier.padding(inner))
                SettingsPage.Session -> SessionPage(state, viewModel, Modifier.padding(inner))
                SettingsPage.Notifications -> NotificationsPage(state, viewModel, Modifier.padding(inner))
                SettingsPage.Program -> ProgramPage(
                    state = state,
                    vm = viewModel,
                    section = programSection,
                    onSectionChange = { programSection = it },
                    modifier = Modifier.padding(inner),
                    onOpenBuilder = onOpenBuilder
                )
                SettingsPage.Coach -> CoachSettingsPage(
                    state, viewModel,
                    onOpenRecovery = { currentPage = SettingsPage.Recovery },
                    modifier = Modifier.padding(inner)
                )
                SettingsPage.Recovery -> RecoveryPage(Modifier.padding(inner))
                SettingsPage.ExercisePrefs -> ExercisePrefsPage(state, viewModel, Modifier.padding(inner))
                SettingsPage.Vacation -> VacationPage(viewModel, Modifier.padding(inner))
                SettingsPage.About -> AboutPage(Modifier.padding(inner), viewModel)
            }
        }
    }

    if (showResetMenu) {
        ResetMenuDialog(
            onPick = { showResetMenu = false; confirmReset = it },
            onDismiss = { showResetMenu = false }
        )
    }

    if (showDataDialog) {
        DataExportDialog(
            viewModel = viewModel,
            onBackup = { backupLauncher.launch("forge_backup_$dateStamp.zip") },
            onRestore = { restoreLauncher.launch(arrayOf("*/*")) },
            onExportCrashLogs = { crashLauncher.launch("forge_crash_logs_$dateStamp.zip") },
            onDismiss = { showDataDialog = false }
        )
    }

    if (showImportDialog) {
        ImportDialog(
            viewModel = viewModel,
            onGrantFolder = { folderGrantLauncher.launch(null) },
            onManualPick = launchImport,
            onImportFound = { viewModel.importData(it) },
            onDismiss = { showImportDialog = false }
        )
    }

    importResult?.let { msg ->
        AlertDialog(
            onDismissRequest = viewModel::clearImportResult,
            title = { Text("Import") },
            text = { Text(msg) },
            confirmButton = { TextButton(onClick = viewModel::clearImportResult) { Text("OK") } }
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
            text = {
                val impact = restoreImpact
                Text(
                    buildString {
                        append("This replaces ALL current data")
                        if (!impact.isNullOrBlank()) append(", your $impact,")
                        append(" with the chosen backup, then restarts the app. It can't be undone; back up first if you're unsure.")
                    }
                )
            },
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
        // Refresh photo count the moment the factory-reset dialog opens so the warning is accurate.
        androidx.compose.runtime.LaunchedEffect(target) {
            if (target == ResetTarget.FACTORY) viewModel.refreshPhotoInfo()
        }
        val photoWarning = if (target == ResetTarget.FACTORY && photoCount > 0) {
            "This also permanently deletes your ${photoCountLabel(photoCount)}."
        } else null
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
            onDismiss = { confirmReset = null },
            photoWarning = photoWarning
        )
    }
}

/** URI that hints the document picker to open in the Downloads folder — where app exports land. Its
 *  exact resolution varies by OEM; if a device doesn't honour it the picker just opens at its default. */
private val DOWNLOADS_TREE_URI: android.net.Uri =
    android.net.Uri.parse("content://com.android.externalstorage.documents/document/primary:Download")

/** [ActivityResultContracts.OpenDocument] that starts in Downloads (via [DocumentsContract.EXTRA_INITIAL_URI]). */
private class OpenImportDocument : ActivityResultContracts.OpenDocument() {
    override fun createIntent(context: android.content.Context, input: Array<String>): android.content.Intent =
        super.createIntent(context, input).apply {
            putExtra(android.provider.DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_TREE_URI)
        }
}

/** [ActivityResultContracts.OpenDocumentTree] that starts at Downloads, for the one-time folder grant. */
private class OpenDownloadsTree : ActivityResultContracts.OpenDocumentTree() {
    override fun createIntent(context: android.content.Context, input: android.net.Uri?): android.content.Intent =
        super.createIntent(context, input ?: DOWNLOADS_TREE_URI).apply {
            putExtra(android.provider.DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_TREE_URI)
        }
}

/** Relaunch the app — used after a restore, since the database file was swapped underneath Room. */
private fun restartApp(context: android.content.Context) {
    val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)
    intent?.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK)
    if (intent != null) context.startActivity(intent)
    Runtime.getRuntime().exit(0)
}

