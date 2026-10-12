package com.tally.app.ui.invest

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.repo.InvestRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.core.AccountType
import com.tally.core.Portfolio
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import java.time.LocalDate
import javax.inject.Inject

/** Everything the Investments screen reads; the lens and every figure are folded from it by [investView]. */
@Immutable
data class InvestState(
    val today: LocalDate,
    val loaded: Boolean = false,
    /** The portfolio as core reads it from the ledger's rows; null before the first read or when it failed. */
    val portfolio: Portfolio? = null,
    /** The open investment accounts with their balances, for the ones valued by hand. */
    val accounts: List<AccountBalance> = emptyList(),
    /** Why the portfolio could not be read; the page says it instead of drawing figures it does not have. */
    val problem: String? = null,
)

/** The portfolio read once, or why it could not be. */
private class Read(val portfolio: Portfolio?, val problem: String?)

/**
 * The Investments screen: the portfolio core reads from the ledger (holdings, activities, prices,
 * room) beside the investment accounts' balances, so an account valued by hand still counts. Nothing
 * here is stored; the screen re-reads it on every change, as the PC does.
 */
@HiltViewModel
class InvestViewModel @Inject constructor(
    invest: InvestRepository,
    ledger: LedgerRepository,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so the page left open overnight reads the new day's staleness. */
    private val today = MutableStateFlow(clock.today())

    fun onResume() { today.value = clock.today() }

    private val read: Flow<Read> = invest.portfolio()
        .map { Read(it, null) }
        .catch { emit(Read(null, "These investments could not be added up. Importing the newest holdings report again usually settles it.")) }

    val state: StateFlow<InvestState> = combine(read, ledger.balances(), today) { r, balances, day ->
        InvestState(
            today = day,
            loaded = true,
            portfolio = r.portfolio,
            accounts = balances.filter { it.type == AccountType.INVESTMENT && !it.archived },
            problem = r.problem,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), InvestState(today = clock.today()))
}
