package com.tally.app.ui.settings

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.automirrored.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.backup.BackupCopy
import com.tally.app.data.backup.BackupStore
import com.tally.app.data.prefs.BackupPrefs
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.RowPill
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.SwitchRow
import com.tally.app.ui.nav.AppNav
import com.tally.core.BackupFile
import com.tally.core.BackupReadResult
import com.tally.core.Copy
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import javax.inject.Inject

/** What is running on the Backup page. One at a time. */
enum class BackupOp(val working: String) {
    BACK_UP("Backing up"),
    SAVE_FILE("Saving the file"),
    READ("Reading the backup"),
    RESTORE("Restoring"),
}

@Immutable
data class BackupState(
    val today: LocalDate,
    val prefs: BackupPrefs = BackupPrefs(),
    /** The copies kept on the phone, newest first. */
    val copies: List<BackupCopy> = emptyList(),
    val entries: Int = 0,
    val busy: BackupOp? = null,
    /** The last act's outcome, the page's one line of news. */
    val result: DataResult? = null,
    /** A backup read and checked, waiting for the owner to confirm the replace. */
    val pendingRestore: RestorePreview? = null,
    val loaded: Boolean = false,
)

/**
 * Backup, Avex's page: the weekly copy and how its last run went, a folder that keeps a copy past
 * an uninstall, back up now, and the way back from a kept copy or any backup file. Restoring is
 * the one act here that asks first.
 */
@HiltViewModel
class BackupViewModel @Inject constructor(
    private val store: BackupStore,
    private val data: DataRepository,
    ledger: LedgerRepository,
    settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private data class Ui(val busy: BackupOp? = null, val result: DataResult? = null, val pending: RestorePreview? = null)

    private val ui = MutableStateFlow(Ui())

    /** The checked backup behind [BackupState.pendingRestore]; kept out of the UI state, it can be large. */
    private var pendingFile: BackupFile? = null

    val state: StateFlow<BackupState> = combine(settings.backup, store.copies, ledger.count(), ui) { p, copies, n, u ->
        BackupState(
            today = today,
            prefs = p,
            copies = copies,
            entries = n,
            busy = u.busy,
            result = u.result,
            pendingRestore = u.pending,
            loaded = true,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), BackupState(today = today))

    init {
        viewModelScope.launch { store.refresh() }
    }

    private fun launchOp(op: BackupOp, block: suspend () -> DataResult?) {
        if (ui.value.busy != null) return
        ui.update { it.copy(busy = op, result = null) }
        viewModelScope.launch {
            val result = runCatching { block() }.getOrElse { DataResult.Failed("That did not finish. Nothing was changed.") }
            ui.update { it.copy(busy = null, result = result) }
            if (result is DataResult.Done) notices.show(result.message)
        }
    }

    fun setAuto(on: Boolean) {
        viewModelScope.launch { store.setAuto(on) }
    }

    fun setFolder(uri: Uri) {
        viewModelScope.launch {
            if (!store.adoptFolder(uri)) {
                ui.update { it.copy(result = DataResult.Failed("That folder cannot be kept. Choose another one.")) }
            }
        }
    }

    fun clearFolder() {
        viewModelScope.launch { store.forgetFolder() }
    }

    fun backupNow() = launchOp(BackupOp.BACK_UP) { store.backupNow() }

    fun saveFile(uri: Uri) = launchOp(BackupOp.SAVE_FILE) { data.writeBackup(uri) }

    fun readFile(uri: Uri) = launchOp(BackupOp.READ) { hold(data.readBackup(uri)) }

    fun readCopy(copy: BackupCopy) = launchOp(BackupOp.READ) { hold(store.read(copy)) }

    /** A good file waits for the confirm; a bad one says why. */
    private fun hold(read: BackupReadResult): DataResult? = when (read) {
        is BackupReadResult.Invalid -> DataResult.Failed(read.reason)
        is BackupReadResult.Ok -> {
            pendingFile = read.file
            ui.update { it.copy(pending = restorePreview(read.file)) }
            null
        }
    }

    fun confirmRestore() {
        val file = pendingFile ?: return
        pendingFile = null
        ui.update { it.copy(pending = null) }
        launchOp(BackupOp.RESTORE) { data.restore(file) }
    }

    fun cancelRestore() {
        pendingFile = null
        ui.update { it.copy(pending = null) }
    }

    fun noPicker() {
        ui.update { it.copy(result = DataResult.Failed("No file picker is available on this phone.")) }
    }
}

@Composable
fun BackupRoute(nav: AppNav) {
    val viewModel: BackupViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    // The Storage Access Framework: the owner picks every place, so the app needs no storage permission.
    val folderPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri != null) viewModel.setFolder(uri)
    }
    val fileSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        if (uri != null) viewModel.saveFile(uri)
    }
    val fileOpener = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.readFile(uri)
    }
    fun pick(launch: () -> Unit) {
        runCatching(launch).onFailure { viewModel.noPicker() }
    }
    BackupScreen(
        state,
        BackupActions(
            back = nav::back,
            setAuto = viewModel::setAuto,
            chooseFolder = { pick { folderPicker.launch(null) } },
            clearFolder = viewModel::clearFolder,
            backupNow = viewModel::backupNow,
            saveFile = { pick { fileSaver.launch(backupFileName(state.today)) } },
            openFile = { pick { fileOpener.launch(arrayOf("application/json", "*/*")) } },
            restoreCopy = viewModel::readCopy,
            confirmRestore = viewModel::confirmRestore,
            cancelRestore = viewModel::cancelRestore,
        ),
    )
}

/** Everything the Backup page can do, as plain lambdas; the pickers stay in the route. */
data class BackupActions(
    val back: () -> Unit = {},
    val setAuto: (Boolean) -> Unit = {},
    val chooseFolder: () -> Unit = {},
    val clearFolder: () -> Unit = {},
    val backupNow: () -> Unit = {},
    val saveFile: () -> Unit = {},
    val openFile: () -> Unit = {},
    val restoreCopy: (BackupCopy) -> Unit = {},
    val confirmRestore: () -> Unit = {},
    val cancelRestore: () -> Unit = {},
)

private val STAMP_SHOWN: DateTimeFormatter get() = DateTimeFormatter.ofPattern("HH:mm", Locale.getDefault())

/** "3 Oct, 14:20", "Today, 09:05": when a backup was written, in words. */
internal fun backupWhen(millis: Long, today: LocalDate): String {
    val time = LocalDateTime.ofInstant(Instant.ofEpochMilli(millis), ZoneId.systemDefault())
    return Dates.day(time.toLocalDate(), today) + ", " + time.format(STAMP_SHOWN)
}

/** "1.2 MB", "84 KB": a file's size, for a kept copy's row. */
internal fun sizeLabel(bytes: Long): String = when {
    bytes >= 1_000_000L -> String.format(Locale.getDefault(), "%.1f MB", bytes / 1_000_000.0)
    bytes >= 1_000L -> "${bytes / 1_000L} KB"
    else -> "$bytes bytes"
}

/** A short, human folder name from a tree Uri ("…/tree/primary:Download/Tally" reads "Tally"). */
internal fun folderLabel(uri: String): String =
    Uri.decode(uri).substringAfterLast(':').substringAfterLast('/').ifBlank { "Chosen folder" }

/**
 * The Backup page: the weekly switch over its last run, the folder, the two acts (back up now, save
 * a file), then the copies kept on this phone with the way to restore any of them, or a file. The
 * last act's outcome is the footer of the group it belongs to.
 */
@Composable
fun BackupScreen(state: BackupState, actions: BackupActions) {
    val idle = state.busy == null
    val p = state.prefs
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        item(key = "head") {
            PageHead("Backup", onBack = actions.back, context = "Copies of everything, kept on this phone")
        }
        item(key = "auto") {
            val switch: @Composable (Shape) -> Unit = { shape ->
                SwitchRow(
                    "Weekly backup",
                    p.auto,
                    actions.setAuto,
                    shape,
                    subtitle = "A restorable copy of everything, written every week",
                )
            }
            val status: @Composable (Shape) -> Unit = { shape ->
                val flag = p.lastFailed || (p.lastAt == 0L && state.entries > 0)
                GroupRow(
                    when {
                        p.lastFailed -> "Last backup failed"
                        p.lastAt > 0L -> "Last backup"
                        else -> "No backup yet"
                    },
                    shape,
                    subtitle = when {
                        p.lastFailed -> "Back up now to try again"
                        p.lastAt > 0L -> backupWhen(p.lastAt, state.today) + if (p.folderFailed) " · the folder copy failed" else ""
                        state.entries > 0 -> "Your entries are not protected by a copy yet"
                        else -> "The first one is written once there is something to keep"
                    },
                    leading = {
                        val tint = if (flag) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground
                        GlyphBadge(if (flag) Icons.Rounded.ErrorOutline else Icons.Rounded.History, tint = tint, fill = if (flag) tint.copy(alpha = 0.14f) else MaterialTheme.colorScheme.surfaceContainerHighest)
                    },
                )
            }
            Group(rows = listOf(switch, status), modifier = Modifier.padding(horizontal = GUTTER), title = "Automatic")
        }
        item(key = "folder") {
            val folder = p.folderUri
            val pickRow: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    folder?.let(::folderLabel) ?: "No folder chosen",
                    shape,
                    subtitle = if (folder != null) "Each backup also lands here" else "Backups stay inside the app only",
                    leading = { GlyphBadge(Icons.Rounded.Folder) },
                    trailing = { RowPill(if (folder == null) "Choose" else "Change") },
                    chevron = false,
                    onClick = if (idle) actions.chooseFolder else null,
                )
            }
            val clearRow: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    "Stop copying to the folder",
                    shape,
                    subtitle = "What is already there stays",
                    leading = { GlyphBadge(Icons.Rounded.Close) },
                    chevron = false,
                    onClick = if (idle) actions.clearFolder else null,
                )
            }
            Group(
                rows = if (folder != null) listOf(pickRow, clearRow) else listOf(pickRow),
                modifier = Modifier.padding(horizontal = GUTTER),
                title = "Folder",
                footer = "A folder keeps your backups even if you uninstall Tally. Copies kept inside the app do not survive that.",
            )
        }
        item(key = "acts") {
            Column(Modifier.padding(horizontal = GUTTER), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                HeroAction(if (state.busy == BackupOp.BACK_UP) "Backing up" else "Back up now", actions.backupNow, Modifier.fillMaxWidth(), enabled = idle)
                SecondaryAction("Save a copy as a file", actions.saveFile, Modifier.fillMaxWidth(), enabled = idle)
            }
        }
        item(key = "restore") {
            val copies = state.copies.map { copy -> copyRow(copy, state.today, idle, actions.restoreCopy) }
            val fileRow: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    "Restore from a file",
                    shape,
                    subtitle = "A Tally backup saved anywhere on this phone",
                    leading = { GlyphBadge(Icons.AutoMirrored.Rounded.InsertDriveFile) },
                    onClick = if (idle) actions.openFile else null,
                )
            }
            val busy = state.busy
            val result = state.result
            Group(
                rows = copies + fileRow,
                modifier = Modifier.padding(horizontal = GUTTER),
                title = "Restore",
                trailing = if (state.copies.isEmpty()) null else Copy.plural(state.copies.size, "copy", "copies") + " kept",
                footer = busy?.working ?: when (result) {
                    is DataResult.Done -> result.message
                    is DataResult.Failed -> result.message
                    null -> "Restoring replaces everything on this phone with the backup."
                },
                footerIsError = busy == null && result is DataResult.Failed,
            )
        }
    }
    val pending = state.pendingRestore
    if (pending != null) {
        val savedOn = pending.savedOn?.let { Dates.short(it, state.today) }
        ConfirmDialog(
            text = restorePrompt(pending.entries, savedOn),
            confirm = "Replace",
            destructive = false,
            onConfirm = actions.confirmRestore,
            onDismiss = actions.cancelRestore,
        )
    }
}

private fun copyRow(copy: BackupCopy, today: LocalDate, enabled: Boolean, onRestore: (BackupCopy) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        backupWhen(copy.savedAt, today),
        shape,
        subtitle = "Kept on this phone · " + sizeLabel(copy.bytes),
        leading = { GlyphBadge(Icons.Rounded.Restore) },
        trailing = { RowPill("Restore") },
        chevron = false,
        onClick = if (enabled) ({ onRestore(copy) }) else null,
    )
}
