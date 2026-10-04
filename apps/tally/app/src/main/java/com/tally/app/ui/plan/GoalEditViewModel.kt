package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import com.tally.core.MoneyFormatter
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** Everything the goal editor draws. */
@Immutable
data class GoalEditState(
    val draft: GoalDraft,
    val today: LocalDate,
    val loaded: Boolean = false,
    /** True when the goal exists in the database (editing, not new). */
    val stored: Boolean = false,
    /** What the goal already holds, for the preview. */
    val saved: Long = 0,
    val monthsLeft: Int? = null,
    val problems: GoalProblems = GoalProblems(),
    /** True once a save was tried with problems: the form then says what is missing. */
    val showErrors: Boolean = false,
) {
    val isNew: Boolean get() = !stored
}

@HiltViewModel
class GoalEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val today: LocalDate = clock.today()

    private val draft = MutableStateFlow(GoalDraft(id = argId))
    private val stored = MutableStateFlow<GoalEntity?>(null)
    private val ready = MutableStateFlow(false)
    private val showErrors = MutableStateFlow(false)
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or delete; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    val state: StateFlow<GoalEditState> = combine(draft, stored, plan.goals(), ready, showErrors) { d, st, goals, isReady, errors ->
        GoalEditState(
            draft = d,
            today = today,
            loaded = isReady,
            stored = st != null,
            saved = if (st == null) 0L else goals.firstOrNull { it.goal.id == st.id }?.saved ?: 0L,
            monthsLeft = d.targetDate?.let { monthsUntil(today, it) },
            problems = goalProblems(d),
            showErrors = errors,
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        GoalEditState(draft = draft.value, today = today),
    )

    init {
        viewModelScope.launch {
            val s = settings.current()
            val money = MoneyFormatter(s.currency, Locale.getDefault())
            val goal = if (argId != 0L) plan.goal(argId) else null
            if (goal != null) {
                stored.value = goal
                draft.value = GoalDraft(
                    id = goal.id,
                    name = goal.name,
                    targetText = money.formatInput(goal.target),
                    target = goal.target,
                    targetDate = goal.targetDate,
                    color = goal.color,
                    archived = goal.archived,
                )
            } else {
                draft.update { it.copy(id = 0L) }
            }
            ready.value = true
        }
    }

    fun setName(name: String) = draft.update { it.copy(name = name) }

    /** [target] is [text] read by the screen's money formatter; null when it is not an amount. */
    fun setTarget(text: String, target: Long?) = draft.update { it.copy(targetText = text, target = target) }

    fun setDate(date: LocalDate?) = draft.update { it.copy(targetDate = date) }

    fun setColor(color: Int) = draft.update { it.copy(color = color) }

    fun save() {
        if (busy) return
        val s = state.value
        if (!s.loaded) return
        if (s.problems.any) {
            showErrors.value = true
            return
        }
        val d = s.draft
        val target = d.target ?: return
        val original = stored.value
        busy = true
        viewModelScope.launch {
            plan.saveGoal(
                GoalEntity(
                    id = original?.id ?: 0L,
                    name = d.name.trim(),
                    target = target,
                    targetDate = d.targetDate,
                    color = d.color,
                    archived = original?.archived ?: false,
                )
            )
            finished.send(Unit)
        }
    }

    /** Deletes the goal and its contributions at once and offers Undo; nothing asks first. */
    fun delete() {
        if (busy || stored.value == null) return
        busy = true
        val repo = plan
        val id = argId
        viewModelScope.launch {
            val deleted = repo.deleteGoal(id)
            // The undo holds the repository and the rows, never this ViewModel.
            if (deleted != null) notices.showUndo("${deleted.goal.name} deleted") { repo.restoreGoal(deleted) }
            finished.send(Unit)
        }
    }
}
