package com.forge.app.ui.nav

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/** Backs [HubScreen] — just the bits that decide which hub tabs are visible. */
@HiltViewModel
class HubViewModel @Inject constructor(settingsRepo: SettingsRepository) : ViewModel() {
    /** When false the Coach tab is removed from the bar/pager (declined in onboarding / Settings). */
    val coachEnabled: StateFlow<Boolean> =
        settingsRepo.coachEnabled.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), true)

    /** "Go with the flow" — the Coach tab is also removed here (there's no plan to coach against). */
    val freestyleMode: StateFlow<Boolean> =
        settingsRepo.freestyleMode.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)
}
