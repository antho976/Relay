package com.forge.app.ui.coach

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.CoachWatch
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Loads the "Coach lab" read-out (finding #5) — what the coach is currently watching and how
 * far its learning has come. Read-only: it never applies anything.
 */
@HiltViewModel
class CoachLabViewModel @Inject constructor(
    private val coachRepo: CoachRepository
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** Null after loading = the read-out failed to assemble. */
        val watch: CoachWatch? = null
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val watch = runCatching { coachRepo.coachLab() }.getOrNull()
            _state.value = UiState(loading = false, watch = watch)
        }
    }
}
