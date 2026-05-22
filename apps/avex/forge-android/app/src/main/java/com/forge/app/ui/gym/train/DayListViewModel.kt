package com.forge.app.ui.gym.train

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
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
import javax.inject.Inject

/**
 * Drives the four-day list. The day cards are constant (taken from the static
 * [Program]); only [DayListItem.displayName] (user override), [DayListItem.lastFinishedAt],
 * [DayListItem.isActive], and [DayListItem.isNextUp] vary at runtime.
 *
 * Next-up rule: if there is an active session, that day is next-up. Otherwise, find the
 * most-recently-finished session and advance one step in the [Program.dayKeys] rotation
 * (Upper A → Lower A → Upper B → Lower B → Upper A …). Defaults to Upper A when no
 * session has ever been completed.
 */
@HiltViewModel
class DayListViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val customizationRepo: CustomizationRepository,
    private val sessionDao: SessionDao
) : ViewModel() {

    val state: StateFlow<DayListUiState> = combine(
        customizationRepo.observeAllDayNames(),
        workoutRepo.observeActiveSession(),
        sessionDao.observeRecent(50)
    ) { dayNames, activeSession, recentSessions ->
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
                if (lastFinished == null) {
                    Program.UPPER_A
                } else {
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
                    exerciseCount = plan.exercises.size
                )
            },
            activeSession = activeSession
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = DayListUiState()
    )
}
