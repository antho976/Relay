package com.tally.app.ui.categories

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.CategoryUse
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.IconHints
import com.tally.core.MoneyFormatter
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** The editor's fields as picked. Serializable so the editor keeps it in its SavedStateHandle. */
@Immutable
@Serializable
data class CategoryDraft(
    val name: String = "",
    val icon: String = "dots",
    val color: Int = 10,
    val archived: Boolean = false,
    /** True once the owner picked the icon; until then a new category's name suggests one. */
    val iconChosen: Boolean = false,
)

/** Everything the category editor draws. */
@Immutable
data class CategoryEditState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val kind: CategoryKind = CategoryKind.EXPENSE,
    val draft: CategoryDraft = CategoryDraft(),
    val loaded: Boolean = false,
    /** True when the category exists in the database (editing, not new). */
    val stored: Boolean = false,
    /** The name as saved, for the delete sheet (the draft may have been retyped). */
    val storedName: String = "",
    val entryCount: Int = 0,
    /** This period's money filed under the category. */
    val periodTotal: Long = 0,
    /** This period's money across the whole kind, the category's share is of this. */
    val kindTotal: Long = 0,
    /** Why the name cannot be saved, or null. */
    val nameProblem: String? = null,
    /** True once a save was tried with a problem: a missing name is then said too. */
    val showErrors: Boolean = false,
    /** Where the entries and bills can go when the category is deleted. */
    val targets: List<MoveTarget> = emptyList(),
    /** What the delete touches, counted afresh, while the move sheet is open; null when it is closed. */
    val moving: CategoryUse? = null,
) {
    val isNew: Boolean get() = !stored

    /** A clash shows as you type; a missing name only once a save was tried. */
    val shownNameProblem: String? get() = nameProblem?.takeIf { it != MISSING_NAME || showErrors }

    val share: Float get() = if (kindTotal > 0) (periodTotal.toDouble() / kindTotal).toFloat() else 0f
}

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class CategoryEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val argId: Long = savedStateHandle.get<Long>(Args.ID) ?: 0L
    private val argKind: CategoryKind = savedStateHandle.get<String>(Args.KIND)
        ?.let { name -> CategoryKind.entries.firstOrNull { it.name == name } }
        ?: CategoryKind.EXPENSE
    private val today: LocalDate = clock.today()

    private data class Flags(val loaded: Boolean = false, val showErrors: Boolean = false, val moving: CategoryUse? = null)

    private data class PeriodTotals(val period: BudgetPeriod, val totals: List<CategoryTotal>)

    private val draft = MutableStateFlow(CategoryDraft())
    private val stored = MutableStateFlow<CategoryEntity?>(null)
    private val kind = MutableStateFlow(argKind)
    private val flags = MutableStateFlow(Flags())
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or delete; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    private val totals: Flow<PeriodTotals> = combine(settings.settings.map { it.periodFor(today) }, kind) { p, k -> p to k }
        .distinctUntilChanged()
        .flatMapLatest { (p, k) -> ledger.byCategory(k.txType(), p.start, p.endExclusive).map { PeriodTotals(p, it) } }

    val state: StateFlow<CategoryEditState> =
        combine(draft, stored, ledger.categoriesWithCounts(), totals, flags) { d, st, all, t, f ->
            val k = st?.kind ?: argKind
            val selfId = st?.id ?: 0L
            CategoryEditState(
                today = today,
                period = t.period,
                kind = k,
                draft = d,
                loaded = f.loaded,
                stored = st != null,
                storedName = st?.name.orEmpty(),
                entryCount = if (st == null) 0 else all.firstOrNull { it.category.id == selfId }?.entryCount ?: 0,
                periodTotal = if (st == null) 0L else t.totals.filter { it.categoryId == selfId }.sumOf { it.total },
                kindTotal = t.totals.sumOf { it.total },
                nameProblem = categoryNameProblem(d.name, k, selfId, all.map { it.category }),
                showErrors = f.showErrors,
                targets = moveTargets(all, k, selfId),
                moving = f.moving,
            )
        }.stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            CategoryEditState(today = today, period = BudgetPeriod.containing(today), kind = argKind),
        )

    init {
        viewModelScope.launch {
            // A draft kept before the process died wins over the stored row: it is what was picked.
            val kept = savedStateHandle.savedDraft(CategoryDraft.serializer())
            val existing = if (argId != 0L) ledger.category(argId) else null
            if (existing != null) {
                draft.value = kept ?: CategoryDraft(existing.name, existing.icon, existing.color, existing.archived, iconChosen = true)
                kind.value = existing.kind
                stored.value = existing
            } else {
                draft.value = kept ?: run {
                    val siblings = ledger.categories().first().filter { it.kind == argKind }
                    CategoryDraft(color = suggestColor(siblings.map { it.color }))
                }
            }
            flags.update { it.copy(loaded = true) }
            draft.collect { savedStateHandle.keepDraft(CategoryDraft.serializer(), it) }
        }
    }

    /** A new category's name picks its icon ("Coffee" the cup) until the owner picks one. */
    fun setName(name: String) = draft.update { d ->
        val hint = if (d.iconChosen || stored.value != null) null else IconHints.suggest(name)
        d.copy(name = name, icon = hint ?: if (d.iconChosen || stored.value != null) d.icon else "dots")
    }

    fun setIcon(icon: String) = draft.update { it.copy(icon = icon, iconChosen = true) }

    fun setColor(color: Int) = draft.update { it.copy(color = color) }

    fun setArchived(archived: Boolean) = draft.update { it.copy(archived = archived) }

    fun save() {
        if (busy) return
        val s = state.value
        if (!s.loaded) return
        if (s.nameProblem != null) {
            flags.update { it.copy(showErrors = true) }
            return
        }
        val d = s.draft
        val original = stored.value
        busy = true
        viewModelScope.launch {
            ledger.saveCategory(
                CategoryEntity(
                    id = original?.id ?: 0L,
                    name = d.name.trim(),
                    kind = s.kind,
                    color = d.color,
                    icon = d.icon,
                    archived = d.archived,
                    sortOrder = original?.sortOrder ?: 0,
                )
            )
            finished.send(Unit)
        }
    }

    /**
     * Counts afresh what the delete touches: entries, bills and the budget. With entries or bills
     * filed under it, the sheet asks where they move first, since moving them cannot be undone.
     * With neither (a budget alone cannot move) it goes at once, with Undo.
     */
    fun requestDelete() {
        val original = stored.value ?: return
        if (busy) return
        viewModelScope.launch {
            val use = ledger.categoryUse(original.id)
            if (use.hasMovable) flags.update { it.copy(moving = use) } else remove(original, null, use)
        }
    }

    fun dismissMove() = flags.update { it.copy(moving = null) }

    /** [moveTo] null leaves the entries and bills uncategorized. */
    fun delete(moveTo: Long?) {
        val original = stored.value ?: return
        val use = flags.value.moving ?: return
        flags.update { it.copy(moving = null) }
        viewModelScope.launch { remove(original, moveTo, use) }
    }

    private suspend fun remove(original: CategoryEntity, moveTo: Long?, use: CategoryUse) {
        if (busy) return
        busy = true
        val targetName = moveTo?.let { id -> state.value.targets.firstOrNull { it.id == id }?.name }
        val money = MoneyFormatter(settings.current().currency, Locale.getDefault())
        val line = deletedLine(original.name, use, use.budget?.let { money.format(it) }, targetName)
        val repo = ledger
        repo.deleteCategory(original.id, moveTo)
        if (use.hasMovable) {
            notices.show(line)
        } else {
            // Nothing moved or lost its category, so the delete undoes exactly. The undo holds
            // the repository and the row, never this ViewModel.
            notices.showUndo(line) { repo.restoreCategory(original, use.budget) }
        }
        finished.send(Unit)
    }
}
