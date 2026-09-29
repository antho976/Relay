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
import androidx.compose.foundation.lazy.rememberLazyListState
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material3.Icon
import androidx.compose.ui.unit.dp
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.theme.ForgeMotion

enum class SettingsPage(val title: String) {
    Appearance("Appearance"),
    Format("Units & format"),
    Session("Session"),
    Notifications("Notifications"),
    Security("Privacy & security"),
    Program("Program & equipment"),
    Coach("Coach"),
    Recovery("Wearable"),
    ExercisePrefs("Exercise likes"),
    CardioActivities("Cardio activities"),
    Vacation("Holidays"),
    Backup("Backup"),
    Storage("Storage"),
    WhatsNew("What's new"),
    PrivacyPolicy("Privacy policy"),
    About("About")
}

internal data class SettingsItem(val name: String, val tags: String, val page: SettingsPage)

/** What a non-page search hit does when tapped — fire an action (dialog/menu), not open a sub-page. */
enum class SearchAction { DATA, IMPORT, RESET, FACTORY, COACH }

/** A search hit that triggers an [action] instead of navigating to a page. [where] is its breadcrumb. */
internal data class SettingsActionEntry(val name: String, val where: String, val tags: String, val action: SearchAction)

/** Search-reachable actions: the export/backup dialog, the reset menu, and the coach brief. */
internal val ACTION_ENTRIES = listOf(
    SettingsActionEntry("Export data", "Data", "data export csv pdf weekly json sessions full", SearchAction.DATA),
    SettingsActionEntry("Import data", "Data", "import data strong hevy fitnotes jefit csv json migrate from another app move history", SearchAction.IMPORT),
    SettingsActionEntry("Backup & restore", "Data", "backup restore database file save load import survive uninstall", SearchAction.DATA),
    SettingsActionEntry("Reset…", "Reset", "reset delete clear wipe erase sessions trophies cardio settings data", SearchAction.RESET),
    SettingsActionEntry("Factory reset", "Reset", "factory reset erase everything wipe delete all clean slate", SearchAction.FACTORY),
    SettingsActionEntry("Your coach", "Coach", "coach brief tracking plan learning weekly review autopilot deload trust history record proposals undo", SearchAction.COACH),
)

/**
 * Page-level search entries — so a query like "app" surfaces the whole Appearance page, not only its
 * individual toggles. A page's own NAME is otherwise absent from the item index (its items are
 * "AMOLED mode", "Accent color", …), which is why "app"/"prog"/"recov" used to return nothing.
 */
internal data class SettingsPageEntry(val page: SettingsPage, val tags: String)

internal val PAGE_ENTRIES = listOf(
    SettingsPageEntry(SettingsPage.Appearance, "appearance amoled dark theme accent color display look app icon launcher home screen startup animation"),
    SettingsPageEntry(SettingsPage.Format, "units format kg lb weight time clock 12h 24h week distance strength standards sex"),
    SettingsPageEntry(SettingsPage.Session, "session haptic feedback vibration notes templates rest timer between sets compound isolation"),
    SettingsPageEntry(SettingsPage.Notifications, "notifications reminders quiet hours recap timer alerts notify suppress daily check-in morning sleep soreness stress drive weight"),
    SettingsPageEntry(SettingsPage.Security, "security lock app lock gallery lock biometric fingerprint face pin passcode privacy mode screenshots recents photos protect unlock"),
    SettingsPageEntry(SettingsPage.Program, "program equipment generate auto split days routine rotate trainings workouts barbell dumbbell cable machine plate focus goal experience emphasis priority"),
    SettingsPageEntry(SettingsPage.Coach, "coach weekly review autopilot auto-apply suggest mode on off tweaks"),
    SettingsPageEntry(SettingsPage.Recovery, "recovery health connect sleep heart rate resting samsung galaxy pixel fitbit wearable watch ring coach deload steps bodyweight"),
    SettingsPageEntry(SettingsPage.ExercisePrefs, "exercise likes dislike favourite exclude preferences movements heart hidden preferred"),
    SettingsPageEntry(SettingsPage.CardioActivities, "cardio custom activity activities types padel kayak sport add own create running walking"),
    SettingsPageEntry(SettingsPage.Vacation, "holiday vacation pause streak break away travel"),
    SettingsPageEntry(SettingsPage.Backup, "backup auto backup automatic weekly restore folder uninstall survive save copy protect data safety back up now sd card cloud"),
    SettingsPageEntry(SettingsPage.Storage, "storage space disk size cache clear breakdown photos database usage used free clean up megabytes"),
    SettingsPageEntry(SettingsPage.WhatsNew, "what's new whats new changelog release notes updates version history latest build recent changes"),
)

internal val ALL_ITEMS = listOf(
    SettingsItem("AMOLED mode", "amoled black dark theme display", SettingsPage.Appearance),
    SettingsItem("Accent color", "color accent theme tint", SettingsPage.Appearance),
    SettingsItem("App icon", "app icon launcher home screen change alternate", SettingsPage.Appearance),
    SettingsItem("Weight unit", "kg lb weight unit pounds kilograms", SettingsPage.Format),
    SettingsItem("Distance unit", "km mi miles kilometers distance cardio pace", SettingsPage.Format),
    SettingsItem("Length unit", "cm in inches centimeters length body measurements waist chest", SettingsPage.Format),
    SettingsItem("Time format", "time 12h 24h clock hour", SettingsPage.Format),
    SettingsItem("First day of week", "week start monday sunday", SettingsPage.Format),
    SettingsItem("Haptic feedback", "haptic vibration strength", SettingsPage.Session),
    SettingsItem("Keep screen on", "keep screen on awake display lock timeout sleep session logging", SettingsPage.Session),
    SettingsItem("Rest times", "rest timer seconds between sets compound isolation default", SettingsPage.Session),
    SettingsItem("Note templates", "notes templates prompts form energy pain focus", SettingsPage.Session),
    SettingsItem("Training reminders", "reminder notify nudge daily streak train schedule engagement", SettingsPage.Notifications),
    SettingsItem("Quiet hours", "quiet hours suppress notifications silent", SettingsPage.Notifications),
    SettingsItem("Available equipment", "equipment barbell dumbbell cable machine body weight", SettingsPage.Program),
    SettingsItem("Weight per plate", "plate weight machine plates count", SettingsPage.Program),
    SettingsItem("Heaviest dumbbell", "dumbbell max heaviest adjustable ceiling", SettingsPage.Program),
    SettingsItem("Privacy mode", "privacy mode blur screenshot screen recents", SettingsPage.Security),
    SettingsItem("App lock", "app lock biometric fingerprint face pin passcode unlock protect open security", SettingsPage.Security),
    SettingsItem("Photo gallery lock", "gallery lock photos progress pictures biometric fingerprint pin protect security", SettingsPage.Security),
    SettingsItem("Auto-lock", "auto lock timeout re-lock immediately minutes grace security", SettingsPage.Security),
    SettingsItem("Strength standards", "strength standards sex male female relative bodyweight ratio elite novice", SettingsPage.Format),
    SettingsItem("Weekly recap", "weekly recap summary notification report", SettingsPage.Notifications),
    SettingsItem("Rest timer alerts", "rest timer alert notification background buzz vibrate", SettingsPage.Notifications),
    SettingsItem("Health Connect", "health connect recovery sync samsung google fit permissions wearable watch", SettingsPage.Recovery),
    SettingsItem("Wearable", "wearable watch galaxy pixel fitbit samsung ring band tracker", SettingsPage.Recovery),
    SettingsItem("Sleep", "sleep recovery hours health connect rest coach deload", SettingsPage.Recovery),
    SettingsItem("Resting heart rate", "resting heart rate hr recovery health connect coach", SettingsPage.Recovery),
    SettingsItem("Steps", "steps recovery health connect cardio wearable daily", SettingsPage.Recovery),
    SettingsItem("Bodyweight sync", "bodyweight weight sync health connect log recovery", SettingsPage.Recovery),
    SettingsItem("Exercise preferences", "exercise likes dislikes preferred hidden movements favourite heart", SettingsPage.ExercisePrefs),
    SettingsItem("Ask to hide after swapping", "swap dislike prompt hide exercise default", SettingsPage.ExercisePrefs),
    SettingsItem("Holiday mode", "holiday vacation pause streak break away travel", SettingsPage.Vacation),
    SettingsItem("Coach mode", "coach mode suggest auto apply autopilot earn", SettingsPage.Coach),
    SettingsItem("Daily check-ins", "morning sleep soreness stress drive weight readiness", SettingsPage.Notifications),
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
    // Names the copies it takes and the ones it cannot: "Deletes ALL data" kept the backup ZIP,
    // exports and crash logs (2026-09-26 audit, D2), and a folder the user picked is not the app's.
    FACTORY(
        "Factory reset",
        "Deletes all data stored in the app, including its backup copy, exports and crash logs, and resets all settings. Backups saved outside the app are kept. This cannot be undone."
    )
}

/** Where Settings is: a page, and inside Program one of its sections. Depth orders the slide. */
private data class SettingsDestination(val page: SettingsPage?, val section: ProgramSection?) {
    val depth: Int get() = when {
        page == null -> 0
        section != null -> 2
        else -> 1
    }
}

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
    val exportProgress by viewModel.exportProgress.collectAsStateWithLifecycle()
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
    // Root-list scroll, hoisted here (outside the AnimatedContent that swaps pages) so it survives
    // opening a sub-page and backing out — the list lands where you left it, not scrolled to the top.
    val mainListState = rememberLazyListState()
    // Saveable so a rotation mid-confirm (restore / factory reset) keeps the dialog and the picked backup.
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var confirmReset by rememberSaveable { mutableStateOf<ResetTarget?>(null) }
    var showResetMenu by rememberSaveable { mutableStateOf(false) }
    var showDataDialog by rememberSaveable { mutableStateOf(false) }
    var showImportDialog by rememberSaveable { mutableStateOf(false) }

    // Complete DB backup & restore via the system file picker (survives uninstall).
    val context = LocalContext.current
    val restoreImpact by viewModel.restoreImpact.collectAsStateWithLifecycle()
    var pendingRestoreUri by rememberSaveable { mutableStateOf<Uri?>(null) }
    // ISO date = ASCII digits whatever the locale (ar/fa would otherwise localise them), read at
    // launch time so a Settings screen left open past midnight doesn't suggest yesterday's name.
    fun dateStamp(): String = java.time.LocalDate.now().toString()
    val backupLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/octet-stream")
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        // The ZIP carries every progress photo, so a gallery lock guards this exactly as it guards
        // "Back up now". Asked AFTER the picker, because the picker round-trip itself re-locks under
        // "Immediately"; a refusal deletes the empty file the picker already created.
        if (state.galleryLockEnabled) {
            authenticateSettingsAction(
                context, viewModel, "Unlock photos for this backup",
                onDenied = { viewModel.discardBackupTarget(uri) }
            ) { viewModel.backupDatabase(uri) }
        } else {
            viewModel.backupDatabase(uri)
        }
    }
    val restoreLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri -> uri?.let { pendingRestoreUri = it } }
    val crashLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/zip")
    ) { uri -> uri?.let { viewModel.exportCrashLogs(it) } }
    // Import from another gym app (#GYMAP-17) — same SAF pattern as restore, but merges instead of replaces.
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

    // One back path for the top-bar arrow and the system gesture: a Program section returns to the
    // Program menu, a deep-linked page returns to whoever linked it, any other page to the root,
    // and the root clears its search before it leaves Settings.
    fun goBack() {
        when {
            currentPage == SettingsPage.Program && programSection != null -> programSection = null
            onDeepLinkedPage && currentPage != null -> onBack()
            currentPage != null -> { currentPage = null; programSection = null }
            searchQuery.isNotBlank() -> searchQuery = ""
            else -> onBack()
        }
    }
    BackHandler(enabled = currentPage != null || searchQuery.isNotBlank()) { goBack() }

    SettingsTheme {
        AnimatedContent(
            targetState = SettingsDestination(currentPage, programSection),
            transitionSpec = {
                // Material's shared X axis: deeper pages arrive from the end edge, backing out
                // returns from the start edge, and both halves fade so the titles never collide.
                val forward = targetState.depth > initialState.depth
                val shift = { full: Int -> (full * 0.1f).toInt() }
                (slideInHorizontally(ForgeMotion.enterTween(ForgeMotion.DurationEmphasized)) { if (forward) shift(it) else -shift(it) } +
                    fadeIn(ForgeMotion.enterTween(ForgeMotion.DurationStandard))) togetherWith
                    (slideOutHorizontally(ForgeMotion.exitTween(ForgeMotion.DurationEmphasized)) { if (forward) -shift(it) else shift(it) } +
                        fadeOut(ForgeMotion.exitTween(ForgeMotion.DurationFast)))
            },
            label = "settings-page"
        ) { dest ->
            val back: () -> Unit = { goBack() }
            when (dest.page) {
                null -> MainList(
                    state = state,
                    searchQuery = searchQuery,
                    listState = mainListState,
                    onBack = back,
                    onSearchChange = { searchQuery = it },
                    onOpenPage = { currentPage = it; programSection = null },
                    onOpenCoachBrief = onOpenCoachBrief,
                    onOpenDataDialog = { showDataDialog = true },
                    onImportData = { showImportDialog = true },
                    onResetTarget = { confirmReset = it },
                    onOpenResetMenu = { showResetMenu = true }
                )
                SettingsPage.Appearance -> AppearancePage(state, viewModel, back)
                SettingsPage.Format -> FormatPage(state, viewModel, back)
                SettingsPage.Session -> SessionPage(state, viewModel, back)
                SettingsPage.Notifications -> NotificationsPage(state, viewModel, back)
                SettingsPage.Security -> SecurityPage(state, viewModel, back)
                SettingsPage.Program -> ProgramPage(
                    state = state,
                    vm = viewModel,
                    section = dest.section,
                    onSectionChange = { programSection = it },
                    onBack = back,
                    onOpenBuilder = onOpenBuilder
                )
                SettingsPage.Coach -> CoachSettingsPage(
                    state, viewModel,
                    onOpenRecovery = { currentPage = SettingsPage.Recovery },
                    onBack = back
                )
                SettingsPage.Recovery -> RecoveryPage(back)
                SettingsPage.ExercisePrefs -> ExercisePrefsPage(state, viewModel, back)
                SettingsPage.CardioActivities -> CardioActivitiesPage(viewModel, back)
                SettingsPage.Vacation -> VacationPage(viewModel, back)
                SettingsPage.Backup -> BackupPage(viewModel, back)
                SettingsPage.Storage -> StoragePage(viewModel, back)
                SettingsPage.WhatsNew -> WhatsNewPage(back)
                SettingsPage.PrivacyPolicy -> PrivacyPolicyPage(back)
                SettingsPage.About -> AboutPage(
                    onBack = back,
                    viewModel = viewModel,
                    onOpenExport = { showDataDialog = true },
                    onOpenPrivacyPolicy = { currentPage = SettingsPage.PrivacyPolicy }
                )
            }
        }
    }

    // When an export finishes, open the system share sheet so it can be saved as a real file
    // (Save to Files / Downloads / Drive…) instead of being stranded in app storage.
    // Not while locked: a long export finishing in the background would otherwise open the sheet,
    // a separate Activity, above the lock with the whole data file attached. The path waits in
    // the StateFlow until unlock.
    if (!com.forge.app.security.LocalAppLockActive.current) exportPath?.let { path ->
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

    SettingsTheme { SettingsDialogs(
        viewModel = viewModel,
        showResetMenu = showResetMenu,
        onCloseResetMenu = { showResetMenu = false },
        onPickReset = { showResetMenu = false; confirmReset = it },
        showDataDialog = showDataDialog,
        onCloseData = { showDataDialog = false },
        onBackup = { backupLauncher.launch("avex_backup_${dateStamp()}.zip") },
        onRestore = { restoreLauncher.launch(arrayOf("*/*")) },
        onExportCrashLogs = { crashLauncher.launch("avex_crash_logs_${dateStamp()}.zip") },
        showImportDialog = showImportDialog,
        onCloseImport = { showImportDialog = false },
        onGrantFolder = { viewModel.openImportFolderPicker { start -> folderGrantLauncher.launch(start) } },
        onManualPick = launchImport,
        onRestoreFound = { pendingRestoreUri = it },
        pendingRestoreUri = pendingRestoreUri,
        onClearRestore = { pendingRestoreUri = null },
        restoreImpact = restoreImpact,
        exportProgress = exportProgress,
        confirmReset = confirmReset,
        onClearReset = { confirmReset = null },
        photoCount = photoCount
    ) }
}

/**
 * Everything Settings opens over its pages: the Data sheets, the reset flow and the restore and
 * export dialogs. Drawn in [SettingsTheme] so they speak the same voice as the pages.
 */
@Composable
private fun SettingsDialogs(
    viewModel: SettingsViewModel,
    showResetMenu: Boolean,
    onCloseResetMenu: () -> Unit,
    onPickReset: (ResetTarget) -> Unit,
    showDataDialog: Boolean,
    onCloseData: () -> Unit,
    onBackup: () -> Unit,
    onRestore: () -> Unit,
    onExportCrashLogs: () -> Unit,
    showImportDialog: Boolean,
    onCloseImport: () -> Unit,
    onGrantFolder: () -> Unit,
    onManualPick: () -> Unit,
    onRestoreFound: (Uri) -> Unit,
    pendingRestoreUri: Uri?,
    onClearRestore: () -> Unit,
    restoreImpact: String?,
    exportProgress: Pair<Int, Int>?,
    confirmReset: ResetTarget?,
    onClearReset: () -> Unit,
    photoCount: Int
) {
    if (showResetMenu) {
        ResetMenuDialog(onPick = onPickReset, onDismiss = onCloseResetMenu)
    }

    if (showDataDialog) {
        DataExportDialog(
            viewModel = viewModel,
            onBackup = onBackup,
            onRestore = onRestore,
            onExportCrashLogs = onExportCrashLogs,
            onDismiss = onCloseData
        )
    }

    if (showImportDialog) {
        ImportDialog(
            viewModel = viewModel,
            onGrantFolder = onGrantFolder,
            onManualPick = onManualPick,
            onImportFound = { viewModel.importData(it) },
            // A found backup goes through the same confirm as one picked by hand.
            onRestoreFound = onRestoreFound,
            onDismiss = onCloseImport
        )
    }

    // Import / backup / restore outcomes are transient lines on the app's ONE snackbar now (§12) —
    // they used to be two AlertDialogs that had to be dismissed to confirm something already done.

    // A restore that found a password-protected backup asks for the password here, at the root, so
    // it outlives the Data dialog the auto-backup restore was started from.
    RestorePasswordDialogHost(viewModel)

    pendingRestoreUri?.let { uri ->
        val impact = restoreImpact
        SettingsConfirmDialog(
            title = "Restore from this backup?",
            body = buildString {
                append("This replaces all current data")
                if (!impact.isNullOrBlank()) append(", your $impact,")
                append(" with the chosen backup, then restarts Avex. It can't be undone, so back up first if you're unsure.")
            },
            confirmLabel = "Restore and restart",
            icon = Icons.Rounded.Restore,
            onConfirm = { viewModel.restoreDatabase(uri); onClearRestore() },
            onDismiss = onClearRestore
        )
    }

    // A whole training history is minutes of work on an old phone, and the export used to report
    // nothing at all until the file appeared — so a user with two years of sessions could not tell
    // a slow export from a stuck one, and had no way to stop it (P-01). Cancelling is safe: the
    // export publishes by rename, so the previous file is still there afterwards.
    exportProgress?.let { (done, total) ->
        AlertDialog(
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            onDismissRequest = { },
            icon = { Icon(SettingsIcons.Export, contentDescription = null) },
            title = { Text("Exporting your data") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    Text(
                        when {
                            total <= 0 -> "Reading your history…"
                            total == 1 -> "One workout to write."
                            else -> "$done of $total workouts written."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    // Drawn, not a platform spinner: a share of a known total, so it reads as progress.
                    SettingsProgressBar(if (total > 0) done.toFloat() / total else 0f)
                }
            },
            confirmButton = {
                TextButton(onClick = { viewModel.cancelExport() }) { Text("Cancel", color = MaterialTheme.colorScheme.onSurface) }
            }
        )
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
                onClearReset()
            },
            onDismiss = onClearReset,
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

/**
 * [ActivityResultContracts.OpenDocumentTree] for the one-time folder grant. Starts at [input] when
 * given (`Download/Avex`, which Android 11+ lets an app use), else at Downloads, which it does not.
 */
private class OpenDownloadsTree : ActivityResultContracts.OpenDocumentTree() {
    override fun createIntent(context: android.content.Context, input: android.net.Uri?): android.content.Intent =
        super.createIntent(context, input ?: DOWNLOADS_TREE_URI).apply {
            putExtra(android.provider.DocumentsContract.EXTRA_INITIAL_URI, input ?: DOWNLOADS_TREE_URI)
        }
}

