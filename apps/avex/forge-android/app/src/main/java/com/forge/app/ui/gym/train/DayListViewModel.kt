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
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    val state: StateFlow<DayListUiState> = combine(
        customizationRepo.observeAllDayNames(),
        workoutRepo.observeActiveSession(),
        sessionDao.observeRecent(50),
        settingsRepo.observeAllDayColors()
    ) { dayNames, activeSession, recentSessions, dayColors ->
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
                if (lastFinished == null) Program.UPPER_A
                else {
                    val idx = Program.dayKeys.indexOf(lastFinished.dayKey)
                    Program.dayKeys[(idx + 1) % Program.dayKeys.size]
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
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = DayListUiState()
    )

    fun setDayColor(dayKey: String, hex: String?) {
        viewModelScope.launch { settingsRepo.setDayColor(dayKey, hex) }
    }
}
