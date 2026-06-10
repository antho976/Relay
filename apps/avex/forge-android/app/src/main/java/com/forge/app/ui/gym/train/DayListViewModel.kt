package com.forge.app.ui.gym.train

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.program.Program
import com.forge.app.ui.gym.train.state.DayListItem
import com.forge.app.ui.gym.train.state.DayListUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class DayListViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val customizationRepo: CustomizationRepository,
    private val sessionDao: SessionDao,
    private val settingsRepo: SettingsRepository,
    private val programRepository: com.forge.app.data.repo.ProgramRepository
) : ViewModel() {

    /** Personal rest pace (engine System 2) — folds into the day cards' "~min" estimate. */
    private val restTuning = kotlinx.coroutines.flow.MutableStateFlow(
        com.forge.app.domain.adapt.RestTuning.NEUTRAL
    )

    init {
        viewModelScope.launch {
            restTuning.value = com.forge.app.domain.adapt.RestAdvisor.tuning(
                com.forge.app.domain.adapt.RestAdvisor.samples(workoutRepo.recentRestEvents()) {
                    Program.exercise(it)
                }
            )
        }
    }

    val state: StateFlow<DayListUiState> = combine(
        customizationRepo.observeAllDayNames(),
        workoutRepo.observeActiveSession(),
        sessionDao.observeRecent(50),
        settingsRepo.observeAllDayColors(),
        programRepository.revision
    ) { dayNames, activeSession, recentSessions, dayColors, _ ->
        val nameByKey = dayNames.associate { it.dayKey to it.customName }
        val lastFinishedByKey = recentSessions
            .filter { it.finishedAt != null }
            .groupBy { it.dayKey }
            .mapValues { (_, sessions) -> sessions.maxOf { it.finishedAt!! } }

        val nextUpKey = when {
            activeSession != null -> activeSession.dayKey
            else -> {
                val lastFinished = recentSessions
                    .filter { it.finishedAt != null }
                    .maxByOrNull { it.finishedAt!! }
                if (lastFinished == null) (Program.dayKeys.firstOrNull() ?: Program.UPPER_A)
                else {
                    // A stale finished key (e.g. the user changed day-count since) isn't in the current
                    // split — fall back to the first day instead of letting indexOf(-1) point at it.
                    val idx = Program.dayKeys.indexOf(lastFinished.dayKey)
                    if (idx < 0) (Program.dayKeys.firstOrNull() ?: Program.UPPER_A)
                    else Program.dayKeys[(idx + 1) % Program.dayKeys.size]
                }
            }
        }

        DayListUiState(
            days = Program.days.map { plan ->
                DayListItem(
                    plan = plan,
                    displayName = nameByKey[plan.key] ?: plan.defaultName,
                    lastFinishedAt = lastFinishedByKey[plan.key],
                    isActive = activeSession?.dayKey == plan.key,
                    isNextUp = plan.key == nextUpKey,
                    exerciseCount = plan.exercises.size,
                    customAccentHex = dayColors[plan.key]
                )
            },
            activeSession = activeSession
        )
    }.combine(restTuning) { s, tuning ->
        s.copy(days = s.days.map { item ->
            item.copy(estimatedMinutes = com.forge.app.domain.adapt.RestAdvisor.estimateMinutes(item.plan, tuning))
        })
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = DayListUiState()
    )

    fun setDayColor(dayKey: String, hex: String?) {
        viewModelScope.launch { settingsRepo.setDayColor(dayKey, hex) }
    }

    /** Re-roll just this day's exercises, keeping the rest of the week (Phase 6). */
    fun rerollDay(dayKey: String) {
        viewModelScope.launch { programRepository.rerollDay(dayKey) }
    }
}
