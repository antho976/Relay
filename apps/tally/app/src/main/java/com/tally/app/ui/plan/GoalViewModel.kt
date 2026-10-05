package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import com.tally.core.GoalKind
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

/** Everything the goal screen draws. */
@Immutable
data class GoalState(
    val today: LocalDate,
    val loaded: Boolean = false,
    /** Null once loaded means the goal is gone (deleted from its editor). */
    val goal: GoalLine? = null,
    /** Newest first. */
    val contributions: List<ContributionEntity> = emptyList(),
    val pace: GoalPace? = null,
    /** A balance goal's day it was set, where its pace is measured from. */
    val startDate: LocalDate? = null,
    val added: Long = 0,
    val withdrawn: Long = 0,
    val addedCount: Int = 0,
    val withdrawnCount: Int = 0,
)

/** One goal: what it holds against its target, its pace, and (a savings pot's) every contribution. */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class GoalViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    settings: SettingsRepository,
    goalSource: GoalSource,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val goalId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val today: LocalDate = clock.today()

    private val inputs = settings.settings
        .map { it.periodFor(today) }
        .distinctUntilChanged()
        .flatMapLatest { period -> goalSource.inputs(period, today) }

    val state: StateFlow<GoalState> = combine(plan.goals(), plan.contributions(goalId), inputs) { goals, contributions, i ->
        val row = goals.firstOrNull { it.goal.id == goalId }
        val line = row?.let { goalLineOf(it, i) }
        GoalState(
            today = today,
            loaded = true,
            goal = line,
            contributions = contributions,
            pace = line?.let {
                if (it.kind == GoalKind.BALANCE) balancePace(it, row?.goal?.startDate, today) else goalPace(contributions, it.saved, it.target, today)
            },
            startDate = row?.goal?.startDate,
            added = contributions.filter { it.amount > 0L }.sumOf { it.amount },
            withdrawn = contributions.filter { it.amount < 0L }.sumOf { -it.amount },
            addedCount = contributions.count { it.amount > 0L },
            withdrawnCount = contributions.count { it.amount < 0L },
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), GoalState(today = today))

    /** True while a contribution is being written: a second tap in that time is the same one. */
    private var posting = false

    /** Money in (positive) or out (negative). */
    fun contribute(amount: Long, note: String) {
        if (amount == 0L || goalId == 0L || posting) return
        posting = true
        viewModelScope.launch {
            try {
                plan.contribute(goalId, amount, note)
            } finally {
                posting = false
            }
        }
    }

    /** Removes one contribution at once and offers Undo. */
    fun deleteContribution(id: Long) {
        val repo = plan
        viewModelScope.launch {
            val row = repo.deleteContribution(id) ?: return@launch
            // The undo holds the repository and the row, never this ViewModel.
            notices.showUndo(if (row.amount < 0L) "Withdrawal removed" else "Contribution removed") { repo.restoreContribution(row) }
        }
    }
}
