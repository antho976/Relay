package com.tally.app.ui.accounts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/** The Accounts list: every balance, the net, and what is held against what is owed. */
@HiltViewModel
class AccountsViewModel @Inject constructor(
    ledger: LedgerRepository,
    settings: SettingsRepository,
) : ViewModel() {

    val state: StateFlow<AccountsState> = combine(ledger.balances(), settings.settings, ledger.count()) { balances, s, count ->
        buildAccountsState(balances, s.defaultAccountId, count)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AccountsState())
}
