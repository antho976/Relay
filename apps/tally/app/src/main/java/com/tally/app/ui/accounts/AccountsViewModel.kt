package com.tally.app.ui.accounts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/** The Accounts list: what each account paid out this month, and the balances of the ones that track one. */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class AccountsViewModel @Inject constructor(
    ledger: LedgerRepository,
    settings: SettingsRepository,
    clock: Clock,
) : ViewModel() {

    private val spent = settings.settings.map { it.periodFor(clock.today()) }.distinctUntilChanged().flatMapLatest { period ->
        ledger.spendByAccount(period.start, period.endExclusive).map { rows -> rows.associate { it.accountId to it.total } }
    }

    val state: StateFlow<AccountsState> = combine(ledger.balances(), settings.settings, ledger.count(), spent) { balances, s, count, out ->
        buildAccountsState(balances, s.defaultAccountId, count, out)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AccountsState())
}
