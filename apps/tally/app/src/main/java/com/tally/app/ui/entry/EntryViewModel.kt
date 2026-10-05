package com.tally.app.ui.entry

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.BudgetRow
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.QuickPick
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.Frequency
import com.tally.core.MoneyFormatter
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapLatest
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** Everything the entry editor draws. Lookups and readings are resolved here, not in composition. */
@Immutable
data class EntryState(
    /** The draft as it will be saved: defaults filled, stale ids dropped. */
    val draft: EntryDraft,
    val today: LocalDate,
    /** The budget period holding the draft's date. */
    val period: BudgetPeriod,
    /** False until the stored entry (when editing) and the lists have arrived. */
    val loaded: Boolean = false,
    /** The grid for the draft's type. */
    val categories: List<CategoryEntity> = emptyList(),
    val accounts: List<AccountBalance> = emptyList(),
    /** Notes used before that start with what is typed. */
    val suggestions: List<String> = emptyList(),
    val category: CategoryEntity? = null,
    val account: AccountBalance? = null,
    val toAccount: AccountBalance? = null,
    /** [account]'s balance once this is saved. */
    val accountAfter: Long? = null,
    val toAccountAfter: Long? = null,
    /** The picked category over [period], this entry included. */
    val month: MonthReading = MonthReading(),
    /** The picked expense category's monthly budget, when it has one. */
    val budget: Long? = null,
    /** Where a repeat would post next. */
    val repeatNext: LocalDate = today,
    /** What is logged most often for this type, one tap from filled. Empty once there is an amount. */
    val quickPicks: List<QuickPick> = emptyList(),
    /** What the month's budget has left once this expense is in; null without a budget, or for income or a transfer. */
    val leftAfter: Long? = null,
    /** The keypad's decimals, so a quick pick's amount types as the owner would type it. */
    val digits: Int = 2,
) {
    val isNew: Boolean get() = draft.id == 0L
    val problem: String? get() = validate(draft)
    val canSave: Boolean get() = problem == null
    val sameAccount: Boolean
        get() = draft.type == TxType.TRANSFER && draft.accountId != null && draft.accountId == draft.toAccountId
}

/**
 * The add / edit entry editor, the app's hot path. The draft lives here in one [MutableStateFlow];
 * the state folds it with the lists and readings. A new entry opens on the default account,
 * today, and the category it was opened for (if any).
 */
/** How far back the grid's order and the quick picks look. */
internal const val USE_DAYS = 90L

/** How many quick picks the editor offers. */
internal const val QUICK_PICKS = 6

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class EntryViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val ledger: LedgerRepository,
    private val plan: PlanRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    private val clock: Clock,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val argType: TxType = savedStateHandle.get<String>(Args.TYPE)
        ?.let { name -> TxType.entries.firstOrNull { it.name == name } } ?: TxType.EXPENSE
    private val argAccount: Long = savedStateHandle.get<Long>(Args.ACCOUNT) ?: 0L
    private val argCategory: Long = savedStateHandle.get<Long>(Args.CATEGORY) ?: 0L

    private val today: LocalDate = clock.today()

    private val draft = MutableStateFlow(
        EntryDraft(
            id = argId,
            type = argType,
            date = today,
            accountId = argAccount.takeIf { it != 0L },
            categoryId = argCategory.takeIf { it != 0L },
            categoryChosen = argCategory != 0L,
        )
    )

    /** The entry as stored, when editing: the balances and totals already include it. */
    private val original = MutableStateFlow<TransactionEntity?>(null)
    private val ready = MutableStateFlow(false)

    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save, delete or duplicate; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit: a double tap on Save must not log the purchase twice. */
    private var busy = false

    private data class MonthKey(val type: TxType, val period: BudgetPeriod)
    private data class MonthTotals(val key: MonthKey, val byCategory: Map<Long, CategoryTotal>)

    private val monthTotals: Flow<MonthTotals> =
        combine(draft, settings.settings) { d, s -> MonthKey(d.type, s.periodFor(d.date)) }
            .distinctUntilChanged()
            .flatMapLatest { key ->
                if (key.type == TxType.TRANSFER) {
                    flowOf(MonthTotals(key, emptyMap()))
                } else {
                    ledger.byCategory(key.type, key.period.start, key.period.endExclusive).map { rows ->
                        MonthTotals(key, rows.mapNotNull { row -> row.categoryId?.let { it to row } }.toMap())
                    }
                }
            }

    private val suggestions: Flow<List<String>> = draft
        .map { it.note.trim() }
        .distinctUntilChanged()
        .mapLatest { prefix -> if (prefix.isEmpty()) emptyList<String>() else ledger.noteSuggestions(prefix) }

    private data class Inputs(
        val draft: EntryDraft,
        val original: TransactionEntity?,
        val categories: List<CategoryEntity>,
        val accounts: List<AccountBalance>,
        val defaultAccountId: Long,
    )

    private val inputs: Flow<Inputs> = combine(draft, original, ledger.categories(), ledger.balances(), settings.settings) { d, o, c, a, s ->
        Inputs(d, o, c, a, s.defaultAccountId)
    }

    /** How often each category was used in the last [USE_DAYS] days, for the grid's order, and the quick picks. */
    private data class Habits(val uses: Map<Long, Int> = emptyMap(), val picks: List<QuickPick> = emptyList(), val periodSpent: Long = 0L)

    private val habits: Flow<Habits> = combine(draft, settings.settings) { d, s -> d.type to s.periodFor(d.date) }
        .distinctUntilChanged()
        .flatMapLatest { (type, period) ->
            if (type == TxType.TRANSFER) {
                flowOf(Habits())
            } else {
                val since = today.minusDays(USE_DAYS)
                combine(
                    ledger.byCategory(type, since, today.plusDays(1)),
                    ledger.quickPicks(type, since, QUICK_PICKS),
                    ledger.typeTotals(period.start, period.endExclusive),
                ) { uses, picks, totals ->
                    Habits(
                        uses = uses.mapNotNull { u -> u.categoryId?.let { it to u.count } }.toMap(),
                        picks = picks,
                        periodSpent = totals.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L,
                    )
                }
            }
        }

    private data class Frame(val inputs: Inputs, val habits: Habits)

    private val frame: Flow<Frame> = combine(inputs, habits) { i, h -> Frame(i, h) }

    val state: StateFlow<EntryState> = combine(frame, monthTotals, plan.budgets(), suggestions, ready) { f, m, budgets, suggested, isReady ->
        buildState(f.inputs, m, budgets, suggested, isReady, f.habits)
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        EntryState(draft = draft.value, today = today, period = BudgetPeriod.containing(today)),
    )

    private fun buildState(
        i: Inputs,
        m: MonthTotals,
        budgets: List<BudgetRow>,
        suggested: List<String>,
        isReady: Boolean,
        h: Habits,
    ): EntryState {
        val kindCategories = byUse(categoriesFor(i.draft.type, i.categories, i.draft.categoryId), h.uses)
        val d = resolveDraft(i.draft, kindCategories, i.accounts, i.defaultAccountId)
        val account = i.accounts.firstOrNull { it.id == d.accountId }
        val toAccount = if (d.type == TxType.TRANSFER) i.accounts.firstOrNull { it.id == d.toAccountId } else null
        val category = if (d.type == TxType.TRANSFER) null else kindCategories.firstOrNull { it.id == d.categoryId }
        // The totals can trail a type flip by one emission; a reading of the wrong type is worse than none.
        val month = if (category != null && m.key.type == d.type) {
            val stored = m.byCategory[category.id]
            categoryMonth(stored?.total ?: 0L, stored?.count ?: 0, i.original, d, m.key.period)
        } else {
            MonthReading()
        }
        val budget = if (d.type == TxType.EXPENSE && category != null) {
            budgets.firstOrNull { it.categoryId == category.id }?.amount
        } else null
        val note = d.note.trim()
        return EntryState(
            draft = d,
            today = today,
            period = m.key.period,
            loaded = isReady,
            categories = kindCategories,
            accounts = accountChoices(i.accounts, d),
            suggestions = suggested.filterNot { it.equals(note, ignoreCase = true) },
            category = category,
            account = account,
            toAccount = toAccount,
            accountAfter = account?.let { balanceAfter(it.id, it.balance, i.original, d) },
            toAccountAfter = toAccount?.let { balanceAfter(it.id, it.balance, i.original, d) },
            month = month,
            budget = budget,
            repeatNext = firstRepeatDate(d.date, d.frequency, today),
            quickPicks = if (d.amount.minor == 0L && d.note.isBlank()) h.picks else emptyList(),
            leftAfter = if (d.type == TxType.EXPENSE) {
                budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }?.amount?.let { overall ->
                    leftAfter(overall, h.periodSpent, i.original, d, m.key.period)
                }
            } else null,
            digits = d.amount.fractionDigits,
        )
    }

    init {
        viewModelScope.launch {
            // A draft kept before the process died wins over the stored entry: it is what was typed.
            val kept = savedStateHandle.savedDraft(SavedEntry.serializer())?.toDraft()
            val s = settings.current()
            // The keypad works in the owner's currency's minor units (0 digits for JPY).
            val digits = MoneyFormatter(s.currency, Locale.getDefault()).fractionDigits
            val stored = if (argId != 0L) ledger.transaction(argId) else null
            if (stored != null) {
                original.value = stored
                draft.value = kept ?: EntryDraft(
                    id = stored.id,
                    type = stored.type,
                    amount = AmountInput.of(stored.amount, digits),
                    date = stored.date,
                    accountId = stored.accountId,
                    toAccountId = stored.toAccountId,
                    categoryId = stored.categoryId,
                    categoryChosen = stored.categoryId != null,
                    note = stored.note,
                    recurringId = stored.recurringId,
                )
            } else {
                // A new entry, or one deleted while its link was open: either way this writes a new row.
                if (kept != null) draft.value = kept
                draft.update { it.copy(id = 0L, amount = it.amount.copy(fractionDigits = digits)) }
            }
            ready.value = true
            draft.collect { savedStateHandle.keepDraft(SavedEntry.serializer(), SavedEntry.of(it)) }
        }
    }

    // ── Edits ────────────────────────────────────────────────────────────────

    fun setType(type: TxType) = draft.update { it.copy(type = type) }

    fun press(key: KeypadKey) = draft.update { it.copy(amount = it.amount.press(key)) }

    fun pickCategory(id: Long) = draft.update { it.copy(categoryId = id, categoryChosen = true) }

    fun pickAccount(id: Long) = draft.update { it.copy(accountId = id) }

    fun pickToAccount(id: Long) = draft.update { it.copy(toAccountId = id) }

    fun setDate(date: LocalDate) = draft.update { it.copy(date = date) }

    fun setNote(note: String) = draft.update { it.copy(note = note) }

    fun setRepeat(on: Boolean) = draft.update { it.copy(repeat = on) }

    fun setFrequency(frequency: Frequency) = draft.update { it.copy(frequency = frequency) }

    /** One of the most-logged entries: its note, its category and its last amount, all at once. */
    fun pickQuick(pick: QuickPick) = draft.update {
        it.copy(
            note = pick.note,
            categoryId = pick.categoryId,
            categoryChosen = true,
            amount = AmountInput.of(pick.lastAmount, it.amount.fractionDigits),
        )
    }

    /**
     * A note used before. Unless the person already picked a category, the one last used with this
     * note comes with it, so "Metro" lands on Groceries by itself.
     */
    fun pickSuggestion(note: String) {
        draft.update { it.copy(note = note) }
        val current = state.value.draft
        val kind = kindOf(current.type) ?: return
        if (current.categoryChosen) return
        viewModelScope.launch {
            val id = ledger.lastCategoryForNote(note) ?: return@launch
            val category = ledger.category(id) ?: return@launch
            if (category.kind != kind || category.archived) return@launch
            if (state.value.draft.categoryChosen) return@launch
            draft.update { d -> if (d.type != current.type) d else d.copy(categoryId = id, categoryChosen = false) }
        }
    }

    // ── Writes ───────────────────────────────────────────────────────────────

    /**
     * Writes the entry. A repeat first makes its bill, starting strictly after this entry's date so
     * the bill never posts the day this entry already covers, then links the entry to it.
     */
    fun save() = write(andNew = false)

    /**
     * Saves and stays for the next one: the form empties but keeps its type, account and day, so
     * a stack of receipts is one screen.
     */
    fun saveAndNew() = write(andNew = true)

    private fun write(andNew: Boolean) {
        if (busy) return
        val s = state.value
        val d = s.draft
        if (validate(d) != null) return
        if (andNew && d.id != 0L) return
        busy = true
        val categoryName = s.category?.name
        val stored = original.value
        viewModelScope.launch {
            val from = d.accountId ?: return@launch
            var recurringId = d.recurringId
            if (d.id == 0L && d.repeat) {
                val next = firstRepeatDate(d.date, d.frequency, clock.today())
                val bill = RecurringEntity(
                    name = repeatName(d.note, categoryName, d.type),
                    type = d.type,
                    amount = d.amount.minor,
                    accountId = from,
                    toAccountId = if (d.type == TxType.TRANSFER) d.toAccountId else null,
                    categoryId = if (d.type == TxType.TRANSFER) null else d.categoryId,
                    frequency = d.frequency,
                    anchorDate = d.date,
                    nextDate = next,
                    autoPost = true,
                )
                val billId = plan.saveRecurring(bill)
                // saveRecurring starts a new bill on or after today, which for an entry dated today
                // (or ahead) is the entry's own date. Move it past the entry so nothing posts twice.
                val saved = plan.recurringItem(billId)
                if (saved != null && saved.nextDate != next) plan.saveRecurring(saved.copy(nextDate = next))
                recurringId = billId
            }
            val row = d.toEntity(stored, recurringId) ?: return@launch
            ledger.save(row)
            if (andNew) {
                draft.update { next ->
                    EntryDraft(
                        type = next.type,
                        date = next.date,
                        accountId = next.accountId,
                        toAccountId = next.toAccountId,
                        amount = AmountInput(fractionDigits = next.amount.fractionDigits),
                    )
                }
                notices.show(savedLine(row.type, s.category?.name) + " · ready for the next")
                busy = false
            } else {
                finished.send(Unit)
            }
        }
    }

    /** Deletes at once and offers Undo; nothing asks first. */
    fun delete() {
        val id = draft.value.id
        if (busy || id == 0L) return
        busy = true
        val repo = ledger
        viewModelScope.launch {
            val row = repo.delete(id)
            // The undo holds the repository and the row, never this ViewModel.
            if (row != null) notices.showUndo(undoLine(row)) { repo.restore(row) }
            finished.send(Unit)
        }
    }

    /** Names the entry by its category, so the snackbar says which one its Undo brings back. */
    private suspend fun undoLine(row: TransactionEntity): String {
        if (row.type == TxType.TRANSFER) return "Transfer deleted"
        val category = row.categoryId?.let { ledger.category(it) }?.name
        return if (category != null) "$category entry deleted" else "Entry deleted"
    }

    /** The stored entry again, dated today, unlinked from any bill. */
    fun duplicate() {
        val stored = original.value ?: return
        if (busy || stored.amount <= 0L) return
        busy = true
        viewModelScope.launch {
            ledger.save(stored.copy(id = 0L, date = clock.today(), recurringId = null, createdAt = 0L))
            notices.show("Copied to today")
            finished.send(Unit)
        }
    }
}
