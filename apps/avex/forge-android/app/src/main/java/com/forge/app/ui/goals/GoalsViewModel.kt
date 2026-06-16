package com.forge.app.ui.goals

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.GoalRepository
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The aggregated Goals screen: every exercise goal with its progress, plus the exercises you can
 * still add a goal for. Per-exercise goals are also settable inside a workout (the goal-setter
 * dialog) — this just surfaces them all in one place and lets you add/clear outside a session.
 */
@HiltViewModel
class GoalsViewModel @Inject constructor(
    private val goalRepo: GoalRepository
) : ViewModel() {

    data class ExerciseOption(val id: String, val name: String)

    data class UiState(
        val loading: Boolean = true,
        val goals: List<GoalRepository.GoalProgress> = emptyList(),
        /** Program exercises that don't have a goal yet — the "add a goal" pick list. */
        val addable: List<ExerciseOption> = emptyList()
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init { reload() }

    private fun reload() = viewModelScope.launch {
        val goals = runCatching { goalRepo.goalsWithProgress() }.getOrDefault(emptyList())
        val have = goals.mapTo(mutableSetOf()) { it.exerciseId }
        val addable = Program.days
            .flatMap { it.exercises }
            .distinctBy { it.id }
            .filter { it.id !in have }
            .map { ExerciseOption(it.id, it.name) }
            .sortedBy { it.name }
        _state.value = UiState(loading = false, goals = goals, addable = addable)
    }

    fun setGoal(exerciseId: String, targetWeightLb: Double) = viewModelScope.launch {
        goalRepo.setGoal(exerciseId, targetWeightLb)
        reload()
    }

    fun clearGoal(exerciseId: String) = viewModelScope.launch {
        goalRepo.clearGoal(exerciseId)
        reload()
    }
}
