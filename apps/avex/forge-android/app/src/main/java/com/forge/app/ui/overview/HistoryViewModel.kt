package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class HistoryViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val cardioRepo: CardioRepository
) : ViewModel() {

    data class HistoryEntry(
        val id: Long,
        val timestampMs: Long,
        val isGym: Boolean,
        val title: String,
        val subtitle: String,
        val tag: String,
        val volumeLb: Double? = null,
        val prCount: Int = 0,
        val vsAvgPct: Int? = null,
        val isBest: Boolean = false,
        val durationMin: Int? = null,
        val distanceKm: Double? = null,
    )

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val entries: StateFlow<List<HistoryEntry>> = combine(
        statsRepo.observeAllFinishedSessions(),
        cardioRepo.observeRecent(1000),
        statsRepo.observeDayVolumeStats()
    ) { sessions, cardio, dayVolStats ->
        val gym = sessions.map { s ->
            val day = Program.days.firstOrNull { it.key == s.dayKey }
            val durationMin = s.finishedAt?.let { ((it - s.startedAt) / 60_000).toInt() }
            val volStats = dayVolStats[s.dayKey]
            val vsAvgPct = if (volStats != null && s.totalVolumeLb != null && volStats.avgVolume > 0)
                (((s.totalVolumeLb - volStats.avgVolume) / volStats.avgVolume) * 100).toInt()
            else null
            val isBest = volStats != null && s.totalVolumeLb != null && s.totalVolumeLb >= volStats.maxVolume
            HistoryEntry(
                id = s.id,
                timestampMs = s.startedAt,
                isGym = true,
                title = day?.defaultName ?: s.dayKey,
                subtitle = listOfNotNull(
                    day?.exercises?.size?.let { "$it ex" },
                    durationMin?.let { "$it min" }
                ).joinToString(" · "),
                tag = day?.word ?: "",
                volumeLb = s.totalVolumeLb,
                prCount = s.prCount,
                vsAvgPct = vsAvgPct,
                isBest = isBest,
                durationMin = durationMin
            )
        }
        val cardioEntries = cardio.map { c ->
            HistoryEntry(
                id = c.id,
                timestampMs = c.date,
                isGym = false,
                title = "Cardio · ${c.type.replaceFirstChar { it.uppercase() }}",
                subtitle = listOfNotNull(
                    "${c.durationMin} min",
                    c.distanceKm?.takeIf { it > 0 }?.let { "%.1f km".format(it) }
                ).joinToString(" · "),
                tag = "MOVE",
                durationMin = c.durationMin,
                distanceKm = c.distanceKm
            )
        }
        (gym + cardioEntries).sortedByDescending { it.timestampMs }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private val _selectedEntry = MutableStateFlow<HistoryEntry?>(null)
    val selectedEntry: StateFlow<HistoryEntry?> = _selectedEntry

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val sessionExerciseLines: StateFlow<List<StatsRepository.SessionExerciseLine>> =
        _selectedEntry.flatMapLatest { entry ->
            if (entry == null || !entry.isGym) flowOf(emptyList())
            else flow { emit(statsRepo.getSessionExerciseLines(entry.id)) }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun selectEntry(entry: HistoryEntry) { _selectedEntry.value = entry }
    fun clearSelectedEntry() { _selectedEntry.value = null }
}
