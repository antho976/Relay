package com.tally.app.ui.settings

import android.net.Uri
import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.ImportRepository
import com.tally.app.data.repo.ImportTarget
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import com.tally.core.AccountType
import com.tally.core.BankStatements
import com.tally.core.MoneyFormatter
import com.tally.core.Statement
import com.tally.core.StatementRead
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** Where the import stands. */
enum class ImportStage { CHOOSE, READING, PREVIEW, WRITING, DONE }

@Immutable
data class ImportState(
    val today: LocalDate,
    val stage: ImportStage = ImportStage.CHOOSE,
    /** The bank the page was opened for, whose directions lead; null when opened plain. */
    val source: BankSource? = null,
    val statement: Statement? = null,
    val plan: ImportPlan = ImportPlan(),
    val choices: ImportChoices = ImportChoices(),
    /** The open accounts the lines can go into. */
    val accounts: List<AccountBalance> = emptyList(),
    /** The account the lines go into; null makes a new one. */
    val targetId: Long? = null,
    val newName: String = "",
    val newType: AccountType = AccountType.CHEQUING,
    /** Why the last file could not be read, or the import did not finish. */
    val problem: String? = null,
    /** What the last import did, said on the done stage. */
    val done: String? = null,
) {
    val target: AccountBalance? get() = targetId?.let { id -> accounts.firstOrNull { it.id == id } }
    val targetName: String get() = target?.name ?: newName.trim().ifEmpty { "a new account" }
}

/**
 * The bank import: read a statement on the phone, plan every line against the account it goes
 * into (what is already there, what the owner's own entries say each payee is), and write the
 * lines the owner confirms, in one go. Nothing is kept of the file once the import is done.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class ImportViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val imports: ImportRepository,
    private val data: DataRepository,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val incoming: IncomingFiles,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()
    private val source: BankSource? = BankSource.of(savedStateHandle.get<String>(Args.SOURCE))

    private data class Picks(
        val choices: ImportChoices = ImportChoices(),
        val targetId: Long? = null,
        val newName: String = "",
        val newType: AccountType = AccountType.CHEQUING,
    )

    private data class Ui(
        val stage: ImportStage = ImportStage.CHOOSE,
        val problem: String? = null,
        val done: String? = null,
        val source: BankSource? = null,
    )

    private val statement = MutableStateFlow<Statement?>(null)
    private val picks = MutableStateFlow(Picks(newName = source?.accountName ?: BankSource.OTHER.accountName))
    private val ui = MutableStateFlow(Ui(source = source))
    private val learnedFlow = flow { emit(imports.learnedNotes()) }

    /** What the target account already holds over the statement's days: the duplicate check's other side. */
    private val existing = combine(statement, picks) { s, p -> Triple(s?.first, s?.last, p.targetId) }
        .distinctUntilChanged()
        .flatMapLatest { (first, last, target) ->
            if (first == null || last == null || target == null) flowOf(emptyList())
            else flow { emit(imports.existingEffects(target, first, last)) }
        }

    private data class Reads(val categories: List<CategoryEntity>, val accounts: List<AccountBalance>, val learned: Map<String, Long>)

    private val reads = combine(ledger.categories(), ledger.balances(), learnedFlow) { c, a, l ->
        Reads(c, a.filter { !it.archived }, learnedCategories(l, c))
    }

    val state: StateFlow<ImportState> = combine(statement, picks, ui, reads, existing) { s, p, u, r, e ->
        ImportState(
            today = today,
            stage = u.stage,
            source = u.source,
            statement = s,
            plan = if (s == null) ImportPlan() else planImport(s, p.choices, r.categories, r.learned, e),
            choices = p.choices,
            accounts = r.accounts,
            targetId = p.targetId,
            newName = p.newName,
            newType = p.newType,
            problem = u.problem,
            done = u.done,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ImportState(today = today, source = source))

    init {
        // A statement shared to Tally opens this page holding the file.
        incoming.take()?.let { read(it) }
    }

    /** The bank whose directions the next file is read under (and whose name a new account takes). */
    fun choose(bank: BankSource?) {
        ui.update { it.copy(source = bank, problem = null) }
        if (bank != null) picks.update { it.copy(newName = bank.accountName, newType = bank.accountType) }
    }

    /** Reads [uri]: a bank statement goes to the preview, Tally's own CSV is imported as it always was. */
    fun read(uri: Uri) {
        if (ui.value.stage == ImportStage.READING || ui.value.stage == ImportStage.WRITING) return
        ui.update { it.copy(stage = ImportStage.READING, problem = null, done = null) }
        viewModelScope.launch {
            val text = imports.readText(uri)
            if (text == null) {
                ui.update { it.copy(stage = ImportStage.CHOOSE, problem = "That file could not be opened.") }
                return@launch
            }
            val s = settings.current()
            val digits = MoneyFormatter(s.currency, Locale.getDefault()).fractionDigits
            val locale = Locale.getDefault()
            when (val read = BankStatements.read(text, digits, dayFirstFor(locale.language, locale.country))) {
                is StatementRead.Invalid -> ui.update { it.copy(stage = ImportStage.CHOOSE, problem = read.reason) }
                StatementRead.TallyCsv -> {
                    val result = data.importCsvText(text)
                    ui.update {
                        when (result) {
                            is DataResult.Done -> it.copy(stage = ImportStage.DONE, done = result.message)
                            is DataResult.Failed -> it.copy(stage = ImportStage.CHOOSE, problem = result.message)
                        }
                    }
                    if (result is DataResult.Done) notices.show(result.message)
                }
                is StatementRead.Ok -> open(read.statement)
            }
        }
    }

    /** A statement read: pick the account it most likely belongs to, and how its signs read. */
    private suspend fun open(found: Statement) {
        val accounts = ledger.balances().first().filter { !it.archived }
        val bank = ui.value.source
        val s = settings.current()
        val target = accounts.firstOrNull { belongsTo(it.name, bank) }
            ?: accounts.firstOrNull { it.id == s.defaultAccountId && bank == null }
        statement.value = found
        picks.update {
            it.copy(
                targetId = target?.id,
                choices = ImportChoices(
                    fileAccount = found.accounts.takeIf { a -> a.size > 1 }?.first(),
                    flip = guessFlip(found, target?.type ?: it.newType),
                ),
            )
        }
        ui.update { it.copy(stage = ImportStage.PREVIEW) }
    }

    fun setTarget(id: Long?) = picks.update { p ->
        val type = id?.let { i -> state.value.accounts.firstOrNull { it.id == i }?.type } ?: p.newType
        val s = statement.value
        p.copy(
            targetId = id,
            choices = p.choices.copy(
                flip = if (s != null) guessFlip(s, type) else p.choices.flip,
                counterpartId = p.choices.counterpartId?.takeIf { it != id },
            ),
        )
    }

    fun setNewName(name: String) = picks.update { it.copy(newName = name) }

    fun setNewType(type: AccountType) = picks.update { p ->
        val s = statement.value
        p.copy(newType = type, choices = p.choices.copy(flip = if (s != null && p.targetId == null) guessFlip(s, type) else p.choices.flip))
    }

    fun setFlip(flip: Boolean) = picks.update { it.copy(choices = it.choices.copy(flip = flip)) }

    fun setFileAccount(account: String?) = picks.update { it.copy(choices = it.choices.copy(fileAccount = account)) }

    fun setTransfers(mode: TransferMode) = picks.update { p ->
        val counterpart = p.choices.counterpartId ?: state.value.accounts.firstOrNull { it.id != p.targetId }?.id
        p.copy(choices = p.choices.copy(transfers = mode, counterpartId = if (mode == TransferMode.TRANSFERS) counterpart else p.choices.counterpartId))
    }

    fun setCounterpart(id: Long) = picks.update { it.copy(choices = it.choices.copy(counterpartId = id)) }

    /** Back to the bank list, the file let go. */
    fun again() {
        statement.value = null
        ui.update { it.copy(stage = ImportStage.CHOOSE, problem = null, done = null) }
    }

    /** Writes the planned lines. One transaction: a failure adds nothing. */
    fun confirm() {
        val st = state.value
        if (st.stage != ImportStage.PREVIEW || st.plan.count == 0) return
        val p = picks.value
        val target = if (p.targetId != null) {
            ImportTarget.Existing(p.targetId)
        } else {
            ImportTarget.New(
                name = p.newName.trim().ifEmpty { BankSource.OTHER.accountName },
                type = p.newType,
                // A new account opens where the statement says it stood before its first line.
                openingBalance = st.plan.openingFromStatement ?: 0L,
            )
        }
        val lines = st.plan.toWrite
        val into = st.targetName
        val duplicates = st.plan.duplicates
        ui.update { it.copy(stage = ImportStage.WRITING, problem = null) }
        viewModelScope.launch {
            val id = imports.write(target, lines)
            if (id == null) {
                ui.update { it.copy(stage = ImportStage.PREVIEW, problem = "The import did not finish. Nothing was added.") }
            } else {
                val line = importedLine(lines.size, into, duplicates)
                statement.value = null
                picks.update { it.copy(targetId = id) }
                ui.update { it.copy(stage = ImportStage.DONE, done = line) }
                notices.show(line)
            }
        }
    }
}
