package com.tally.app.ui.shell

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.data.sync.CloudConnect
import com.tally.app.data.sync.Connect
import com.tally.app.data.sync.RelaySync
import com.tally.app.data.sync.SyncPrefs
import com.tally.app.data.sync.SyncScheduler
import com.tally.app.data.sync.SyncStore
import com.tally.app.ui.common.Notices
import com.tally.core.BudgetPeriod
import com.tally.core.Copy
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

/** What the frame shows on every page: the sync lamp and line, and the month in a few words. */
@Immutable
data class ShellState(
    val sync: SyncLine = SyncLine("", Tone.OFF),
    val period: BudgetPeriod? = null,
    /** What the overall budget has left, or null without one. */
    val left: Long? = null,
    val spent: Long = 0,
    val daysLeft: Int = 0,
    val connect: Connect = Connect.Idle,
    val paired: Boolean = false,
)

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class ShellViewModel @Inject constructor(
    settings: SettingsRepository,
    private val ledger: LedgerRepository,
    plan: PlanRepository,
    store: SyncStore,
    sync: RelaySync,
    private val cloud: CloudConnect,
    private val scheduler: SyncScheduler,
    private val notices: Notices,
    private val clock: Clock,
) : ViewModel() {

    /** Ticks each half minute, so "synced 2m ago" stays true while a page stays open. */
    private val now = MutableStateFlow(System.currentTimeMillis())
    private val today = MutableStateFlow(clock.today())

    init {
        viewModelScope.launch {
            while (isActive) {
                delay(30_000)
                now.value = System.currentTimeMillis()
                today.value = clock.today()
            }
        }
    }

    private data class Month(val period: BudgetPeriod, val day: LocalDate, val left: Long?, val spent: Long)

    private val month = combine(settings.settings, today) { s, d -> s.periodFor(d) to d }.flatMapLatest { (period, day) ->
        combine(ledger.typeTotals(period.start, period.endExclusive), plan.budgets()) { totals, budgets ->
            val spent = totals.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L
            val budget = budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }?.amount
            Month(period, day, budget?.let { it - spent }, spent)
        }
    }

    private data class Link(val prefs: SyncPrefs, val busy: Boolean, val connect: Connect, val now: Long)

    private val link = combine(store.prefs, sync.busy, cloud.state, now) { p, b, c, n -> Link(p, b, c, n) }

    val state: StateFlow<ShellState> = combine(month, link) { m, l ->
        ShellState(
            sync = syncLine(l.prefs, l.busy, l.connect, l.now),
            period = m.period,
            left = m.left,
            spent = m.spent,
            daysLeft = m.period.daysLeft(m.day),
            connect = l.connect,
            paired = l.prefs.paired,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ShellState())

    /** The status line was tapped: sync now (or connect, before the first time). */
    fun syncNow() = scheduler.syncNow()

    fun retryConnect() = cloud.retry()

    /** Deletes an entry from a list at once; the toast offers it back. */
    fun deleteEntry(id: Long) {
        viewModelScope.launch {
            val row = ledger.delete(id) ?: return@launch
            notices.showUndo("Entry deleted") { ledger.restore(row) }
        }
    }

    /** "3 days left", for the status bar. */
    fun daysText(days: Int): String = Copy.plural(days, "day") + " left"
}
