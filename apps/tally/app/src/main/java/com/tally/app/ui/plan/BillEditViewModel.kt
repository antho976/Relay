package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.core.Frequency
import com.tally.core.MoneyFormatter
import com.tally.core.TxType
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

/** Everything the bill editor draws. Lookups and the preview are resolved here. */
@Immutable
data class BillEditState(
    /** The draft as it will be saved: defaults filled, stale picks dropped. */
    val draft: BillDraft,
    val today: LocalDate,
    val loaded: Boolean = false,
    /** True when the bill exists in the database (editing, not new). */
    val stored: Boolean = false,
    val accounts: List<AccountEntity> = emptyList(),
    /** The categories of the draft's kind. */
    val categories: List<CategoryEntity> = emptyList(),
    /** The picked account's name, and the receiving one's for a transfer. */
    val accountName: String? = null,
    val toAccountName: String? = null,
    /** The picked category, when it is one of [categories]. */
    val category: CategoryEntity? = null,
    /** The next three dates it would post on; empty while it is paused, since then it posts nothing. */
    val preview: List<LocalDate> = emptyList(),
    /** The amount in an average month. */
    val perMonth: Long = 0,
    val problems: BillProblems = BillProblems(),
    /** True once a save was tried with problems: the form then says what is missing. */
    val showErrors: Boolean = false,
) {
    val isNew: Boolean get() = !stored
}

/**
 * The bill editor. The draft lives in one [MutableStateFlow] and folds with the lists; a new bill
 * opens on the default account, monthly, starting today, posting itself.
 */
@HiltViewModel
class BillEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val today: LocalDate = clock.today()

    /** The bill as stored when the editor opened, and the latest date already entered against it. */
    private data class Stored(val item: RecurringEntity, val lastPosted: LocalDate?)

    private val draft = MutableStateFlow(BillDraft(id = argId, anchorDate = today))
    private val stored = MutableStateFlow<Stored?>(null)
    private val ready = MutableStateFlow(false)
    private val showErrors = MutableStateFlow(false)
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or delete; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    private data class Lists(val accounts: List<AccountEntity>, val categories: List<CategoryEntity>, val defaultAccountId: Long)

    private val lists: Flow<Lists> = combine(ledger.accounts(), ledger.categories(), settings.settings) { a, c, s ->
        Lists(a, c, s.defaultAccountId)
    }

    val state: StateFlow<BillEditState> = combine(draft, stored, lists, ready, showErrors) { d, st, l, isReady, errors ->
        val resolved = resolveBill(d, l.accounts, l.categories, l.defaultAccountId)
        val categories = billCategories(l.categories, resolved.type, resolved.categoryId)
        BillEditState(
            draft = resolved,
            today = today,
            loaded = isReady,
            stored = st != null,
            accounts = billAccounts(l.accounts, resolved),
            categories = categories,
            accountName = l.accounts.firstOrNull { it.id == resolved.accountId }?.name,
            toAccountName = if (resolved.type == TxType.TRANSFER) l.accounts.firstOrNull { it.id == resolved.toAccountId }?.name else null,
            category = categories.firstOrNull { it.id == resolved.categoryId },
            preview = nextDates(resolved, st?.item, today, lastPosted = st?.lastPosted),
            perMonth = perMonth(resolved.amount ?: 0L, resolved.frequency, resolved.interval),
            problems = billProblems(resolved),
            showErrors = errors,
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        BillEditState(draft = draft.value, today = today),
    )

    init {
        viewModelScope.launch {
            // A draft kept before the process died wins over the stored row: it is what was typed.
            val kept = savedStateHandle.savedDraft(SavedBill.serializer())?.toDraft()
            val s = settings.current()
            val money = MoneyFormatter(s.currency, Locale.getDefault())
            val item = if (argId != 0L) plan.recurringItem(argId) else null
            if (item != null) {
                stored.value = Stored(item, plan.lastPosted(item.id))
                draft.value = kept ?: BillDraft(
                    id = item.id,
                    name = item.name,
                    type = item.type,
                    amountText = money.formatInput(item.amount),
                    amount = item.amount,
                    accountId = item.accountId,
                    toAccountId = item.toAccountId,
                    categoryId = item.categoryId,
                    frequency = item.frequency,
                    interval = item.interval,
                    anchorDate = item.anchorDate,
                    autoPost = item.autoPost,
                    active = item.active,
                )
            } else {
                // A new bill, or one deleted while its link was open: either way this writes a new row.
                if (kept != null) draft.value = kept
                draft.update { it.copy(id = 0L) }
            }
            ready.value = true
            draft.collect { savedStateHandle.keepDraft(SavedBill.serializer(), SavedBill.of(it)) }
        }
    }

    // ── Edits ────────────────────────────────────────────────────────────────

    fun setName(name: String) = draft.update { it.copy(name = name) }

    fun setType(type: TxType) = draft.update { it.copy(type = type) }

    /** [amount] is [text] read by the screen's money formatter; null when it is not an amount. */
    fun setAmount(text: String, amount: Long?) = draft.update { it.copy(amountText = text, amount = amount) }

    fun pickAccount(id: Long) = draft.update { it.copy(accountId = id) }

    fun pickToAccount(id: Long) = draft.update { it.copy(toAccountId = id) }

    /** A second tap on the picked category clears it: a bill can post uncategorized. */
    fun pickCategory(id: Long) = draft.update { d ->
        val current = state.value.draft.categoryId
        d.copy(categoryId = if (current == id) null else id)
    }

    fun setFrequency(frequency: Frequency) = draft.update { it.copy(frequency = frequency) }

    fun stepInterval(delta: Int) = draft.update { it.copy(interval = (it.interval + delta).coerceIn(1, MAX_INTERVAL)) }

    fun setAnchor(date: LocalDate) = draft.update { it.copy(anchorDate = date) }

    fun setAutoPost(on: Boolean) = draft.update { it.copy(autoPost = on) }

    fun setActive(on: Boolean) = draft.update { it.copy(active = on) }

    // ── Writes ───────────────────────────────────────────────────────────────

    fun save() {
        if (busy) return
        val s = state.value
        if (!s.loaded) return
        if (s.problems.any) {
            showErrors.value = true
            return
        }
        val d = s.draft
        val amount = d.amount ?: return
        val accountId = d.accountId ?: return
        val original = stored.value?.item
        busy = true
        viewModelScope.launch {
            val item = RecurringEntity(
                id = original?.id ?: 0L,
                name = d.name.trim(),
                type = d.type,
                amount = amount,
                accountId = accountId,
                toAccountId = if (d.type == TxType.TRANSFER) d.toAccountId else null,
                categoryId = if (d.type == TxType.TRANSFER) null else d.categoryId,
                frequency = d.frequency,
                interval = d.interval,
                anchorDate = d.anchorDate,
                // Only a new bill's earliest start. For a stored one the repository reads the row
                // fresh and keeps its schedule, so this editor's copy, which the poster may have
                // moved past while it was open, is never written back.
                nextDate = d.anchorDate,
                endDate = original?.endDate,
                autoPost = d.autoPost,
                // Pausing and resuming ride along: the repository starts a resumed bill from today
                // instead of owing the dates it was paused for.
                active = d.active,
            )
            plan.saveRecurring(item)
            finished.send(Unit)
        }
    }

    /** Deletes at once and offers Undo; nothing asks first. Entries it already posted stay. */
    fun delete() {
        if (busy || stored.value == null) return
        busy = true
        val repo = plan
        val id = argId
        viewModelScope.launch {
            val row = repo.deleteRecurring(id)
            // The undo holds the repository and the row, never this ViewModel.
            if (row != null) notices.showUndo("${row.name} deleted") { repo.restoreRecurring(row) }
            finished.send(Unit)
        }
    }
}
