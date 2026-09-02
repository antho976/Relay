package com.forge.app.ui.trophies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
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
    private val trophyRepo: TrophyRepository,
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    private val snapshotFlow = MutableStateFlow<TrophyStatsSnapshot?>(null)
    private val filterFlow = MutableStateFlow(TrophyFilter.ALL)

    val state: StateFlow<TrophiesUiState> = combine(
        trophyRepo.observeAll(),
        trophyRepo.observeNearMisses(),
        snapshotFlow,
        filterFlow,
        settingsRepo.weightUnit
    ) { unlocked, nearMisses, snapshot, filter, weightUnit ->
        trophiesStateFor(
            unlockedByIdToDate = unlocked.associate { it.trophyId to it.unlockedAt },
            nearMisses = nearMisses.map { nm ->
                NearMissEntry(
                    trophyName = nm.trophyName,
                    progress = nm.progress,
                    target = nm.target,
                    recordedAt = nm.recordedAt
                )
            }.distinctBy { it.trophyName }.take(10),
            snapshot = snapshot,
            filter = filter,
            weightUnit = weightUnit
        )
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
}

/**
 * The catalog state for one set of inputs — a pure function of them, so the loading case has a test
 * rather than a screenshot.
 *
 * [snapshot] is the stats read that powers progress hints, and it lands SEPARATELY from (and later
 * than) the unlock rows. The old code answered a pending snapshot with a bare placeholder that
 * discarded the unlock rows it already had, and the screen rendered that placeholder as fact:
 * "0 EARNED", an empty progress bar and "Nothing earned yet" on an account full of trophies (L-08).
 *
 * The rows are the truth about what is earned and are used whether or not the snapshot has arrived;
 * only the things that genuinely need it — each locked trophy's progress hint and fraction, and the
 * closest-trophy nudge — are absent until it does. [TrophiesUiState.isLoading] is therefore false
 * here: by the time this runs the account is known. It stays true only for the placeholder the
 * screen shows before the very first emission, which is the one moment no count can be claimed.
 */
internal fun trophiesStateFor(
    unlockedByIdToDate: Map<String, Long>,
    nearMisses: List<NearMissEntry>,
    snapshot: TrophyStatsSnapshot?,
    filter: TrophyFilter,
    weightUnit: com.forge.app.domain.units.WeightUnit
): TrophiesUiState {
    val displays = Trophies.all.map { trophy ->
        val unlockedAt = unlockedByIdToDate[trophy.id]
        val locked = unlockedAt == null
        TrophyDisplay(
            trophy = trophy,
            unlockedAt = unlockedAt,
            progressHint = if (locked && snapshot != null) {
                TrophyEvaluator.progressHint(trophy.unlock, snapshot, weightUnit)
            } else null,
            progressFraction = if (locked && snapshot != null) {
                TrophyEvaluator.progressFraction(trophy.unlock, snapshot)
            } else null
        )
    }

    // Closest-trophy nudge (#55): locked trophy with the most progress. Needs the snapshot.
    val closestTrophyNudge = snapshot?.let { snap ->
        displays
            .filter { !it.isUnlocked && (it.progressFraction ?: 0f) > 0f }
            .maxByOrNull { it.progressFraction ?: 0f }
            ?.let { d ->
                TrophyEvaluator.progressRemaining(d.trophy.unlock, snap, weightUnit)
                    ?.let { remaining -> "$remaining away from ${d.trophy.name}" }
            }
    }

    val sections = displays.groupBy { it.trophy.category }
        .map { (cat, items) -> TrophySection(category = cat, displays = items) }
    val cumulativeScore = Trophies.all
        .filter { it.id in unlockedByIdToDate }
        .sumOf { it.tier.points }

    return TrophiesUiState(
        isLoading = false,
        unlockedCount = unlockedByIdToDate.size,
        totalCount = Trophies.all.size,
        sections = sections,
        selectedFilter = filter,
        closestTrophyNudge = closestTrophyNudge,
        nearMisses = nearMisses,
        cumulativeScore = cumulativeScore,
        maxScore = Trophies.all.sumOf { it.tier.points }
    )
}
