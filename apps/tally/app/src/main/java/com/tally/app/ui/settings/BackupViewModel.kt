package com.tally.app.ui.settings

import android.net.Uri
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
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
import com.tally.app.ui.common.Notices
import com.tally.core.BackupFile
import com.tally.core.BackupReadResult
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
