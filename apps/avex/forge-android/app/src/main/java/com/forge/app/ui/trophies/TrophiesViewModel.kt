package com.forge.app.ui.trophies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Trophies
import com.forge.app.ui.trophies.state.TrophiesUiState
import com.forge.app.ui.trophies.state.TrophyDisplay
import com.forge.app.ui.trophies.state.TrophySection
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives the trophies catalog screen. Two reactive inputs:
 *   - persisted unlock rows (with unlock dates) — live Flow
 *   - the stats snapshot for progress hints — read once on init (rules can't tick mid-view)
 *
 * Frozen-per-open progress is intentional: the user usually opens this screen between
 * sessions, and live updates would just mean spending 8 DAO calls per emission for an
 * imperceptible label change.
 */
@HiltViewModel
class TrophiesViewModel @Inject constructor(
    private val trophyRepo: TrophyRepository
) : ViewModel() {

    private val snapshotFlow = MutableStateFlow<TrophyStatsSnapshot?>(null)

    val state: StateFlow<TrophiesUiState> = combine(
        trophyRepo.observeAll(),
        snapshotFlow
    ) { unlocked, snapshot ->
        if (snapshot == null) {
            TrophiesUiState(isLoading = true, totalCount = Trophies.all.size)
        } else {
            buildState(unlockedByIdToDate = unlocked.associate { it.trophyId to it.unlockedAt }, snapshot = snapshot)
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = TrophiesUiState(totalCount = Trophies.all.size)
    )

    init {
        viewModelScope.launch {
            snapshotFlow.value = trophyRepo.snapshot()
        }
    }

    private fun buildState(
        unlockedByIdToDate: Map<String, Long>,
        snapshot: TrophyStatsSnapshot
    ): TrophiesUiState {
        val displays = Trophies.all.map { trophy ->
            val unlockedAt = unlockedByIdToDate[trophy.id]
            TrophyDisplay(
                trophy = trophy,
                unlockedAt = unlockedAt,
                progressHint = if (unlockedAt == null) {
                    TrophyEvaluator.progressHint(trophy.unlock, snapshot)
                } else null
            )
        }
        val sections = displays.groupBy { it.trophy.category }
            .map { (cat, items) -> TrophySection(category = cat, displays = items) }
        return TrophiesUiState(
            isLoading = false,
            unlockedCount = unlockedByIdToDate.size,
            totalCount = Trophies.all.size,
            sections = sections
        )
    }
}
