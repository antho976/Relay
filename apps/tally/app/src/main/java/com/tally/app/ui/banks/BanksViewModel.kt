package com.tally.app.ui.banks

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.banks.Bank
import com.tally.app.data.banks.BankAccount
import com.tally.app.data.banks.Banks
import com.tally.app.data.banks.BanksAnswer
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.sync.SyncScheduler
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

/** How far back a newly followed bank is read. */
enum class Since(val label: String) {
    MONTH("Last 30 days"),
    YEAR("Since January 1"),
    TODAY("From today on"),
    ;

    fun date(today: LocalDate): String = when (this) {
        MONTH -> today.minusDays(30)
        YEAR -> today.withDayOfYear(1)
        TODAY -> today
    }.toString()
}

data class BanksState(
    val loading: Boolean = true,
    val answer: BanksAnswer? = null,
    /** A bank just connected, its accounts being picked; null on the list. */
    val choosing: Bank? = null,
    val since: Since = Since.MONTH,
    /** Talking to the server or Plaid: what it is doing, in words. */
    val working: String? = null,
    val problem: String? = null,
    val notice: String? = null,
)

@HiltViewModel
class BanksViewModel @Inject constructor(
    private val banks: Banks,
    private val scheduler: SyncScheduler,
    ledger: LedgerRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(BanksState())
    val state: StateFlow<BanksState> = _state.asStateFlow()

    /** Tally's own accounts, for "Into" and for matching a bank's accounts to them. */
    val accounts: StateFlow<List<AccountEntity>> = ledger.accounts().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    init { reload() }

    fun reload() = viewModelScope.launch {
        _state.update { it.copy(loading = true) }
        val answer = banks.list()
        _state.update { it.copy(loading = false, answer = answer) }
    }

    /** Asks the server for a link token; [open] runs Plaid's sign-in with it. */
    fun startLink(open: (String) -> Unit) = viewModelScope.launch {
        _state.update { it.copy(working = "Asking your server to open Plaid", problem = null) }
        banks.linkToken()
            .onSuccess { token -> _state.update { it.copy(working = null) }; open(token) }
            .onFailure { e -> _state.update { it.copy(working = null, problem = e.message) } }
    }

    fun linked(publicToken: String, institutionId: String?, institutionName: String?) = viewModelScope.launch {
        _state.update { it.copy(working = "Connecting " + (institutionName ?: "your bank"), problem = null) }
        banks.exchange(publicToken, institutionId, institutionName)
            .onSuccess { bank ->
                val suggested = bank.copy(accounts = bank.accounts.map { a -> a.copy(follow = followsByDefault(a), into = a.into ?: match(bank.institution, a, accounts.value)?.uid) })
                _state.update { it.copy(working = null, choosing = suggested) }
            }
            .onFailure { e -> _state.update { it.copy(working = null, problem = e.message) } }
    }

    fun linkClosed(reason: String?) {
        _state.update { it.copy(working = null, problem = reason) }
    }

    fun toggle(accountId: String) = _state.update { s ->
        val b = s.choosing ?: return@update s
        s.copy(choosing = b.copy(accounts = b.accounts.map { if (it.id == accountId) it.copy(follow = !it.follow) else it }))
    }

    fun into(accountId: String, uid: String?) = _state.update { s ->
        val b = s.choosing ?: return@update s
        s.copy(choosing = b.copy(accounts = b.accounts.map { if (it.id == accountId) it.copy(into = uid) else it }))
    }

    fun since(value: Since) = _state.update { it.copy(since = value) }

    fun follow() = viewModelScope.launch {
        val b = _state.value.choosing ?: return@launch
        _state.update { it.copy(working = "Bringing in your purchases", problem = null) }
        banks.follow(b.id, b.accounts, _state.value.since.date(LocalDate.now()))
            .onSuccess {
                scheduler.syncNow()
                _state.update { it.copy(working = null, choosing = null, notice = b.institution + " is connected. New purchases arrive with the next sync.") }
                reload()
            }
            .onFailure { e -> _state.update { it.copy(working = null, problem = e.message) } }
    }

    /** Edits which accounts of a connected bank are followed. */
    fun edit(bank: Bank) = _state.update { it.copy(choosing = bank) }

    fun cancelChoosing() = _state.update { it.copy(choosing = null) }

    fun refresh() = viewModelScope.launch {
        _state.update { it.copy(working = "Checking your banks", problem = null) }
        banks.refresh()
            .onSuccess { scheduler.syncNow(); _state.update { it.copy(working = null, notice = "Your server is reading your banks. New purchases arrive with the next sync.") } }
            .onFailure { e -> _state.update { it.copy(working = null, problem = e.message) } }
    }

    fun remove(bank: Bank) = viewModelScope.launch {
        _state.update { it.copy(working = "Disconnecting " + bank.institution, problem = null) }
        banks.remove(bank.id)
            .onSuccess { _state.update { it.copy(working = null, notice = bank.institution + " is disconnected. Its entries stay in your ledger.") }; reload() }
            .onFailure { e -> _state.update { it.copy(working = null, problem = e.message) } }
    }

    fun dismissNotice() = _state.update { it.copy(notice = null, problem = null) }

    companion object {
        /** Spending accounts are followed unless the person says otherwise; savings and investments are not. */
        fun followsByDefault(a: BankAccount): Boolean {
            val k = a.kind.lowercase()
            return listOf("checking", "credit", "cash", "prepaid", "chequing").any { it in k } || k.isBlank()
        }

        /** The Tally account a bank's account most likely is: same bank in its name, same kind. */
        fun match(institution: String, a: BankAccount, mine: List<AccountEntity>): AccountEntity? {
            val bank = institution.lowercase().split(' ').firstOrNull().orEmpty()
            val credit = "credit" in a.kind.lowercase()
            val live = mine.filter { !it.archived }
            val sameBank = live.filter { bank.isNotBlank() && (bank in it.name.lowercase() || bank in it.institution.lowercase()) }
            val byMask = live.firstOrNull { a.mask.isNotBlank() && it.externalRef.endsWith(a.mask) }
            if (byMask != null) return byMask
            val byKind = sameBank.filter { (it.type == com.tally.core.AccountType.CREDIT) == credit }
            return byKind.singleOrNull() ?: sameBank.singleOrNull()
        }
    }
}
