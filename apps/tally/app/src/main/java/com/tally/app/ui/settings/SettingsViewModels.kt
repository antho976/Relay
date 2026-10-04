package com.tally.app.ui.settings

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.BuildConfig
import com.tally.app.data.Clock
import com.tally.app.data.prefs.Accent
import com.tally.app.data.prefs.Settings
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import dagger.hilt.android.lifecycle.HiltViewModel
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
    val loaded: Boolean = false,
)

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

    val state: StateFlow<SettingsState> = combine(settings.settings, ledgerCounts, planCounts) { s, l, p ->
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
    val loaded: Boolean = false,
)

@HiltViewModel
class FormatViewModel @Inject constructor(
    private val settings: SettingsRepository,
    clock: Clock,
) : ViewModel() {

    private val today: LocalDate = clock.today()

    val state: StateFlow<FormatState> = settings.settings
        .map { s ->
            FormatState(
                today = today,
                currency = s.currency,
                monthStartDay = s.monthStartDay,
                weekStartsMonday = s.weekStartsMonday,
                period = s.periodFor(today),
                loaded = true,
            )
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), FormatState(today = today))

    fun setCurrency(code: String) {
        viewModelScope.launch { settings.setCurrency(code) }
    }

    fun setMonthStartDay(day: Int) {
        viewModelScope.launch { settings.setMonthStartDay(day) }
    }

    fun setWeekStartsMonday(monday: Boolean) {
        viewModelScope.launch { settings.setWeekStartsMonday(monday) }
    }
}
