package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
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
    val added: Long = 0,
    val withdrawn: Long = 0,
    val addedCount: Int = 0,
    val withdrawnCount: Int = 0,
)

/** One goal: what is saved against the target, its pace, and every contribution. */
@HiltViewModel
class GoalViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val goalId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val today: LocalDate = clock.today()

    val state: StateFlow<GoalState> = combine(plan.goals(), plan.contributions(goalId)) { goals, contributions ->
        val line = goals.firstOrNull { it.goal.id == goalId }?.let { goalLine(it, today) }
        GoalState(
            today = today,
            loaded = true,
            goal = line,
            contributions = contributions,
            pace = line?.let { goalPace(contributions, it.saved, it.target, today) },
            added = contributions.filter { it.amount > 0L }.sumOf { it.amount },
            withdrawn = contributions.filter { it.amount < 0L }.sumOf { -it.amount },
            addedCount = contributions.count { it.amount > 0L },
            withdrawnCount = contributions.count { it.amount < 0L },
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), GoalState(today = today))

    /** Money in (positive) or out (negative). */
    fun contribute(amount: Long, note: String) {
        if (amount == 0L || goalId == 0L) return
        viewModelScope.launch { plan.contribute(goalId, amount, note) }
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
