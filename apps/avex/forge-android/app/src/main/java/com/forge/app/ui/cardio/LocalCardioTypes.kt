package com.forge.app.ui.cardio

import androidx.compose.runtime.compositionLocalOf
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.cardio.CustomCardioType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/**
 * The user's custom cardio activities (GYMAP-37), provided once near the nav root so any cardio
 * surface — a recent-session row, the session detail, a History row, the log picker — can resolve a
 * stored `custom_` code to its real name + glyph without threading the list through every ViewModel.
 * Mirrors [com.forge.app.ui.common.LocalGoHome]: a process-wide read rather than per-screen plumbing.
 * Empty default so previews / tests need no provider (a custom code then reads as "Other").
 */
val LocalCardioTypes = compositionLocalOf<List<CustomCardioType>> { emptyList() }

/** Feeds [LocalCardioTypes] from the persisted list. Kept tiny and separate from the per-screen VMs. */
@HiltViewModel
class CardioTypesViewModel @Inject constructor(
    settingsRepo: SettingsRepository
) : ViewModel() {
    val types: StateFlow<List<CustomCardioType>> = settingsRepo.customCardioTypes
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())
}
