package com.margin.app.ui.categories

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.margin.app.data.Clock
import com.margin.app.data.prefs.SettingsRepository
import com.margin.app.data.repo.LedgerRepository
import com.margin.core.BudgetPeriod
import com.margin.core.CategoryKind
import com.margin.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import java.time.LocalDate
import javax.inject.Inject

/** The Categories list: both kinds, each with its entry counts and this period's money by category. */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class CategoriesViewModel @Inject constructor(
    ledger: LedgerRepository,
    settings: SettingsRepository,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    val state: StateFlow<CategoriesState> = settings.settings
        .map { it.periodFor(today) }
        .distinctUntilChanged()
        .flatMapLatest { period ->
            combine(
                ledger.categoriesWithCounts(),
                ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive),
                ledger.byCategory(TxType.INCOME, period.start, period.endExclusive),
            ) { categories, spent, earned ->
                CategoriesState(
                    today = today,
                    period = period,
                    spending = summarize(CategoryKind.EXPENSE, categories, spent),
                    income = summarize(CategoryKind.INCOME, categories, earned),
                    loaded = true,
                )
            }
        }
        .stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            CategoriesState(today = today, period = BudgetPeriod.containing(today)),
        )
}
