package com.forge.app.ui.goals

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The aggregated Goals screen. Two kinds of goal live here:
 *   - lift targets (a heaviest-set weight goal per exercise, [GoalRepository]) — settable in a workout
 *     too, surfaced here for any library exercise via the searchable picker.
 *   - custom goals (cardio / bodyweight / sessions / volume with a target + period, [ExtendedGoalRepository]) —
 *     assembled from parameters and auto-tracked.
 */
@HiltViewModel
class GoalsViewModel @Inject constructor(
    private val goalRepo: GoalRepository,
    private val extendedGoalRepo: ExtendedGoalRepository,
    private val settingsRepo: SettingsRepository,
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        val liftGoals: List<GoalRepository.GoalProgress> = emptyList(),
        val customGoals: List<ExtendedGoalRepository.Progress> = emptyList(),
        /**
         * Exercise ids the lift-target picker must not offer: ones that already have a goal, plus the
         * user's Hidden/disliked exercises (from Exercise likes) so hidden lifts never resurface here.
         */
        val liftPickerExclude: Set<String> = emptySet(),
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init { reload() }

    private fun reload() = viewModelScope.launch {
        val liftGoals = runCatching { goalRepo.goalsWithProgress() }.getOrDefault(emptyList())
        val customGoals = runCatching { extendedGoalRepo.goalsWithProgress() }.getOrDefault(emptyList())
        val disliked = runCatching { settingsRepo.dislikedExercises.first() }.getOrDefault(emptySet())
        _state.value = UiState(
            loading = false,
            liftGoals = liftGoals,
            customGoals = customGoals,
            liftPickerExclude = liftGoals.mapTo(mutableSetOf()) { it.exerciseId }.apply { addAll(disliked) },
        )
    }

    // ─── Lift targets (exercise_goal) ──────────────────────────────────────────

    fun setLiftGoal(exerciseId: String, targetWeightLb: Double) = viewModelScope.launch {
        goalRepo.setGoal(exerciseId, targetWeightLb)
        reload()
    }

    fun clearLiftGoal(exerciseId: String) = viewModelScope.launch {
        goalRepo.clearGoal(exerciseId)
        reload()
    }

    // ─── Custom goals (extended_goal) ──────────────────────────────────────────

    fun createCustomGoal(metric: GoalMetric, period: GoalPeriod, targetValue: Double, label: String) =
        viewModelScope.launch {
            extendedGoalRepo.create(metric, period, targetValue, label)
            reload()
        }

    fun updateCustomGoalTarget(id: Long, targetValue: Double) = viewModelScope.launch {
        extendedGoalRepo.updateTarget(id, targetValue)
        reload()
    }

    fun deleteCustomGoal(id: Long) = viewModelScope.launch {
        extendedGoalRepo.delete(id)
        reload()
    }
}
