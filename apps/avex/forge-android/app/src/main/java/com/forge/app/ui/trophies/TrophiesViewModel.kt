package com.forge.app.ui.trophies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Trophies
import com.forge.app.ui.trophies.state.NearMissEntry
import com.forge.app.ui.trophies.state.TrophiesUiState
import com.forge.app.ui.trophies.state.TrophyDisplay
import com.forge.app.ui.trophies.state.TrophyFilter
import com.forge.app.ui.trophies.state.TrophySection
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives the trophies catalog screen. Reactive inputs:
 *   - persisted unlock rows (with unlock dates) — live Flow
 *   - the stats snapshot for progress hints — read once on init
 *   - selected filter chip — MutableStateFlow
 */
@HiltViewModel
class TrophiesViewModel @Inject constructor(
    private val trophyRepo: TrophyRepository
) : ViewModel() {

    private val snapshotFlow = MutableStateFlow<TrophyStatsSnapshot?>(null)
    private val filterFlow = MutableStateFlow(TrophyFilter.ALL)

    val state: StateFlow<TrophiesUiState> = combine(
        trophyRepo.observeAll(),
        trophyRepo.observeNearMisses(),
        snapshotFlow,
        filterFlow
    ) { unlocked, nearMisses, snapshot, filter ->
        if (snapshot == null) {
            TrophiesUiState(isLoading = true, totalCount = Trophies.all.size, selectedFilter = filter)
        } else {
            buildState(
                unlockedByIdToDate = unlocked.associate { it.trophyId to it.unlockedAt },
                snapshot = snapshot,
                filter = filter,
                nearMisses = nearMisses.map { nm ->
                    NearMissEntry(
                        trophyName = nm.trophyName,
                        progress = nm.progress,
                        target = nm.target,
                        recordedAt = nm.recordedAt
                    )
                }.distinctBy { it.trophyName }.take(10)
            )
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

    fun setFilter(filter: TrophyFilter) {
        filterFlow.value = filter
    }

    private fun buildState(
        unlockedByIdToDate: Map<String, Long>,
        snapshot: TrophyStatsSnapshot,
        filter: TrophyFilter,
        nearMisses: List<NearMissEntry> = emptyList()
    ): TrophiesUiState {
        val displays = Trophies.all.map { trophy ->
            val unlockedAt = unlockedByIdToDate[trophy.id]
            val progressFraction = if (unlockedAt == null) TrophyEvaluator.progressFraction(trophy.unlock, snapshot) else null
            TrophyDisplay(
                trophy = trophy,
                unlockedAt = unlockedAt,
                progressHint = if (unlockedAt == null) TrophyEvaluator.progressHint(trophy.unlock, snapshot) else null,
                progressFraction = progressFraction
            )
        }

        // Closest-trophy nudge (#55): locked trophy with the most progress
        val closestDisplay = displays
            .filter { !it.isUnlocked && (it.progressFraction ?: 0f) > 0f }
            .maxByOrNull { it.progressFraction ?: 0f }
        val closestTrophyNudge = closestDisplay?.let { d ->
            TrophyEvaluator.progressRemaining(d.trophy.unlock, snapshot)
                ?.let { remaining -> "$remaining away from ${d.trophy.name}" }
        }

        val sections = displays.groupBy { it.trophy.category }
            .map { (cat, items) -> TrophySection(category = cat, displays = items) }
        val cumulativeScore = Trophies.all
            .filter { it.id in unlockedByIdToDate }
            .sumOf { it.tier.points }
        val maxScore = Trophies.all.sumOf { it.tier.points }

        return TrophiesUiState(
            isLoading = false,
            unlockedCount = unlockedByIdToDate.size,
            totalCount = Trophies.all.size,
            sections = sections,
            selectedFilter = filter,
            closestTrophyNudge = closestTrophyNudge,
            nearMisses = nearMisses,
            cumulativeScore = cumulativeScore,
            maxScore = maxScore
        )
    }
}
