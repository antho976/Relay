package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives the Settings → Recovery page: Health Connect availability, whether the recovery
 * permissions are granted, and (HC-2/HC-3) the optional bodyweight bridge — read access, the
 * write-back toggle, and a one-tap import. The page owns the permission-request launchers; this
 * just exposes the permission sets and re-checks state after the user returns from the HC UI.
 */
@HiltViewModel
class HealthConnectViewModel @Inject constructor(
    private val manager: HealthConnectManager,
    private val settingsRepo: SettingsRepository,
    private val bodyweightRepo: BodyweightRepository
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** A usable Health Connect provider is installed. */
        val available: Boolean = false,
        /** Provider present but needs a Play-store update before it works. */
        val needsUpdate: Boolean = false,
        /** Every recovery permission is granted. */
        val granted: Boolean = false,
        /** Bodyweight READ permission is granted — Forge may import a scale value (HC-2). */
        val weightGranted: Boolean = false,
        /** Write-back opt-in: mirror each weigh-in to Health Connect (HC-3). */
        val writeBodyweight: Boolean = false,
        /** Transient one-tap-import result line, cleared on the next refresh. */
        val importMessage: String? = null
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    /** Permissions the recovery launcher should request (sleep + resting HR). */
    val permissions: Set<String> get() = manager.permissions

    /** Permissions the bodyweight launcher should request (read + write WeightRecord). */
    val weightPermissions: Set<String> get() = manager.weightPermissions

    init { refresh() }

    fun refresh() = viewModelScope.launch {
        val available = manager.isAvailable
        val granted = if (available) manager.hasAllPermissions() else false
        val weightGranted = if (available) manager.canReadWeight() else false
        val writeBodyweight = settingsRepo.hcWriteBodyweight.first()
        _state.value = _state.value.copy(
            loading = false,
            available = available,
            needsUpdate = manager.needsUpdate,
            granted = granted,
            weightGranted = weightGranted,
            writeBodyweight = writeBodyweight
            // importMessage preserved (copy, not a fresh UiState) so a just-shown import result line
            // isn't wiped by a lifecycle-driven refresh before the user can read it.
        )
    }

    fun setWriteBodyweight(value: Boolean) = viewModelScope.launch {
        settingsRepo.setHcWriteBodyweight(value)
        _state.value = _state.value.copy(writeBodyweight = value)
    }

    fun importNow() = viewModelScope.launch {
        val imported = bodyweightRepo.importLatestFromHealthConnect()
        _state.value = _state.value.copy(
            importMessage = if (imported != null) "Imported your latest weight." else "No newer weight in Health Connect."
        )
    }
}
