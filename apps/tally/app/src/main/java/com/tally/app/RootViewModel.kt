package com.tally.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.prefs.Settings
import com.tally.app.data.prefs.SettingsRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/** The settings the whole app is themed by. Null until the first read, which holds the splash. */
@HiltViewModel
class RootViewModel @Inject constructor(settings: SettingsRepository) : ViewModel() {
    val settings: StateFlow<Settings?> = settings.settings
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)
}
