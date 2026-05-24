package com.forge.app.ui.bodyweight

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.BodyweightRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class BodyweightUiState(
    val entries: List<BodyweightEntry> = emptyList(),
    val latestLb: Double? = null
)

@HiltViewModel
class BodyweightViewModel @Inject constructor(
    private val repo: BodyweightRepository
) : ViewModel() {

    val state: StateFlow<BodyweightUiState> = repo.observeRecent(90)
        .map { entries ->
            BodyweightUiState(
                entries = entries,
                latestLb = entries.firstOrNull()?.weightLb
            )
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), BodyweightUiState())

    fun log(weightLb: Double) = viewModelScope.launch { repo.log(weightLb) }
    fun delete(id: Long) = viewModelScope.launch { repo.delete(id) }
}
