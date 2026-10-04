package com.tally.app.ui.settings

import android.net.Uri
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.core.BackupFile
import com.tally.core.BackupReadResult
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

/** The four groups of the Data page; each shows the outcome of its own last act under it. */
enum class DataGroup { BACKUP, SHEETS, SAMPLE, ERASE }

/** What is running. One at a time: every row goes passive until it is done. */
enum class DataOp(val group: DataGroup, val working: String) {
    SAVE_BACKUP(DataGroup.BACKUP, "Saving the backup"),
    READ_BACKUP(DataGroup.BACKUP, "Reading the backup"),
    RESTORE(DataGroup.BACKUP, "Restoring"),
    EXPORT_CSV(DataGroup.SHEETS, "Writing the CSV"),
    IMPORT_CSV(DataGroup.SHEETS, "Importing"),
    SAMPLE(DataGroup.SAMPLE, "Loading the sample"),
    ERASE(DataGroup.ERASE, "Erasing"),
}

@Immutable
data class DataState(
    val today: LocalDate,
    val entries: Int = 0,
    val accounts: Int = 0,
    val categories: Int = 0,
    val budgets: Int = 0,
    val bills: Int = 0,
    val goals: Int = 0,
    /** The newest entry, for the "last entry" reading; null in an empty ledger. */
    val newest: TransactionRow? = null,
    val sampleLoaded: Boolean = false,
    val busy: DataOp? = null,
    /** The last outcome per group, drawn as that group's footer. */
    val results: Map<DataGroup, DataResult> = emptyMap(),
    /** A backup that was read and checked, waiting for the owner to confirm the replace. */
    val pendingRestore: RestorePreview? = null,
    val loaded: Boolean = false,
) {
    val isEmpty: Boolean get() = entries == 0
}

/**
 * Backup, export, import, sample and erase. The file pickers live in the route (they need the
 * Activity); this holds only the Uris they hand back, never a Context.
 */
@HiltViewModel
class DataViewModel @Inject constructor(
    private val data: DataRepository,
    ledger: LedgerRepository,
    plan: PlanRepository,
    settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private data class Ui(
        val busy: DataOp? = null,
        val results: Map<DataGroup, DataResult> = emptyMap(),
        val pending: RestorePreview? = null,
    )

    private val ui = MutableStateFlow(Ui())

    /** The checked backup behind [DataState.pendingRestore]; kept out of the UI state, it can be large. */
    private var pendingFile: BackupFile? = null

    private val erasedChannel = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after an erase finishes; the route restarts into onboarding on it. */
    val erased: Flow<Unit> = erasedChannel.receiveAsFlow()

    private data class Counts(val entries: Int, val accounts: Int, val categories: Int, val newest: TransactionRow?)
    private data class PlanCounts(val budgets: Int, val bills: Int, val goals: Int)

    private val counts = combine(ledger.count(), ledger.accounts(), ledger.categories(), ledger.recent(1)) { n, accounts, categories, recent ->
        Counts(n, accounts.count { !it.archived }, categories.count { !it.archived }, recent.firstOrNull())
    }

    private val planCounts = combine(plan.budgets(), plan.recurring(), plan.goals()) { b, r, g ->
        PlanCounts(b.size, r.size, g.count { !it.goal.archived })
    }

    val state: StateFlow<DataState> = combine(counts, planCounts, settings.settings, ui) { c, p, s, u ->
        DataState(
            today = today,
            entries = c.entries,
            accounts = c.accounts,
            categories = c.categories,
            budgets = p.budgets,
            bills = p.bills,
            goals = p.goals,
            newest = c.newest,
            sampleLoaded = s.sampleLoaded,
            busy = u.busy,
            results = u.results,
            pendingRestore = u.pending,
            loaded = true,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), DataState(today = today))

    /** Starts [op] unless something else is running; [block] returns the outcome for the op's group. */
    private fun launchOp(op: DataOp, block: suspend () -> DataResult?) {
        if (ui.value.busy != null) return
        ui.update { it.copy(busy = op, results = it.results - op.group) }
        viewModelScope.launch {
            val result = runCatching { block() }.getOrElse { DataResult.Failed("That did not finish. Nothing was changed.") }
            ui.update { u -> u.copy(busy = null, results = if (result != null) u.results + (op.group to result) else u.results) }
            if (result is DataResult.Done) notices.show(result.message)
        }
    }

    fun saveBackup(uri: Uri) = launchOp(DataOp.SAVE_BACKUP) { data.writeBackup(uri) }

    /** Reads and checks the file; a good one waits for the confirm, a bad one says why under the group. */
    fun readBackup(uri: Uri) = launchOp(DataOp.READ_BACKUP) {
        when (val read = data.readBackup(uri)) {
            is BackupReadResult.Invalid -> DataResult.Failed(read.reason)
            is BackupReadResult.Ok -> {
                pendingFile = read.file
                ui.update { it.copy(pending = restorePreview(read.file)) }
                null
            }
        }
    }

    fun confirmRestore() {
        val file = pendingFile ?: return
        pendingFile = null
        ui.update { it.copy(pending = null) }
        launchOp(DataOp.RESTORE) { data.restore(file) }
    }

    fun cancelRestore() {
        pendingFile = null
        ui.update { it.copy(pending = null) }
    }

    fun exportCsv(uri: Uri) = launchOp(DataOp.EXPORT_CSV) { data.writeCsv(uri) }

    fun importCsv(uri: Uri) = launchOp(DataOp.IMPORT_CSV) { data.importCsv(uri) }

    fun loadSample() = launchOp(DataOp.SAMPLE) { data.loadSample() }

    /** Erase is the one act that asks first; the screen's dialog has already asked. */
    fun eraseAll() {
        if (ui.value.busy != null) return
        ui.update { it.copy(busy = DataOp.ERASE, results = it.results - DataGroup.ERASE) }
        viewModelScope.launch {
            val result = runCatching { data.eraseAll() }.getOrElse { DataResult.Failed("Erase did not finish.") }
            if (result is DataResult.Done) {
                notices.show(result.message)
                erasedChannel.send(Unit)
            } else {
                ui.update { u -> u.copy(busy = null, results = u.results + (DataGroup.ERASE to result)) }
            }
        }
    }

    /** No app on the phone can pick a file (rare, but a stripped-down phone has none). */
    fun noPicker(group: DataGroup) {
        ui.update { it.copy(results = it.results + (group to DataResult.Failed("No file picker is available on this phone."))) }
    }
}
