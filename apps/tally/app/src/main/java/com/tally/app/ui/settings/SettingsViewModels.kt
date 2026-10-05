package com.tally.app.ui.settings

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.BuildConfig
import com.tally.app.data.Clock
import com.tally.app.data.prefs.Accent
import com.tally.app.data.prefs.BackupPrefs
import com.tally.app.data.prefs.Settings
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.CurrencyPlan
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

// ── Settings ─────────────────────────────────────────────────────────────────

/** Everything the Settings page reads: the look and format it summarises, and what the ledger holds. */
@Immutable
data class SettingsState(
    val today: LocalDate,
    val settings: Settings = Settings(currency = "CAD"),
    val period: BudgetPeriod = BudgetPeriod.containing(today),
    val version: String = "1.0",
    val entries: Int = 0,
    val accounts: Int = 0,
    /** Across the accounts that are not archived. */
    val net: Long = 0,
    val spendingCategories: Int = 0,
    val incomeCategories: Int = 0,
    val budgets: Int = 0,
    val bills: Int = 0,
    val goals: Int = 0,
    val backup: BackupPrefs = BackupPrefs(),
    val loaded: Boolean = false,
) {
    /** The sample loads only into an app with nothing the owner made (the seeded categories aside). */
    val canLoadSample: Boolean get() = entries == 0 && accounts == 0 && budgets == 0 && bills == 0 && goals == 0
}

@HiltViewModel
class SettingsViewModel @Inject constructor(
    settings: SettingsRepository,
    ledger: LedgerRepository,
    plan: PlanRepository,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private data class LedgerCounts(val entries: Int, val accounts: Int, val net: Long, val spending: Int, val income: Int)
    private data class PlanCounts(val budgets: Int, val bills: Int, val goals: Int)

    private val ledgerCounts = combine(ledger.count(), ledger.balances(), ledger.categories()) { count, balances, categories ->
        val open = balances.filter { !it.archived }
        val live = categories.filter { !it.archived }
        LedgerCounts(
            entries = count,
            accounts = open.size,
            net = open.sumOf { it.balance },
            spending = live.count { it.kind == CategoryKind.EXPENSE },
            income = live.count { it.kind == CategoryKind.INCOME },
        )
    }

    private val planCounts = combine(plan.budgets(), plan.recurring(), plan.goals()) { budgets, bills, goals ->
        PlanCounts(
            budgets = budgets.size,
            bills = bills.count { it.recurring.active },
            goals = goals.count { !it.goal.archived },
        )
    }

    val state: StateFlow<SettingsState> = combine(settings.settings, ledgerCounts, planCounts, settings.backup) { s, l, p, b ->
        SettingsState(
            today = today,
            settings = s,
            period = s.periodFor(today),
            version = BuildConfig.VERSION_NAME,
            entries = l.entries,
            accounts = l.accounts,
            net = l.net,
            spendingCategories = l.spending,
            incomeCategories = l.income,
            budgets = p.budgets,
            bills = p.bills,
            goals = p.goals,
            backup = b,
            loaded = true,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SettingsState(today = today))
}

// ── Appearance ───────────────────────────────────────────────────────────────

@Immutable
data class AppearanceState(
    val accent: Accent = Accent.DEFAULT,
    val accentEnabled: Boolean = true,
    val amoled: Boolean = false,
    val loaded: Boolean = false,
)

/** The look. Every write lands at once; the theme at the root follows the stored settings live. */
@HiltViewModel
class AppearanceViewModel @Inject constructor(
    private val settings: SettingsRepository,
) : ViewModel() {

    val state: StateFlow<AppearanceState> = settings.settings
        .map { AppearanceState(accent = it.accent, accentEnabled = it.accentEnabled, amoled = it.amoled, loaded = true) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AppearanceState())

    fun setAmoled(on: Boolean) {
        viewModelScope.launch { settings.setAmoled(on) }
    }

    fun setAccentEnabled(on: Boolean) {
        viewModelScope.launch { settings.setAccentEnabled(on) }
    }

    fun setAccent(accent: Accent) {
        viewModelScope.launch { settings.setAccent(accent) }
    }
}

// ── Currency & dates ─────────────────────────────────────────────────────────

@Immutable
data class FormatState(
    val today: LocalDate,
    val currency: String = "CAD",
    val monthStartDay: Int = 1,
    val weekStartsMonday: Boolean = true,
    val period: BudgetPeriod = BudgetPeriod.containing(today),
    /** A switch to fewer decimals that would round stored amounts, waiting for the owner's yes. */
    val pendingCurrency: CurrencyPlan? = null,
    val loaded: Boolean = false,
)

@HiltViewModel
class FormatViewModel @Inject constructor(
    private val settings: SettingsRepository,
    private val data: DataRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    private val pending = MutableStateFlow<CurrencyPlan?>(null)

    /** A switch is being worked out or written; further taps wait for it instead of racing it. */
    private var switching = false

    val state: StateFlow<FormatState> = combine(settings.settings, pending) { s, p ->
        FormatState(
            today = today,
            currency = s.currency,
            monthStartDay = s.monthStartDay,
            weekStartsMonday = s.weekStartsMonday,
            period = s.periodFor(today),
            pendingCurrency = p,
            loaded = true,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), FormatState(today = today))

    /**
     * Amounts are stored in the current currency's minor units, so a switch to a currency with
     * other decimals rewrites them to keep each figure. When that would round any amount, the
     * screen asks first; an exact switch (same decimals, more of them, or only whole amounts)
     * runs at once.
     */
    fun setCurrency(code: String) {
        if (switching || pending.value != null) return
        switching = true
        viewModelScope.launch {
            try {
                val plan = runCatching { data.planCurrencyChange(code) }.getOrNull() ?: return@launch
                if (plan.rounded > 0) pending.value = plan else switchTo(plan.code)
            } finally {
                switching = false
            }
        }
    }

    /** The owner said yes to the rounding. */
    fun confirmCurrency() {
        val plan = pending.value ?: return
        pending.value = null
        if (switching) return
        switching = true
        viewModelScope.launch {
            try {
                switchTo(plan.code)
            } finally {
                switching = false
            }
        }
    }

    fun cancelCurrency() {
        pending.value = null
    }

    private suspend fun switchTo(code: String) {
        val result = data.changeCurrency(code)
        if (result is DataResult.Failed) notices.show(result.message)
    }

    fun setMonthStartDay(day: Int) {
        viewModelScope.launch { settings.setMonthStartDay(day) }
    }

    fun setWeekStartsMonday(monday: Boolean) {
        viewModelScope.launch { settings.setWeekStartsMonday(monday) }
    }
}
