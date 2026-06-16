package com.forge.app.ui.coach

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.CoachTimeline
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Loads the "Coach learning timeline" (Tier 6) — earned trust, journey milestones, and the
 * week-by-week record. Read-only: it never applies anything.
 */
@HiltViewModel
class CoachTimelineViewModel @Inject constructor(
    private val coachRepo: CoachRepository
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        val timeline: CoachTimeline? = null,
        /** True when the read threw — distinct from a brand-new user with an empty (but valid) timeline. */
        val error: Boolean = false
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            runCatching { coachRepo.timeline() }
                .onSuccess { _state.value = UiState(loading = false, timeline = it) }
                .onFailure { _state.value = UiState(loading = false, error = true) }
        }
    }
}
