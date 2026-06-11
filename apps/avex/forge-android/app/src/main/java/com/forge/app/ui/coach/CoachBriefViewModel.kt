package com.forge.app.ui.coach

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.CoachBrief
import com.forge.app.data.repo.CoachRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Loads the Week Brief and drives the propose/apply lifecycle (auto-coach Phase 3):
 * apply / skip per decision, apply-all, and single-change undo.
 */
@HiltViewModel
class CoachBriefViewModel @Inject constructor(
    private val coachRepo: CoachRepository
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** Null after loading = even the pass record failed to load — render the error body. */
        val brief: CoachBrief? = null
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val brief = runCatching { coachRepo.brief() }.getOrNull()
            _state.value = UiState(loading = false, brief = brief)
            // Opening the brief (from the banner OR Settings) clears the Overview "new report" banner.
            brief?.let { runCatching { coachRepo.markSeen(it.pass.weekId) } }
        }
    }

    fun apply(decisionId: Long) = act { coachRepo.applyDecision(decisionId) }
    fun skip(decisionId: Long) = act { coachRepo.skipDecision(decisionId) }
    fun undo(decisionId: Long) = act { coachRepo.undoDecision(decisionId) }
    fun applyAll(weekId: String) = act { coachRepo.applyAll(weekId) }

    private fun act(block: suspend () -> Unit) = viewModelScope.launch {
        runCatching { block() }
        _state.value = _state.value.copy(brief = runCatching { coachRepo.refreshBrief() }.getOrNull() ?: _state.value.brief)
    }
}
