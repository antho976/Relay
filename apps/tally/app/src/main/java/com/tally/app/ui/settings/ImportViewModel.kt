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
import com.tally.app.data.repo.ImportPreviewData
import com.tally.app.data.repo.ImportRepository
import com.tally.app.data.repo.ImportTarget
import com.tally.app.data.repo.InvestRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.invest.FileAccount
import com.tally.app.ui.invest.INVEST_SOURCE
import com.tally.app.ui.invest.InvestImport
import com.tally.app.ui.invest.InvestPreview
import com.tally.app.ui.invest.KIND_ACTIVITIES
import com.tally.app.ui.invest.KIND_HOLDINGS
import com.tally.app.ui.invest.KIND_STATEMENT
import com.tally.app.ui.invest.guessMapping
import com.tally.app.ui.invest.investImportedLine
import com.tally.app.ui.nav.Args
import com.tally.core.AccountType
import com.tally.core.BankStatements
import com.tally.core.MoneyFormatter
import com.tally.core.Statement
import com.tally.core.StatementRead
import com.tally.core.WsKind
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
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
    /** A Wealthsimple investment file under way: its preview and where its accounts go, instead of [statement]. */
    val invest: InvestImport? = null,
    /** Opened for investments: Wealthsimple's investment files lead the page. */
    val investFirst: Boolean = false,
) {
    val target: AccountBalance? get() = targetId?.let { id -> accounts.firstOrNull { it.id == id } }
    val targetName: String get() = target?.name ?: newName.trim().ifEmpty { "a new account" }
}

/**
 * The bank import: read a statement on the phone, plan every line against the account it goes
 * into (what is already there, what the owner's own entries say each payee is), and write the
 * lines the owner confirms, in one go. Nothing is kept of the file once the import is done.
 *
 * A Wealthsimple investment file (a holdings report, an activities export, a TFSA's or an RRSP's
 * monthly statement) never goes through that plan: buys, sells and dividends are not spending and
 * income. The investment import previews it, the owner says where each of its accounts goes, and
 * it writes holdings and activities instead.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class ImportViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val imports: ImportRepository,
    private val invest: InvestRepository,
    private val data: DataRepository,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val incoming: IncomingFiles,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()
    private val source: BankSource? = BankSource.of(savedStateHandle.get<String>(Args.SOURCE))
    private val investFirst: Boolean = savedStateHandle.get<String>(Args.SOURCE) == INVEST_SOURCE

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
        val invest: InvestImport? = null,
    )

    private val statement = MutableStateFlow<Statement?>(null)
    private val picks = MutableStateFlow(Picks(newName = source?.accountName ?: BankSource.OTHER.accountName))
    private val ui = MutableStateFlow(Ui(source = source))
    private val learnedFlow = flow { emit(imports.learnedNotes()) }

    /** The investment file's text, held only until it is written or let go. */
    private var investText: String? = null

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
            invest = u.invest,
            investFirst = investFirst,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ImportState(today = today, source = source, investFirst = investFirst))

    init {
        // A statement shared to Tally opens this page holding the file.
        incoming.take()?.let { read(it) }
    }

    /** The bank whose directions the next file is read under (and whose name a new account takes). */
    fun choose(bank: BankSource?) {
        ui.update { it.copy(source = bank, problem = null) }
        if (bank != null) picks.update { it.copy(newName = bank.accountName, newType = bank.accountType) }
    }

    /**
     * Reads [uri]: a bank statement goes to the preview, a Wealthsimple investment file to the
     * investment preview, and Tally's own CSV is imported as it always was.
     */
    fun read(uri: Uri) {
        if (ui.value.stage == ImportStage.READING || ui.value.stage == ImportStage.WRITING) return
        investText = null
        ui.update { it.copy(stage = ImportStage.READING, problem = null, done = null, invest = null) }
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
                StatementRead.Investments -> openInvestments(text)
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

    /**
     * A Wealthsimple investment file, read by the investment import. Each account it names goes
     * where its number already lives, or to the investment account named for its kind, or to a
     * new one; a statement, which names none, goes to the one investment account when there is one.
     */
    private suspend fun openInvestments(text: String) {
        val found = try {
            invest.preview(text)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // The investment import words its refusals to be shown as they are.
            ui.update { it.copy(stage = ImportStage.CHOOSE, problem = e.message ?: "That file could not be read.") }
            return
        }
        val preview = investPreview(found)
        val held = ledger.balances().first().filter { !it.archived && it.type == AccountType.INVESTMENT }
        investText = text
        statement.value = null
        ui.update {
            it.copy(
                stage = ImportStage.PREVIEW,
                invest = InvestImport(
                    preview = preview,
                    mapping = guessMapping(preview.accounts, held),
                    statementAccountId = if (preview.kind == KIND_STATEMENT) held.singleOrNull()?.id else null,
                ),
            )
        }
    }

    /** Where the file's account [number] goes: an investment account's id, or null for a new one. */
    fun mapAccount(number: String, id: Long?) = ui.update { u ->
        u.copy(invest = u.invest?.let { it.copy(mapping = it.mapping + (number to id)) })
    }

    /** The investment account a monthly statement belongs to. */
    fun setStatementAccount(id: Long) = ui.update { u -> u.copy(invest = u.invest?.copy(statementAccountId = id)) }

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
        investText = null
        ui.update { it.copy(stage = ImportStage.CHOOSE, problem = null, done = null, invest = null) }
    }

    /** Writes the planned lines. One transaction: a failure adds nothing. */
    fun confirm() {
        ui.value.invest?.let { pending ->
            confirmInvest(pending)
            return
        }
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

    /** Writes the investment file as previewed, its accounts where the owner put them. */
    private fun confirmInvest(pending: InvestImport) {
        val text = investText ?: return
        if (ui.value.stage != ImportStage.PREVIEW || !pending.ready || pending.preview.count == 0) return
        ui.update { it.copy(stage = ImportStage.WRITING, problem = null) }
        viewModelScope.launch {
            val counts = try {
                invest.import(text, pending.mapping, pending.statementAccountId)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                ui.update { it.copy(stage = ImportStage.PREVIEW, problem = e.message ?: "The import did not finish.") }
                return@launch
            }
            val line = investImportedLine(counts.holdings, counts.activities, counts.accountsCreated, counts.duplicates)
            investText = null
            ui.update { it.copy(stage = ImportStage.DONE, done = line) }
            notices.show(line)
        }
    }
}

/** The data layer's preview in the screen's own terms, so the screen and its tests need no database. */
private fun investPreview(p: ImportPreviewData): InvestPreview = InvestPreview(
    kind = when (p.kind) {
        WsKind.HOLDINGS -> KIND_HOLDINGS
        WsKind.ACTIVITIES -> KIND_ACTIVITIES
        WsKind.STATEMENT -> KIND_STATEMENT
    },
    asOf = p.asOf,
    accounts = p.accounts.map { a -> FileAccount(a.number, a.name, a.registration, a.accountId, a.rows) },
    holdings = p.holdings,
    activities = p.activities,
    new = p.new,
    duplicates = p.duplicates,
    skipped = p.skipped,
)
