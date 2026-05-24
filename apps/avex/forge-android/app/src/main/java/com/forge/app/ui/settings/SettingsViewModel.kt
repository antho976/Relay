package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ResetRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SettingsUiState(
    val amoledMode: Boolean = false,
    val useKg: Boolean = false,
    val showEncouragement: Boolean = true,
    val compactSetLogging: Boolean = false,
    val noteTemplates: Set<String> = setOf("form felt: ", "energy: ", "pain/discomfort: ", "focus cue: "),
    val hiddenOverviewTiles: Set<String> = emptySet(),
    val overviewTileOrder: List<String> = listOf("gym", "cardio", "trophies"),
    val dateFormat: String = "MMM d, yyyy",
    val timeFormat24h: Boolean = false,
    val firstDayMonday: Boolean = true,
    val hapticStrength: String = "strong",
    val quietHoursEnabled: Boolean = false,
    val quietHoursStart: Int = 22,
    val quietHoursEnd: Int = 7,
    val privacyMode: Boolean = false,
    val availableEquipment: Set<String> = emptySet(),
    val accentColorHex: String = ""
)

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val resetRepo: ResetRepository,
    private val backupRepo: com.forge.app.data.repo.BackupRepository,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder,
    private val pdfExport: com.forge.app.data.repo.PdfExportRepository
) : ViewModel() {

    val state: StateFlow<SettingsUiState> = combine(
        settingsRepo.amoledMode,
        settingsRepo.useKg,
        settingsRepo.dateFormat,
        settingsRepo.timeFormat24h,
        settingsRepo.firstDayMonday,
        settingsRepo.hapticStrength,
        settingsRepo.quietHoursEnabled,
        settingsRepo.quietHoursStart,
        settingsRepo.quietHoursEnd
    ) { values ->
        SettingsUiState(
            amoledMode = values[0] as Boolean,
            useKg = values[1] as Boolean,
            dateFormat = values[2] as String,
            timeFormat24h = values[3] as Boolean,
            firstDayMonday = values[4] as Boolean,
            hapticStrength = values[5] as String,
            quietHoursEnabled = values[6] as Boolean,
            quietHoursStart = values[7] as Int,
            quietHoursEnd = values[8] as Int
        )
    }.combine(settingsRepo.noteTemplates) { s, templates ->
        s.copy(noteTemplates = templates)
    }.combine(settingsRepo.hiddenOverviewTiles) { s, hidden ->
        s.copy(hiddenOverviewTiles = hidden)
    }.combine(settingsRepo.showEncouragement) { s, v ->
        s.copy(showEncouragement = v)
    }.combine(settingsRepo.compactSetLogging) { s, v ->
        s.copy(compactSetLogging = v)
    }.combine(settingsRepo.overviewTileOrder) { s, order ->
        s.copy(overviewTileOrder = order)
    }.combine(settingsRepo.privacyMode) { s, v ->
        s.copy(privacyMode = v)
    }.combine(settingsRepo.availableEquipment) { s, equip ->
        s.copy(availableEquipment = equip)
    }.combine(settingsRepo.accentColorHex) { s, v ->
        s.copy(accentColorHex = v)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SettingsUiState())

    fun setAmoledMode(v: Boolean) = viewModelScope.launch { settingsRepo.setAmoledMode(v) }
    fun setUseKg(v: Boolean) = viewModelScope.launch { settingsRepo.setUseKg(v) }
    fun setDateFormat(v: String) = viewModelScope.launch { settingsRepo.setDateFormat(v) }
    fun setTimeFormat24h(v: Boolean) = viewModelScope.launch { settingsRepo.setTimeFormat24h(v) }
    fun setFirstDayMonday(v: Boolean) = viewModelScope.launch { settingsRepo.setFirstDayMonday(v) }
    fun setHapticStrength(v: String) = viewModelScope.launch { settingsRepo.setHapticStrength(v) }
    fun setQuietHoursEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setQuietHoursEnabled(v) }
    fun setQuietHoursStart(v: Int) = viewModelScope.launch { settingsRepo.setQuietHoursStart(v) }
    fun setQuietHoursEnd(v: Int) = viewModelScope.launch { settingsRepo.setQuietHoursEnd(v) }

    fun setTileHidden(id: String, hidden: Boolean) = viewModelScope.launch { settingsRepo.setTileHidden(id, hidden) }
    fun setShowEncouragement(v: Boolean) = viewModelScope.launch { settingsRepo.setShowEncouragement(v) }
    fun setCompactSetLogging(v: Boolean) = viewModelScope.launch { settingsRepo.setCompactSetLogging(v) }
    fun setCustomWarmup(dayKey: String, items: List<String>) =
        viewModelScope.launch { settingsRepo.setCustomWarmup(dayKey, items) }
    fun setOverviewTileOrder(order: List<String>) =
        viewModelScope.launch { settingsRepo.setOverviewTileOrder(order) }

    fun resetSessions() = viewModelScope.launch { resetRepo.resetSessions() }
    fun resetTrophies() = viewModelScope.launch { resetRepo.resetTrophies() }
    fun resetCardio() = viewModelScope.launch { resetRepo.resetCardio() }
    fun resetSettings() = viewModelScope.launch { resetRepo.resetAppSettings() }
    fun factoryReset() = viewModelScope.launch { resetRepo.factoryReset() }
    fun loadSampleData() = viewModelScope.launch { sampleDataSeeder.seed() }
    fun setPrivacyMode(v: Boolean) = viewModelScope.launch { settingsRepo.setPrivacyMode(v) }
    fun setAvailableEquipment(codes: Set<String>) = viewModelScope.launch { settingsRepo.setAvailableEquipment(codes) }
    fun setAccentColorHex(hex: String) = viewModelScope.launch { settingsRepo.setAccentColorHex(hex) }
    fun exportLastSessionPdf() = viewModelScope.launch {
        val file = pdfExport.exportLastSessionPdf()
        if (file != null) _exportPath.value = file.absolutePath
    }

    private val _exportPath = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val exportPath: kotlinx.coroutines.flow.StateFlow<String?> = _exportPath.asStateFlow()

    fun exportWeeklyJson() = viewModelScope.launch {
        val file = backupRepo.exportWeeklyJson()
        _exportPath.value = file.absolutePath
    }
    fun exportFullBackup() = viewModelScope.launch {
        val file = backupRepo.exportFullBackup()
        _exportPath.value = file.absolutePath
    }
    fun exportSessionsCsv() = viewModelScope.launch {
        val file = backupRepo.exportSessionsCsv()
        _exportPath.value = file.absolutePath
    }
    fun clearExportPath() { _exportPath.value = null }
}
