package com.tally.app.ui.accounts

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.AccountUse
import com.tally.app.data.repo.InvestRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.core.AccountType
import com.tally.core.MoneyFormatter
import com.tally.core.Registration
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
import java.util.Locale
import javax.inject.Inject
import kotlin.math.abs

/** Everything the account editor draws. */
@Immutable
data class AccountEditState(
    val id: Long = 0,
    val draft: AccountDraft = AccountDraft(),
    val loaded: Boolean = false,
    /** True when the account exists in the database (editing, not new). */
    val stored: Boolean = false,
    val storedName: String = "",
    /** The account is archived as saved, so the delete dialog has no archive to offer. */
    val storedArchived: Boolean = false,
    /** The balance as it will read with the draft's opening balance. */
    val balance: Long = 0,
    val entryCount: Int = 0,
    val share: SharePreview = SharePreview(),
    val problems: AccountProblems = AccountProblems(),
    /** True once a save was tried with problems: the form then says what is missing. */
    val showErrors: Boolean = false,
    /** What goes with the account, counted afresh, while the delete dialog is open; null when it is closed. */
    val confirmDelete: AccountUse? = null,
    /** The values recorded for an investment account, newest first. */
    val values: List<AccountValueEntity> = emptyList(),
    /** Ready-made accounts not yet taken, for a new account. */
    val templates: List<AccountTemplate> = emptyList(),
    val today: LocalDate = LocalDate.now(),
) {
    val isNew: Boolean get() = !stored

    /** Archiving is the way to keep the entries and bills, offered when there are some and it is not archived yet. */
    val canArchive: Boolean get() = stored && !storedArchived && confirmDelete?.isEmpty == false
}

@HiltViewModel
class AccountEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val ledger: LedgerRepository,
    private val invest: InvestRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    private val clock: Clock,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L

    private data class Flags(val loaded: Boolean = false, val showErrors: Boolean = false, val confirmDelete: AccountUse? = null)

    private val draft = MutableStateFlow(AccountDraft())
    private val stored = MutableStateFlow<AccountEntity?>(null)
    private val flags = MutableStateFlow(Flags())
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or delete; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    val state: StateFlow<AccountEditState> = combine(draft, stored, ledger.balances(), flags, ledger.values(argId)) { d, st, balances, f, values ->
        val row = st?.let { s -> balances.firstOrNull { it.id == s.id } }
        val opening = d.signedOpening ?: st?.openingBalance ?: 0L
        // The stored balance already holds the stored opening; swap in the draft's. One that starts
        // from a recorded value holds no opening at all, so it reads as stored.
        val balance = when {
            st != null && row != null && row.valuedOn != null -> row.balance
            st != null && row != null -> row.balance - st.openingBalance + opening
            else -> opening
        }
        AccountEditState(
            id = st?.id ?: 0L,
            draft = d,
            loaded = f.loaded,
            stored = st != null,
            storedName = st?.name.orEmpty(),
            storedArchived = st?.archived ?: false,
            balance = balance,
            entryCount = row?.entryCount ?: 0,
            share = previewShare(st?.id ?: 0L, balance, d.archived, balances),
            problems = accountProblems(d),
            showErrors = f.showErrors,
            confirmDelete = f.confirmDelete,
            values = values,
            templates = if (st == null) freeTemplates(balances.map { it.name }) else emptyList(),
            today = clock.today(),
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AccountEditState())

    init {
        viewModelScope.launch {
            // A draft kept before the process died wins over the stored row: it is what was typed.
            val kept = savedStateHandle.savedDraft(AccountDraft.serializer())
            val s = settings.current()
            val account = if (argId != 0L) ledger.account(argId) else null
            if (account != null) {
                val money = MoneyFormatter(s.currency, Locale.getDefault())
                draft.value = kept ?: AccountDraft(
                    name = account.name,
                    type = account.type,
                    openingText = if (account.openingBalance == 0L) "" else money.formatInput(account.openingBalance),
                    opening = abs(account.openingBalance),
                    owe = account.openingBalance < 0,
                    isDefault = s.defaultAccountId == account.id,
                    archived = account.archived,
                    registration = account.registration,
                )
                stored.value = account
            } else {
                // The first account becomes the default, so the entry screen has somewhere to start.
                draft.value = kept ?: AccountDraft(isDefault = s.defaultAccountId == 0L)
            }
            flags.update { it.copy(loaded = true) }
            draft.collect { savedStateHandle.keepDraft(AccountDraft.serializer(), it) }
        }
    }

    fun setName(name: String) = draft.update { it.copy(name = name) }

    fun setType(type: AccountType) = draft.update { it.copy(type = type) }

    /** An investment account's kind; the room on Investments counts the TFSA, RRSP and FHSA ones. */
    fun setRegistration(registration: Registration?) = draft.update { it.copy(registration = registration) }

    /** A ready-made account: its name, type and kind, and a card starts as owed. */
    fun applyTemplate(t: AccountTemplate) = draft.update {
        it.copy(name = t.name, type = t.type, owe = t.type == AccountType.CREDIT, registration = t.registration)
    }

    /** Records what an investment account is worth today; a second value today replaces the first. */
    fun recordValue(value: Long) {
        if (argId == 0L) return
        viewModelScope.launch {
            ledger.setValue(argId, clock.today(), value)
            notices.show("Value recorded")
        }
    }

    /** Removes one recorded value at once and offers Undo. */
    fun deleteValue(id: Long) {
        val repo = ledger
        viewModelScope.launch {
            val row = repo.deleteValue(id) ?: return@launch
            notices.showUndo("Value removed") { repo.restoreValue(row) }
        }
    }

    /** [parsed] is [text] read by the screen's money formatter; null when it is not an amount. */
    fun setOpening(text: String, parsed: Long?) = draft.update { it.copy(openingText = text, opening = readOpening(text, parsed)) }

    fun setOwe(owe: Boolean) = draft.update { it.copy(owe = owe) }

    fun setDefault(isDefault: Boolean) = draft.update { it.copy(isDefault = isDefault) }

    fun setArchived(archived: Boolean) = draft.update { it.copy(archived = archived) }

    fun save() {
        if (busy) return
        val s = state.value
        if (!s.loaded) return
        if (s.problems.any) {
            flags.update { it.copy(showErrors = true) }
            return
        }
        val d = s.draft
        val opening = d.signedOpening ?: return
        val original = stored.value
        busy = true
        viewModelScope.launch {
            val id = ledger.saveAccount(
                AccountEntity(
                    id = original?.id ?: 0L,
                    name = d.name.trim(),
                    type = d.type,
                    openingBalance = opening,
                    archived = d.archived,
                    sortOrder = original?.sortOrder ?: 0,
                )
            )
            // The kind and the institution are the investment ledger's to set; a save keeps the stored ones.
            if (d.type == AccountType.INVESTMENT) {
                val institution = institutionFor(d.name, original?.institution.orEmpty())
                if (d.registration != original?.registration || institution != original?.institution) {
                    invest.setRegistration(id, d.registration, institution)
                }
            }
            val currentDefault = settings.current().defaultAccountId
            if (d.effectiveDefault) {
                if (currentDefault != id) settings.setDefaultAccount(id)
            } else if (currentDefault == id) {
                settings.setDefaultAccount(0L)
            }
            finished.send(Unit)
        }
    }

    /** Counts afresh what goes with the account (entries, bills, other accounts' transfers) and opens the dialog that names it. */
    fun requestDelete() {
        val original = stored.value ?: return
        if (busy) return
        viewModelScope.launch {
            val use = ledger.accountUse(original.id)
            flags.update { it.copy(confirmDelete = use) }
        }
    }

    fun dismissDelete() = flags.update { it.copy(confirmDelete = null) }

    /**
     * Irreversible: the account goes, and through the foreign keys every entry that touches it and
     * every bill that pays from or into it.
     */
    fun delete() {
        val original = stored.value ?: return
        if (busy) return
        busy = true
        val use = flags.value.confirmDelete ?: AccountUse()
        flags.update { it.copy(confirmDelete = null) }
        viewModelScope.launch {
            ledger.deleteAccount(original.id)
            if (settings.current().defaultAccountId == original.id) settings.setDefaultAccount(0L)
            notices.show(accountDeletedLine(original.name, use))
            finished.send(Unit)
        }
    }

    /**
     * The dialog's way to keep everything: the account as saved, archived, so its entries and
     * bills stay and it leaves the pickers. Undo puts it back as it was, default included.
     */
    fun archiveInstead() {
        val original = stored.value ?: return
        if (busy || original.archived) return
        busy = true
        flags.update { it.copy(confirmDelete = null) }
        val repo = ledger
        val prefs = settings
        viewModelScope.launch {
            repo.saveAccount(original.copy(archived = true))
            val wasDefault = prefs.current().defaultAccountId == original.id
            if (wasDefault) prefs.setDefaultAccount(0L)
            // The undo holds the repositories and the row, never this ViewModel.
            notices.showUndo("${original.name} archived · its entries and bills stay") {
                repo.saveAccount(original)
                if (wasDefault) prefs.setDefaultAccount(original.id)
            }
            finished.send(Unit)
        }
    }
}
