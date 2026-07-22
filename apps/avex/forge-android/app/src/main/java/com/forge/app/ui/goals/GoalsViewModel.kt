package com.forge.app.ui.goals

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.ui.common.SnackbarController
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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
    private val snackbar: SnackbarController,
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

    /** Latest hidden-exercises set — collected once in init; only shapes [UiState.liftPickerExclude]. */
    private var disliked: Set<String> = emptySet()

    init {
        // Reactive: reload whenever a goal table changes, so an edit made on the routed editor
        // screen is already reflected here when you come back.
        combine(goalRepo.observeAll(), extendedGoalRepo.observeAll()) { _, _ -> }
            .onEach { reload() }
            .launchIn(viewModelScope)
        // The hidden-exercises pref only affects the lift-picker exclude set — patch it in place
        // instead of re-running the goal-progress queries on every like/dislike toggle.
        settingsRepo.dislikedExercises
            .onEach { d ->
                disliked = d
                _state.value = _state.value.let { s -> s.copy(liftPickerExclude = excludeFrom(s.liftGoals, d)) }
            }
            .launchIn(viewModelScope)
    }

    /** Single-flight: a fresh reload cancels any in-flight one so a slow earlier read can't land after
     *  a newer one and overwrite _state with a stale snapshot (e.g. a just-deleted goal reappearing).
     *  [fetchOr] rethrows cancellation, so a cancelled reload dies before the state write. */
    private var reloadJob: Job? = null

    private fun reload() {
        reloadJob?.cancel()
        reloadJob = viewModelScope.launch {
            val (liftGoals, customGoals) = coroutineScope {
                val lifts = async { fetchOr(emptyList()) { goalRepo.goalsWithProgress() } }
                val customs = async { fetchOr(emptyList()) { extendedGoalRepo.goalsWithProgress() } }
                lifts.await() to customs.await()
            }
            _state.value = UiState(
                loading = false,
                liftGoals = liftGoals,
                customGoals = customGoals,
                liftPickerExclude = excludeFrom(liftGoals, disliked),
            )
        }
    }

    /** Fallback on real failures ONLY — swallowing CancellationException (as a bare runCatching
     *  would) lets a just-cancelled reload keep running and publish an empty snapshot. */
    private suspend fun <T> fetchOr(fallback: T, read: suspend () -> T): T =
        try { read() } catch (e: CancellationException) { throw e } catch (_: Exception) { fallback }

    private fun excludeFrom(liftGoals: List<GoalRepository.GoalProgress>, disliked: Set<String>): Set<String> =
        liftGoals.mapTo(mutableSetOf()) { it.exerciseId }.apply { addAll(disliked) }

    // ─── Mutations ─────────────────────────────────────────────────────────────
    // Writes run under NonCancellable: the routed editor fires one of these and immediately pops its
    // back-stack entry, which clears this ViewModel — without the shield the Room write could be
    // cancelled mid-flight and silently dropped (viewModelScope launches undispatched on
    // Main.immediate, so the shield is entered before the pop's cancellation lands). No explicit
    // reload afterwards: the observeAll() combine in init refreshes any live list reactively.

    // ─── Lift targets (exercise_goal) ──────────────────────────────────────────

    fun setLiftGoal(exerciseId: String, targetWeightLb: Double) = viewModelScope.launch {
        // Repo-level GoalRepository has no floor; guard here so no picker path stores a degenerate
        // target (fraction math needs > 0).
        if (targetWeightLb <= 0) return@launch
        withContext(NonCancellable) { goalRepo.setGoal(exerciseId, targetWeightLb) }
    }

    fun clearLiftGoal(exerciseId: String) = viewModelScope.launch {
        withContext(NonCancellable) { goalRepo.clearGoal(exerciseId) }
    }

    // ─── Custom goals (extended_goal) ──────────────────────────────────────────

    fun createCustomGoal(metric: GoalMetric, period: GoalPeriod, targetValue: Double, label: String) =
        viewModelScope.launch {
            withContext(NonCancellable) { extendedGoalRepo.create(metric, period, targetValue, label) }
        }

    fun updateCustomGoalTarget(id: Long, targetValue: Double) = viewModelScope.launch {
        withContext(NonCancellable) { extendedGoalRepo.updateTarget(id, targetValue) }
    }

    // §13 undo over confirm: delete now, offer a short Undo. NonCancellable (like the other mutations)
    // because the editor pops its back-stack entry — clearing this VM — the moment it fires this; the
    // shield lets the capture + delete + snackbar emit finish. The removed row (baseline and all) is
    // held for the undo, which re-inserts it with its original id.
    fun deleteCustomGoal(id: Long) = viewModelScope.launch {
        withContext(NonCancellable) {
            val removed = extendedGoalRepo.delete(id) ?: return@withContext
            snackbar.showUndo("Goal deleted") { extendedGoalRepo.restore(removed) }
        }
    }
}
