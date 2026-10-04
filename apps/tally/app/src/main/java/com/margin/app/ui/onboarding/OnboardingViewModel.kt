package com.margin.app.ui.onboarding

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.margin.app.data.Clock
import com.margin.app.data.db.AccountEntity
import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.prefs.Settings
import com.margin.app.data.prefs.SettingsRepository
import com.margin.app.data.repo.DataRepository
import com.margin.app.data.repo.DataResult
import com.margin.app.data.repo.LedgerRepository
import com.margin.app.data.repo.PlanRepository
import com.margin.core.AccountType
import com.margin.core.BudgetPeriod
import com.margin.core.MoneyFormatter
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/**
 * First run. The draft lives here until the last step; only the currency is written as it is
 * picked, so every amount on the next two steps already reads in it.
 */
@HiltViewModel
class OnboardingViewModel @Inject constructor(
    private val settings: SettingsRepository,
    private val ledger: LedgerRepository,
    private val plan: PlanRepository,
    private val data: DataRepository,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private val draft = MutableStateFlow(OnboardingState(today = today, period = BudgetPeriod.containing(today)))

    val state: StateFlow<OnboardingState> = draft.asStateFlow()

    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once when the ledger is set up (or the sample is in); the route leaves for Home on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    init {
        viewModelScope.launch {
            val s = settings.current()
            draft.update {
                it.copy(
                    currency = s.currency,
                    deviceCurrency = Settings.defaultCurrency(),
                    period = s.periodFor(today),
                    loaded = true,
                )
            }
        }
    }

    /** Writes the pick at once, and reads any amount already typed again at the new currency's decimals. */
    fun pickCurrency(code: String) {
        val fmt = MoneyFormatter(code, Locale.getDefault())
        draft.update { it.copy(currency = code, balance = fmt.parse(it.balanceText), budget = fmt.parse(it.budgetText)) }
        viewModelScope.launch { settings.setCurrency(code) }
    }

    fun toggleMoreCurrencies() = draft.update { it.copy(moreCurrencies = !it.moreCurrencies) }

    fun setName(name: String) = draft.update { it.copy(accountName = name) }

    fun setType(type: AccountType) = draft.update {
        it.copy(accountType = type, accountName = nameForType(it.accountName, it.accountType, type))
    }

    fun setBalance(text: String, parsed: Long?) = draft.update { it.copy(balanceText = text, balance = parsed, error = null) }

    fun setBudget(text: String, parsed: Long?) = draft.update { it.copy(budgetText = text, budget = parsed, error = null) }

    /** The step's main act: on to the next question, or set everything up after the last. */
    fun next() {
        val d = draft.value
        if (!d.canContinue) return
        val following = nextStep(d.step)
        if (following != null) draft.update { it.copy(step = following, error = null) } else finish(withBudget = true)
    }

    fun back() {
        val d = draft.value
        if (d.busy) return
        previousStep(d.step)?.let { prev -> draft.update { it.copy(step = prev, error = null) } }
    }

    /** "skip": no monthly budget; Home then reads the month against income. */
    fun skipBudget() {
        if (draft.value.busy) return
        finish(withBudget = false)
    }

    private fun finish(withBudget: Boolean) {
        val d = draft.value
        if (d.busy) return
        draft.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            val ok = runCatching {
                settings.setCurrency(d.currency)
                ledger.seedCategoriesIfEmpty()
                val id = ledger.saveAccount(
                    AccountEntity(name = accountNameToSave(d.accountName, d.accountType), type = d.accountType, openingBalance = d.storedBalance)
                )
                settings.setDefaultAccount(id)
                val budget = d.budget ?: 0L
                if (withBudget && budget > 0L) plan.setBudget(BudgetEntity.OVERALL, budget)
                settings.setOnboarded(true)
            }.isSuccess
            if (ok) {
                finished.send(Unit)
            } else {
                draft.update { it.copy(busy = false, error = "Setup did not finish. Try again.") }
            }
        }
    }

    /** Fills the empty ledger with the labelled sample, in the picked currency, and goes straight to Home. */
    fun trySample() {
        val d = draft.value
        if (d.busy || !d.loaded) return
        draft.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            settings.setCurrency(d.currency)
            when (val result = data.loadSample()) {
                is DataResult.Done -> finished.send(Unit)
                is DataResult.Failed -> draft.update { it.copy(busy = false, error = result.message) }
            }
        }
    }
}
