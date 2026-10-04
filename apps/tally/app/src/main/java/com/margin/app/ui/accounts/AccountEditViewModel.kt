package com.margin.app.ui.accounts

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.margin.app.data.db.AccountEntity
import com.margin.app.data.prefs.SettingsRepository
import com.margin.app.data.repo.LedgerRepository
import com.margin.app.ui.common.Notices
import com.margin.app.ui.nav.Args
import com.margin.core.AccountType
import com.margin.core.Copy
import com.margin.core.MoneyFormatter
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
    /** The balance as it will read with the draft's opening balance. */
    val balance: Long = 0,
    val entryCount: Int = 0,
    val share: SharePreview = SharePreview(),
    val problems: AccountProblems = AccountProblems(),
    /** True once a save was tried with problems: the form then says what is missing. */
    val showErrors: Boolean = false,
    /** The real entry count while the delete dialog is open; null when it is closed. */
    val confirmDelete: Int? = null,
) {
    val isNew: Boolean get() = !stored
}

@HiltViewModel
class AccountEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L

    private data class Flags(val loaded: Boolean = false, val showErrors: Boolean = false, val confirmDelete: Int? = null)

    private val draft = MutableStateFlow(AccountDraft())
    private val stored = MutableStateFlow<AccountEntity?>(null)
    private val flags = MutableStateFlow(Flags())
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or delete; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    val state: StateFlow<AccountEditState> = combine(draft, stored, ledger.balances(), flags) { d, st, balances, f ->
        val row = st?.let { s -> balances.firstOrNull { it.id == s.id } }
        val opening = d.signedOpening ?: st?.openingBalance ?: 0L
        // The stored balance already holds the stored opening; swap in the draft's.
        val balance = if (st != null && row != null) row.balance - st.openingBalance + opening else opening
        AccountEditState(
            id = st?.id ?: 0L,
            draft = d,
            loaded = f.loaded,
            stored = st != null,
            storedName = st?.name.orEmpty(),
            balance = balance,
            entryCount = row?.entryCount ?: 0,
            share = previewShare(st?.id ?: 0L, balance, d.archived, balances),
            problems = accountProblems(d),
            showErrors = f.showErrors,
            confirmDelete = f.confirmDelete,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AccountEditState())

    init {
        viewModelScope.launch {
            val s = settings.current()
            val account = if (argId != 0L) ledger.account(argId) else null
            if (account != null) {
                val money = MoneyFormatter(s.currency, Locale.getDefault())
                draft.value = AccountDraft(
                    name = account.name,
                    type = account.type,
                    openingText = if (account.openingBalance == 0L) "" else money.formatInput(account.openingBalance),
                    opening = abs(account.openingBalance),
                    owe = account.openingBalance < 0,
                    isDefault = s.defaultAccountId == account.id,
                    archived = account.archived,
                )
                stored.value = account
            } else {
                // The first account becomes the default, so the entry screen has somewhere to start.
                draft.value = AccountDraft(isDefault = s.defaultAccountId == 0L)
            }
            flags.update { it.copy(loaded = true) }
        }
    }

    fun setName(name: String) = draft.update { it.copy(name = name) }

    fun setType(type: AccountType) = draft.update { it.copy(type = type) }

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
            val currentDefault = settings.current().defaultAccountId
            if (d.effectiveDefault) {
                if (currentDefault != id) settings.setDefaultAccount(id)
            } else if (currentDefault == id) {
                settings.setDefaultAccount(0L)
            }
            finished.send(Unit)
        }
    }

    /** Counts the entries afresh and opens the dialog that names them. */
    fun requestDelete() {
        val original = stored.value ?: return
        if (busy) return
        viewModelScope.launch {
            val count = ledger.entriesForAccount(original.id)
            flags.update { it.copy(confirmDelete = count) }
        }
    }

    fun dismissDelete() = flags.update { it.copy(confirmDelete = null) }

    /** Irreversible: the account goes, and through the foreign key every entry that touches it. */
    fun delete() {
        val original = stored.value ?: return
        if (busy) return
        busy = true
        val count = flags.value.confirmDelete ?: 0
        flags.update { it.copy(confirmDelete = null) }
        viewModelScope.launch {
            ledger.deleteAccount(original.id)
            if (settings.current().defaultAccountId == original.id) settings.setDefaultAccount(0L)
            notices.show(
                if (count == 0) "${original.name} deleted"
                else "${original.name} deleted with its ${Copy.plural(count, "entry", "entries")}"
            )
            finished.send(Unit)
        }
    }
}
