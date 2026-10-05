package com.tally.app.ui.onboarding

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.prefs.Settings
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import com.tally.core.MoneyFormatter
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
 * First run. The draft lives here until the last step, kept in the SavedStateHandle as it changes
 * so a process death resumes on the same question; only the currency is written as it is picked,
 * so every amount on the next two steps already reads in it.
 */
@HiltViewModel
class OnboardingViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
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
            val kept = savedStateHandle.savedDraft(SavedOnboarding.serializer())
            val s = settings.current()
            val fmt = MoneyFormatter(s.currency, Locale.getDefault())
            draft.update {
                val base = it.copy(
                    currency = s.currency,
                    deviceCurrency = Settings.defaultCurrency(),
                    period = s.periodFor(today),
                    loaded = true,
                )
                kept?.applyTo(base) { text -> fmt.parse(text) } ?: base
            }
            draft.collect { savedStateHandle.keepDraft(SavedOnboarding.serializer(), SavedOnboarding.of(it)) }
        }
    }

    /** Writes the pick at once, and reads any amount already typed again at the new currency's decimals. */
    fun pickCurrency(code: String) {
        val fmt = MoneyFormatter(code, Locale.getDefault())
        draft.update {
            it.copy(
                currency = code,
                accounts = it.accounts.map { a -> a.copy(balance = fmt.parse(a.balanceText) ?: 0L) },
                balance = fmt.parse(it.balanceText),
                budget = fmt.parse(it.budgetText),
            )
        }
        viewModelScope.launch { settings.setCurrency(code) }
    }

    fun toggleMoreCurrencies() = draft.update { it.copy(moreCurrencies = !it.moreCurrencies) }

    fun setName(name: String) = draft.update { it.copy(accountName = name) }

    fun setType(type: AccountType) = draft.update {
        it.copy(accountType = type, accountName = nameForType(it.accountName, it.accountType, type))
    }

    fun setBalance(text: String, parsed: Long?) = draft.update { it.copy(balanceText = text, balance = parsed, error = null) }

    fun setBudget(text: String, parsed: Long?) = draft.update { it.copy(budgetText = text, budget = parsed, error = null) }

    /** Moves the form's account into the list and closes the form; "Add another account" opens a fresh one. */
    fun addAccount() = draft.update { d ->
        if (!d.formOpen || d.balanceProblem != null || d.busy) return@update d
        cleared(d.copy(accounts = d.accounts + d.draftAccount, editing = false, error = null))
    }

    /** Opens the form for one more account, on the next usual type. */
    fun openNewAccount() = draft.update { d -> if (d.busy) d else cleared(d).copy(editing = true) }

    /** Closes a form opened by mistake; only once there is an account in the list. */
    fun cancelNewAccount() = draft.update { d -> if (d.accounts.isEmpty() || d.busy) d else cleared(d).copy(editing = false, error = null) }

    /** Takes an added account back out; with none left, the form opens again. */
    fun removeAccount(index: Int) = draft.update { d ->
        if (d.busy || index !in d.accounts.indices) return@update d
        val left = d.accounts.filterIndexed { i, _ -> i != index }
        d.copy(accounts = left, editing = d.editing || left.isEmpty())
    }

    /** A blank form on the type the list does not have yet. */
    private fun cleared(d: OnboardingState): OnboardingState {
        val type = suggestedType(d.accounts)
        return d.copy(accountType = type, accountName = defaultAccountName(type), balanceText = "", balance = null)
    }

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
                // Every account in one write; the first added is where new entries start.
                val ids = ledger.saveNewAccounts(
                    d.toCreate.map { AccountEntity(name = it.name, type = it.type, openingBalance = it.storedBalance) }
                )
                ids.firstOrNull()?.let { settings.setDefaultAccount(it) }
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
