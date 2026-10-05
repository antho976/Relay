package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.core.GoalKind
import com.tally.core.MoneyFormatter
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
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
    /** The stored goal's first contribution, where the preview's pace tick starts; null for a new goal or none. */
    val firstDate: LocalDate? = null,
    val monthsLeft: Int? = null,
    val problems: GoalProblems = GoalProblems(),
    /** True once a save was tried with problems: the form then says what is missing. */
    val showErrors: Boolean = false,
    /** The open accounts a balance or invest goal can read. */
    val accounts: List<AccountBalance> = emptyList(),
    /** What a balance goal's pick reads now: the account's balance, or the net of every open one. */
    val balanceNow: Long = 0,
) {
    val isNew: Boolean get() = !stored
}

@HiltViewModel
class GoalEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    private val ledger: LedgerRepository,
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

    private data class Flags(val ready: Boolean, val errors: Boolean)

    private val flags = combine(ready, showErrors) { r, e -> Flags(r, e) }

    val state: StateFlow<GoalEditState> = combine(draft, stored, plan.goals(), ledger.balances(), flags) { d, st, goals, balances, f ->
        val row = if (st == null) null else goals.firstOrNull { it.goal.id == st.id }
        GoalEditState(
            draft = d,
            today = today,
            loaded = f.ready,
            stored = st != null,
            saved = row?.saved ?: 0L,
            firstDate = row?.firstDate,
            monthsLeft = d.targetDate?.let { monthsUntil(today, it) },
            problems = goalProblems(d),
            showErrors = f.errors,
            accounts = balances.filter { !it.archived || it.id == d.accountId },
            balanceNow = reading(d.accountId, balances),
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        GoalEditState(draft = draft.value, today = today),
    )

    /** A balance goal's reading: the picked account's balance, or the net of every open account. */
    private fun reading(accountId: Long?, balances: List<AccountBalance>): Long =
        accountId?.let { id -> balances.firstOrNull { it.id == id }?.balance } ?: netWorth(balances)

    init {
        viewModelScope.launch {
            // A draft kept before the process died wins over the stored row: it is what was typed.
            val kept = savedStateHandle.savedDraft(SavedGoal.serializer())?.toDraft()
            val s = settings.current()
            val money = MoneyFormatter(s.currency, Locale.getDefault())
            val goal = if (argId != 0L) plan.goal(argId) else null
            if (goal != null) {
                stored.value = goal
                draft.value = kept ?: GoalDraft(
                    id = goal.id,
                    name = goal.name,
                    targetText = if (goal.target > 0L || goal.kind == GoalKind.BALANCE) money.formatInput(goal.target) else "",
                    target = goal.target,
                    targetDate = goal.targetDate,
                    color = goal.color,
                    archived = goal.archived,
                    kind = goal.kind,
                    accountId = goal.accountId,
                    percent = if (goal.percent > 0) goal.percent else 10,
                    byShare = goal.percent > 0 || goal.target <= 0L,
                )
            } else {
                draft.value = (kept ?: draft.value).copy(id = 0L)
            }
            ready.value = true
            draft.collect { savedStateHandle.keepDraft(SavedGoal.serializer(), SavedGoal.of(it)) }
        }
    }

    fun setName(name: String) = draft.update { it.copy(name = name) }

    /** [target] is [text] read by the screen's money formatter; null when it is not an amount. */
    fun setTarget(text: String, target: Long?) = draft.update { it.copy(targetText = text, target = target) }

    fun setDate(date: LocalDate?) = draft.update { it.copy(targetDate = date) }

    fun setColor(color: Int) = draft.update { it.copy(color = color) }

    /** The kind is picked once, on a new goal: a pot's contributions mean nothing to another kind. */
    fun setKind(kind: GoalKind) {
        if (stored.value != null) return
        draft.update { it.copy(kind = kind, accountId = null) }
    }

    fun setAccount(id: Long?) = draft.update { it.copy(accountId = id) }

    fun setPercent(percent: Int) = draft.update { it.copy(percent = percent.coerceIn(1, 100)) }

    fun setByShare(byShare: Boolean) = draft.update { it.copy(byShare = byShare) }

    fun save() {
        if (busy) return
        val s = state.value
        if (!s.loaded) return
        if (s.problems.any) {
            showErrors.value = true
            return
        }
        val d = s.draft
        val original = stored.value
        busy = true
        viewModelScope.launch {
            val balances = ledger.balances().first()
            val monthly = d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE
            // A balance goal's pace starts the day it is set, from what its pick reads then; an edit
            // keeps that start unless the pick changed under it.
            val keepStart = original != null && original.kind == GoalKind.BALANCE && original.accountId == d.accountId
            plan.saveGoal(
                GoalEntity(
                    id = original?.id ?: 0L,
                    name = d.name.trim(),
                    target = when {
                        monthly && d.byShare -> 0L
                        else -> d.target ?: 0L
                    },
                    targetDate = if (monthly) null else d.targetDate,
                    color = d.color,
                    archived = original?.archived ?: false,
                    kind = d.kind,
                    accountId = if (d.kind == GoalKind.SAVINGS || d.kind == GoalKind.SAVE) null else d.accountId,
                    percent = if (monthly && d.byShare) d.percent else 0,
                    startDate = when {
                        d.kind != GoalKind.BALANCE -> null
                        keepStart -> original?.startDate ?: today
                        else -> today
                    },
                    startAmount = when {
                        d.kind != GoalKind.BALANCE -> 0L
                        keepStart -> original?.startAmount ?: 0L
                        else -> reading(d.accountId, balances)
                    },
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
