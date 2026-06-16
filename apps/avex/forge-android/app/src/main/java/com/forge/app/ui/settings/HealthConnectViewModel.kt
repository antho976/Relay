package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.health.HealthConnectManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives the Settings → Recovery page: Health Connect availability + whether the recovery
 * permissions are granted. The page owns the permission-request launcher; this just exposes the
 * permission set and re-checks state after the user returns from the Health Connect UI.
 */
@HiltViewModel
class HealthConnectViewModel @Inject constructor(
    private val manager: HealthConnectManager
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** A usable Health Connect provider is installed. */
        val available: Boolean = false,
        /** Provider present but needs a Play-store update before it works. */
        val needsUpdate: Boolean = false,
        /** Every recovery permission is granted. */
        val granted: Boolean = false
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    /** Permissions the page's launcher should request. */
    val permissions: Set<String> get() = manager.permissions

    init { refresh() }

    fun refresh() = viewModelScope.launch {
        val available = manager.isAvailable
        val granted = if (available) manager.hasAllPermissions() else false
        _state.value = UiState(
            loading = false,
            available = available,
            needsUpdate = manager.needsUpdate,
            granted = granted
        )
    }
}
