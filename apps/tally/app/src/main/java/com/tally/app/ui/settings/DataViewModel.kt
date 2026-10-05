package com.tally.app.ui.settings

import android.net.Uri
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Notices
import com.tally.core.BudgetPeriod
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

/** The groups whose acts this holds; each shows the outcome of its own last act under it. */
enum class DataGroup { EXPORT, RESET }

/** What is running. One at a time: every row goes passive until it is done. */
enum class DataOp(val group: DataGroup, val working: String) {
    EXPORT_CSV(DataGroup.EXPORT, "Writing the CSV"),
    EXPORT_FILE(DataGroup.EXPORT, "Saving the backup file"),
    SAMPLE(DataGroup.RESET, "Loading the sample"),
    ERASE(DataGroup.RESET, "Erasing"),
}

@Immutable
data class DataState(
    val today: LocalDate,
    val period: BudgetPeriod = BudgetPeriod.containing(today),
    val entries: Int = 0,
    val sampleLoaded: Boolean = false,
    val busy: DataOp? = null,
    /** The last outcome per group, drawn as that group's footer. */
    val results: Map<DataGroup, DataResult> = emptyMap(),
    val loaded: Boolean = false,
) {
    val isEmpty: Boolean get() = entries == 0
}

/**
 * Export, the sample and erase: the acts that move the whole ledger out, in, or away. The file
 * pickers live in the routes (they need the Activity); this holds only the Uris they hand back.
 * Backup and restore are [BackupViewModel]'s, the bank import [ImportViewModel]'s.
 */
@HiltViewModel
class DataViewModel @Inject constructor(
    private val data: DataRepository,
    ledger: LedgerRepository,
    settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private data class Ui(val busy: DataOp? = null, val results: Map<DataGroup, DataResult> = emptyMap())

    private val ui = MutableStateFlow(Ui())

    private val erasedChannel = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after an erase finishes; the route restarts into onboarding on it. */
    val erased: Flow<Unit> = erasedChannel.receiveAsFlow()

    val state: StateFlow<DataState> = combine(ledger.count(), settings.settings, ui) { n, s, u ->
        DataState(
            today = today,
            period = s.periodFor(today),
            entries = n,
            sampleLoaded = s.sampleLoaded,
            busy = u.busy,
            results = u.results,
            loaded = true,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), DataState(today = today))

    /** Starts [op] unless something else is running; [block] returns the outcome for the op's group. */
    private fun launchOp(op: DataOp, block: suspend () -> DataResult) {
        if (ui.value.busy != null) return
        ui.update { it.copy(busy = op, results = it.results - op.group) }
        viewModelScope.launch {
            val result = runCatching { block() }.getOrElse { DataResult.Failed("That did not finish. Nothing was changed.") }
            ui.update { u -> u.copy(busy = null, results = u.results + (op.group to result)) }
            if (result is DataResult.Done) notices.show(result.message)
        }
    }

    /** Every entry, or [period]'s alone, as CSV. */
    fun exportCsv(uri: Uri, period: BudgetPeriod? = null) =
        launchOp(DataOp.EXPORT_CSV) { data.writeCsv(uri, period?.start, period?.endExclusive) }

    fun exportFile(uri: Uri) = launchOp(DataOp.EXPORT_FILE) { data.writeBackup(uri) }

    fun loadSample() = launchOp(DataOp.SAMPLE) { data.loadSample() }

    /** Erase is the one act that asks first; the screen's dialog has already asked. */
    fun eraseAll() {
        if (ui.value.busy != null) return
        ui.update { it.copy(busy = DataOp.ERASE, results = it.results - DataGroup.RESET) }
        viewModelScope.launch {
            val result = runCatching { data.eraseAll() }.getOrElse { DataResult.Failed("Erase did not finish.") }
            if (result is DataResult.Done) {
                notices.show(result.message)
                erasedChannel.send(Unit)
            } else {
                ui.update { u -> u.copy(busy = null, results = u.results + (DataGroup.RESET to result)) }
            }
        }
    }

    /** No app on the phone can pick a file (rare, but a stripped-down phone has none). */
    fun noPicker(group: DataGroup) {
        ui.update { it.copy(results = it.results + (group to DataResult.Failed("No file picker is available on this phone."))) }
    }
}
