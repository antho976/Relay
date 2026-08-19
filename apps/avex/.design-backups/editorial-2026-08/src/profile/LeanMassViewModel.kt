package com.forge.app.ui.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.repo.LeanMassRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives the Profile BODY section's MUSCLE row (W6) — the watch's BIA lean-mass trend. Its own
 * small ViewModel like [BodyMeasurementsViewModel], so ProfileViewModel stays untouched. Import-
 * only: the row's one action pulls the latest Health Connect reading.
 */
@HiltViewModel
class LeanMassViewModel @Inject constructor(
    private val repo: LeanMassRepository
) : ViewModel() {

    data class UiState(
        /** Chronological (oldest → newest), the spark/figure convention of the sibling rows. */
        val entries: List<LeanMassEntry> = emptyList(),
        /** The Health Connect read is granted — the row may render and offer `sync →`. */
        val connected: Boolean = false
    )

    private val connected = MutableStateFlow(false)

    init { refreshConnection() }

    /** Re-check the grant (call on resume — it can change in the HC app). */
    fun refreshConnection() = viewModelScope.launch {
        connected.value = repo.canImportFromHealthConnect()
    }

    val state: StateFlow<UiState> = repo.observeRecent(90)
        .map { it.sortedBy { e -> e.dateKey } }
        .combine(connected) { entries, conn -> UiState(entries = entries, connected = conn) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), UiState())

    /** Pull the latest reading from Health Connect (idempotent; the row updates reactively). */
    fun syncNow() = viewModelScope.launch { repo.importLatestFromHealthConnect() }
}
