package com.forge.app.ui.coach

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.CoachBrief
import com.forge.app.data.repo.CoachRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Loads the Week Brief and drives the propose/apply lifecycle (auto-coach Phase 3):
 * apply / skip per decision, apply-all, and single-change undo.
 */
@HiltViewModel
class CoachBriefViewModel @Inject constructor(
    private val coachRepo: CoachRepository,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** Null after loading = even the pass record failed to load — render the error body. */
        val brief: CoachBrief? = null,
        /** Show the one-time "how your coach learns" card on first Brief open (CO6). */
        val showIntroCard: Boolean = false
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val brief = runCatching { coachRepo.brief() }.getOrNull()
            // Default to "seen" on a read failure so we never flash the intro on every open.
            val introSeen = runCatching { settingsRepo.coachBriefIntroSeen.first() }.getOrDefault(true)
            _state.value = UiState(loading = false, brief = brief, showIntroCard = !introSeen)
            // Opening the brief (from the banner OR Settings) clears the Overview "new report" banner.
            brief?.let { runCatching { coachRepo.markSeen(it.pass.weekId) } }
        }
    }

    /** Dismiss the one-time coach intro card and remember it so it never shows again (CO6). */
    fun dismissIntro() {
        _state.value = _state.value.copy(showIntroCard = false)
        viewModelScope.launch { runCatching { settingsRepo.setCoachBriefIntroSeen() } }
    }

    fun apply(decisionId: Long) {
        // A deload decision regenerates the program (discarding any in-progress workout); guard it.
        // Every other type is an overlay edit that leaves the session alone, so apply directly.
        val isDeload = _state.value.brief?.decisions?.firstOrNull { it.id == decisionId }?.type == "deload"
        if (isDeload) guardedAct { coachRepo.applyDecision(decisionId) } else act { coachRepo.applyDecision(decisionId) }
    }

    fun skip(decisionId: Long) = act { coachRepo.skipDecision(decisionId) }
    fun undo(decisionId: Long) = act { coachRepo.undoDecision(decisionId) }

    fun applyAll(weekId: String) {
        // Apply-all runs every proposed decision; if a deload is among them the program regenerates
        // and the active workout is discarded — guard the whole batch in that case.
        val hasDeload = _state.value.brief?.decisions
            ?.any { it.status == "proposed" && it.type == "deload" } == true
        if (hasDeload) guardedAct { coachRepo.applyAll(weekId) } else act { coachRepo.applyAll(weekId) }
    }

    private fun act(block: suspend () -> Unit) = viewModelScope.launch {
        runCatching { block() }
        _state.value = _state.value.copy(brief = runCatching { coachRepo.refreshBrief() }.getOrNull() ?: _state.value.brief)
    }

    /** Like [act], but routed through the workout-discard guard for program-regenerating changes. */
    private fun guardedAct(block: suspend () -> Unit) = viewModelScope.launch {
        programChangeGuard.run {
            runCatching { block() }
            _state.value = _state.value.copy(brief = runCatching { coachRepo.refreshBrief() }.getOrNull() ?: _state.value.brief)
        }
    }
}
