package com.forge.app.ui.gym.train

import kotlinx.coroutines.flow.distinctUntilChanged
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
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class DayListViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val customizationRepo: CustomizationRepository,
    private val sessionDao: SessionDao,
    private val settingsRepo: SettingsRepository,
    private val programRepository: com.forge.app.data.repo.ProgramRepository,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard
) : ViewModel() {

    /** Personal rest pace (engine System 2) + the user's rest bases — both fold into the day cards'
     *  "~min" estimate. Bundled so the estimate is computed against the SAME base the tuning was
     *  calibrated against (mismatched bases mis-scale the estimate). */
    private data class RestConfig(
        val tuning: com.forge.app.domain.adapt.RestTuning,
        val compoundBase: Int,
        val isolationBase: Int
    )

    private val restConfig = kotlinx.coroutines.flow.MutableStateFlow(
        RestConfig(
            com.forge.app.domain.adapt.RestTuning.NEUTRAL,
            com.forge.app.program.SessionEstimate.COMPOUND_REST,
            com.forge.app.program.SessionEstimate.ISOLATION_REST
        )
    )

    init {
        // COLLECT the rest settings, don't sample them once.
        //
        // This ViewModel is scoped to the day-list destination and survives the round trip into
        // Settings and back, so a `first()` read taken at construction was the answer for the rest
        // of its life: change "Compound rest" from 150 s to 180 s, come back, and every day still
        // showed the estimate built from 150 — a number the settings screen had just told the user
        // was no longer true. The tuning pass is re-run with the new bases, because the samples are
        // interpreted relative to them.
        viewModelScope.launch {
            combine(
                settingsRepo.restCompoundSeconds,
                settingsRepo.restIsolationSeconds
            ) { cBase, iBase -> cBase to iBase }
                .distinctUntilChanged()
                .collect { (cBase, iBase) ->
                    val tuning = com.forge.app.domain.adapt.RestAdvisor.tuning(
                        com.forge.app.domain.adapt.RestAdvisor.samples(
                            workoutRepo.recentRestEvents(),
                            compoundBase = cBase,
                            isolationBase = iBase
                        ) { Program.exercise(it) }
                    )
                    restConfig.value = RestConfig(tuning, cBase, iBase)
                }
        }
    }

    val state: StateFlow<DayListUiState> = combine(
        customizationRepo.observeAllDayNames(),
        workoutRepo.observeActiveSession(),
        // TRACKED: all three things derived from this — each day's last-trained date, today's
        // trained keys, and the next-up resolution — are schedule signals. A session the user
        // excluded from their record was still consuming today's slot and deciding what came next.
        sessionDao.observeRecentTracked(50),
        settingsRepo.observeAllDayColors(),
        combine(
            programRepository.revision,
            settingsRepo.scheduleMode,
            settingsRepo.weeklySchedule
        ) { _, mode, schedule -> mode to schedule }
    ) { dayNames, activeSession, recentSessions, dayColors, modeSchedule ->
        val (scheduleMode, schedule) = modeSchedule
        val nameByKey = dayNames.associate { it.dayKey to it.customName }
        val lastFinishedByKey = recentSessions
            .filter { it.finishedAt != null }
            .groupBy { it.dayKey }
            .mapValues { (_, sessions) -> sessions.maxOf { it.finishedAt!! } }

        // "Next up" is calendar-aware in weekday mode and the legacy day-after-last otherwise — one
        // shared resolver so the day list, Overview, and widget agree.
        val zone = java.time.ZoneId.systemDefault()
        val today = java.time.LocalDate.now(zone)
        val trainedTodayKeys = recentSessions
            .filter { it.finishedAt != null && java.time.Instant.ofEpochMilli(it.finishedAt!!).atZone(zone).toLocalDate() == today }
            .map { it.dayKey }.toSet()
        val lastFinishedDayKey = recentSessions
            .filter { it.finishedAt != null }.maxByOrNull { it.finishedAt!! }?.dayKey
        val nextUpKey = when {
            activeSession != null -> activeSession.dayKey
            else -> com.forge.app.domain.schedule.WeeklySchedule.resolveNextUp(
                mode = scheduleMode, todayIndex = today.dayOfWeek.value - 1, schedule = schedule,
                dayKeys = Program.dayKeys, lastFinishedDayKey = lastFinishedDayKey,
                trainedTodayKeys = trainedTodayKeys
            ) ?: (Program.dayKeys.firstOrNull() ?: Program.UPPER_A)
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
    }.combine(restConfig) { s, cfg ->
        s.copy(days = s.days.map { item ->
            item.copy(
                estimatedMinutes = com.forge.app.domain.adapt.RestAdvisor.estimateMinutes(
                    item.plan, cfg.tuning,
                    compoundBase = cfg.compoundBase, isolationBase = cfg.isolationBase
                )
            )
        })
    }.combine(settingsRepo.freestyleMode) { s, freestyle ->
        s.copy(freestyleMode = freestyle)
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = DayListUiState()
    )

    fun setDayColor(dayKey: String, hex: String?) {
        viewModelScope.launch { settingsRepo.setDayColor(dayKey, hex) }
    }

    /**
     * Re-roll just this day's exercises, keeping the rest of the week (Phase 6). Re-rolling the day
     * you're currently training discards that workout, so guard it — but only when the active session
     * is on THIS day (re-rolling an unrelated day never touches the workout, so it passes through).
     */
    fun rerollDay(dayKey: String) {
        viewModelScope.launch {
            programChangeGuard.run(affectsSession = { it.dayKey == dayKey }) {
                programRepository.rerollDay(dayKey)
            }
        }
    }
}
