package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ResetRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SettingsUiState(
    val amoledMode: Boolean = false,
    val useKg: Boolean = false,
    val compactSetLogging: Boolean = false,
    val noteTemplates: Set<String> = setOf("form felt: ", "energy: ", "pain/discomfort: ", "focus cue: "),
    val hiddenOverviewTiles: Set<String> = emptySet(),
    val overviewTileOrder: List<String> = listOf("gym", "cardio", "trophies"),
    val dateFormat: String = "MMM d, yyyy",
    val timeFormat24h: Boolean = false,
    val firstDayMonday: Boolean = true,
    val hapticStrength: String = "strong",
    /** Default rest bases (seconds) per movement type — the Session-settings rest override. */
    val restCompoundSeconds: Int = 180,
    val restIsolationSeconds: Int = 90,
    val quietHoursEnabled: Boolean = false,
    val quietHoursStart: Int = 22,
    val quietHoursEnd: Int = 7,
    /** Daily training reminder (engagement) — opt-in, default off. */
    val trainingReminderEnabled: Boolean = false,
    val trainingReminderHour: Int = 18,
    val privacyMode: Boolean = false,
    val availableEquipment: Set<String> = emptySet(),
    /** Curated/frozen exercise pool (Developer's preset); null = ordinary equipment filtering. */
    val frozenExerciseIds: Set<String>? = null,
    val plateWeightLb: Double = 15.0,
    /** Heaviest dumbbell owned (lb); null = no ceiling (auto-coach Phase 0). */
    val maxDbWeightLb: Double? = null,
    val accentColorHex: String = "",
    val accentEmphasis: String = "off",
    val timezone: String = java.util.TimeZone.getDefault().id,
    val daysPerWeek: Int = 4,
    val liked: Set<String> = emptySet(),
    val disliked: Set<String> = emptySet(),
    val rotationCadence: String = "never",
    val rotationEveryN: Int = 4,
    val cardioWeeklyTargetMin: Int = 0,
    val userGoal: String = "build_muscle",
    /** User's sex ("male" | "female" | "") — selects the bodyweight-relative strength bands. */
    val userSex: String = "",
    val experience: String = "intermediate",
    val problemAreas: Set<String> = emptySet(),
    val priorityMuscles: Set<String> = emptySet(),
    val pinnedExercises: Set<String> = emptySet(),
    /** Coarse one-tap volume emphasis: "balanced" | "upper" | "legs" | "arms-shoulders". */
    val programEmphasis: String = "balanced",
    /** Coach mode (auto-coach Phase 4): "suggest" | "auto" (earned auto-apply). */
    val coachMode: String = "suggest",
    /** Current program's weekly sets per muscle (display name → sets), busiest first (Phase 6). */
    val weeklyVolume: List<Pair<String, Int>> = emptyList()
)

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val resetRepo: ResetRepository,
    private val backupRepo: com.forge.app.data.repo.BackupRepository,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder,
    private val pdfExport: com.forge.app.data.repo.PdfExportRepository,
    private val programRepository: com.forge.app.data.repo.ProgramRepository,
    private val vacationRepo: com.forge.app.data.repo.VacationRepository,
    private val coachRepo: com.forge.app.data.repo.CoachRepository,
    private val reminderScheduler: com.forge.app.service.ReminderScheduler,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard
) : ViewModel() {

    // ─── Coach (auto-coach Phase 4) ───────────────────────────────────────────

    private val _coachTrust = MutableStateFlow<List<com.forge.app.domain.coach.TypeTrust>>(emptyList())
    val coachTrust: StateFlow<List<com.forge.app.domain.coach.TypeTrust>> = _coachTrust.asStateFlow()

    private val _coachHistory =
        MutableStateFlow<List<com.forge.app.data.repo.CoachRepository.CoachHistoryEntry>>(emptyList())
    val coachHistory: StateFlow<List<com.forge.app.data.repo.CoachRepository.CoachHistoryEntry>> =
        _coachHistory.asStateFlow()

    /** Load the Coach page data (called when the section opens — not part of the big combine). */
    fun loadCoachData() = viewModelScope.launch {
        runCatching {
            _coachTrust.value = coachRepo.trust()
            _coachHistory.value = coachRepo.history()
        }
    }

    fun setCoachMode(mode: String) = viewModelScope.launch { settingsRepo.setCoachMode(mode) }

    fun undoCoachDecision(id: Long) = viewModelScope.launch {
        runCatching { coachRepo.undoDecision(id) }
        loadCoachData()
    }

    // ─── Day-aware scheduling (weekly plan vs sequence) ───────────────────────
    val scheduleMode: StateFlow<String> = settingsRepo.scheduleMode
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000),
            com.forge.app.domain.schedule.WeeklySchedule.MODE_SEQUENCE)

    val weeklySchedule: StateFlow<List<String>> = settingsRepo.weeklySchedule
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun setScheduleMode(mode: String) = viewModelScope.launch { settingsRepo.setScheduleMode(mode) }

    /** Assign [dayKey] ("" = rest) to weekday [weekdayIndex] (0=Mon..6=Sun) in the weekly schedule. */
    fun setScheduleDay(weekdayIndex: Int, dayKey: String) = viewModelScope.launch {
        val current = settingsRepo.weeklySchedule.first().toMutableList()
        while (current.size < com.forge.app.domain.schedule.WeeklySchedule.SLOTS) current.add("")
        current[weekdayIndex] = dayKey
        settingsRepo.setWeeklySchedule(current)
    }

    // ─── Holiday / vacation (#135) ────────────────────────────────────────────
    val vacations: StateFlow<List<com.forge.app.data.db.entities.VacationPeriod>> =
        vacationRepo.observeAll()
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun addVacation(startDate: String, endDate: String, label: String) =
        viewModelScope.launch { vacationRepo.add(startDate, endDate, label) }

    fun deleteVacation(period: com.forge.app.data.db.entities.VacationPeriod) =
        viewModelScope.launch { vacationRepo.delete(period) }

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
    }.combine(settingsRepo.compactSetLogging) { s, v ->
        s.copy(compactSetLogging = v)
    }.combine(settingsRepo.overviewTileOrder) { s, order ->
        s.copy(overviewTileOrder = order)
    }.combine(settingsRepo.privacyMode) { s, v ->
        s.copy(privacyMode = v)
    }.combine(settingsRepo.trainingReminderEnabled) { s, v ->
        s.copy(trainingReminderEnabled = v)
    }.combine(settingsRepo.trainingReminderHour) { s, v ->
        s.copy(trainingReminderHour = v)
    }.combine(settingsRepo.availableEquipment) { s, equip ->
        s.copy(availableEquipment = equip)
    }.combine(settingsRepo.frozenExerciseIds) { s, v ->
        s.copy(frozenExerciseIds = v)
    }.combine(settingsRepo.plateWeightLb) { s, v ->
        s.copy(plateWeightLb = v)
    }.combine(settingsRepo.maxDbWeightLb) { s, v ->
        s.copy(maxDbWeightLb = v)
    }.combine(settingsRepo.coachMode) { s, v ->
        s.copy(coachMode = v)
    }.combine(settingsRepo.accentColorHex) { s, v ->
        s.copy(accentColorHex = v)
    }.combine(settingsRepo.accentEmphasis) { s, v ->
        s.copy(accentEmphasis = v)
    }.combine(settingsRepo.timezone) { s, v ->
        s.copy(timezone = v)
    }.combine(settingsRepo.daysPerWeek) { s, v ->
        s.copy(daysPerWeek = v)
    }.combine(settingsRepo.likedExercises) { s, v ->
        s.copy(liked = v)
    }.combine(settingsRepo.dislikedExercises) { s, v ->
        s.copy(disliked = v)
    }.combine(settingsRepo.rotationCadence) { s, v ->
        s.copy(rotationCadence = v)
    }.combine(settingsRepo.rotationEveryN) { s, v ->
        s.copy(rotationEveryN = v)
    }.combine(settingsRepo.cardioWeeklyTargetMin) { s, v ->
        s.copy(cardioWeeklyTargetMin = v)
    }.combine(settingsRepo.restCompoundSeconds) { s, v ->
        s.copy(restCompoundSeconds = v)
    }.combine(settingsRepo.restIsolationSeconds) { s, v ->
        s.copy(restIsolationSeconds = v)
    }.combine(settingsRepo.userGoal) { s, v ->
        s.copy(userGoal = v.ifBlank { "build_muscle" })
    }.combine(settingsRepo.userSex) { s, v ->
        s.copy(userSex = v)
    }.combine(settingsRepo.programExperience) { s, v ->
        s.copy(experience = v)
    }.combine(settingsRepo.problemAreas) { s, v ->
        s.copy(problemAreas = v)
    }.combine(settingsRepo.priorityMuscles) { s, v ->
        s.copy(priorityMuscles = v)
    }.combine(settingsRepo.pinnedExercises) { s, v ->
        s.copy(pinnedExercises = v)
    }.combine(settingsRepo.programEmphasis) { s, v ->
        s.copy(programEmphasis = v)
    }.combine(programRepository.revision) { s, _ ->
        s.copy(weeklyVolume = computeWeeklyVolume())
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SettingsUiState())

    /** Sets-per-muscle across the active program (busiest first) — drives the Program page readout. */
    private fun computeWeeklyVolume(): List<Pair<String, Int>> =
        com.forge.app.program.Program.days
            .flatMap { it.exercises }
            .groupBy { it.muscle }
            .map { (muscle, exs) -> muscle to exs.sumOf { it.sets } }
            .sortedByDescending { it.second }
            .map { (muscle, sets) -> muscle.displayName to sets }

    fun setAmoledMode(v: Boolean) = viewModelScope.launch { settingsRepo.setAmoledMode(v) }
    fun setUseKg(v: Boolean) = viewModelScope.launch { settingsRepo.setUseKg(v) }
    fun setDateFormat(v: String) = viewModelScope.launch { settingsRepo.setDateFormat(v) }
    fun setTimeFormat24h(v: Boolean) = viewModelScope.launch { settingsRepo.setTimeFormat24h(v) }
    fun setFirstDayMonday(v: Boolean) = viewModelScope.launch { settingsRepo.setFirstDayMonday(v) }
    fun setHapticStrength(v: String) = viewModelScope.launch { settingsRepo.setHapticStrength(v) }
    fun setRestCompoundSeconds(s: Int) = viewModelScope.launch { settingsRepo.setRestCompoundSeconds(s) }
    fun setRestIsolationSeconds(s: Int) = viewModelScope.launch { settingsRepo.setRestIsolationSeconds(s) }
    fun addNoteTemplate(t: String) = viewModelScope.launch { settingsRepo.addNoteTemplate(t) }
    fun removeNoteTemplate(t: String) = viewModelScope.launch { settingsRepo.removeNoteTemplate(t) }
    fun setQuietHoursEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setQuietHoursEnabled(v) }
    fun setQuietHoursStart(v: Int) = viewModelScope.launch { settingsRepo.setQuietHoursStart(v) }
    fun setQuietHoursEnd(v: Int) = viewModelScope.launch { settingsRepo.setQuietHoursEnd(v) }

    fun setTrainingReminderEnabled(v: Boolean) = viewModelScope.launch {
        settingsRepo.setTrainingReminderEnabled(v)
        reminderScheduler.apply(v, settingsRepo.trainingReminderHour.first())
    }
    fun setTrainingReminderHour(h: Int) = viewModelScope.launch {
        settingsRepo.setTrainingReminderHour(h)
        // Re-arm only if reminders are on; otherwise just persist the preferred time for later.
        if (settingsRepo.trainingReminderEnabled.first()) reminderScheduler.apply(true, h.coerceIn(0, 23))
    }

    fun setTileHidden(id: String, hidden: Boolean) = viewModelScope.launch { settingsRepo.setTileHidden(id, hidden) }
    fun setCompactSetLogging(v: Boolean) = viewModelScope.launch { settingsRepo.setCompactSetLogging(v) }
    fun setCustomWarmup(dayKey: String, items: List<String>) =
        viewModelScope.launch { settingsRepo.setCustomWarmup(dayKey, items) }
    fun setOverviewTileOrder(order: List<String>) =
        viewModelScope.launch { settingsRepo.setOverviewTileOrder(order) }

    fun resetSessions() = viewModelScope.launch { resetRepo.resetSessions() }
    fun resetTrophies() = viewModelScope.launch { resetRepo.resetTrophies() }
    fun resetCardio() = viewModelScope.launch { resetRepo.resetCardio() }
    fun resetSettings() = viewModelScope.launch { resetRepo.resetAppSettings() }
    /** Scoped per-section "reset to defaults" (#544) — clears just this page's preferences. */
    fun resetSection(section: com.forge.app.data.prefs.SettingsSection) =
        viewModelScope.launch { settingsRepo.resetSection(section) }
    fun factoryReset() = viewModelScope.launch { resetRepo.factoryReset() }
    fun loadSampleData() = viewModelScope.launch { sampleDataSeeder.seed() }
    fun setPrivacyMode(v: Boolean) = viewModelScope.launch { settingsRepo.setPrivacyMode(v) }
    fun setAvailableEquipment(codes: Set<String>) = viewModelScope.launch {
        settingsRepo.setAvailableEquipment(codes)
        // Hand-editing the equipment set leaves any curated preset — drop the freeze.
        settingsRepo.setFrozenExerciseIds(null)
    }
    /** One-tap preset: fills the equipment set AND applies/clears its curated freeze together. */
    fun selectEquipmentPreset(preset: com.forge.app.program.EquipmentPreset) = viewModelScope.launch {
        settingsRepo.setAvailableEquipment(preset.equipment)
        settingsRepo.setFrozenExerciseIds(preset.frozenIds)
    }
    fun setPlateWeightLb(lb: Double) = viewModelScope.launch { settingsRepo.setPlateWeightLb(lb) }
    fun setDaysPerWeek(n: Int) = viewModelScope.launch { settingsRepo.setDaysPerWeek(n) }
    /** Weekly cardio-minutes goal for the cardio tab (no effect on the lifting plan). */
    fun setCardioWeeklyTargetMin(min: Int) = viewModelScope.launch { settingsRepo.setCardioWeeklyTargetMin(min) }
    /** All generation inputs read from prefs — keeps the three generate paths in sync (Phase 2 / 3). */
    private suspend fun buildParams(days: Int) = com.forge.app.program.GenerationParams(
        daysPerWeek = days,
        emphasis = settingsRepo.programEmphasis.first(),
        goal = settingsRepo.userGoal.first().ifBlank { "build_muscle" },
        experience = settingsRepo.programExperience.first(),
        problemAreas = settingsRepo.problemAreas.first()
            .mapNotNull { com.forge.app.program.ProblemArea.fromCode(it) }.toSet(),
        priorityMuscles = settingsRepo.priorityMuscles.first()
            .mapNotNull { runCatching { com.forge.app.program.MuscleGroup.fromCode(it) }.getOrNull() }.toSet(),
        pinned = settingsRepo.pinnedExercises.first(),
        dbMaxLb = settingsRepo.maxDbWeightLb.first(),
        frozenIds = settingsRepo.frozenExerciseIds.first()
    )
    private suspend fun currentEquipment(): Set<com.forge.app.program.Equipment> =
        settingsRepo.availableEquipment.first()
            .mapNotNull { runCatching { com.forge.app.program.Equipment.valueOf(it) }.getOrNull() }.toSet()

    fun toggleLike(libId: String) = viewModelScope.launch {
        settingsRepo.setExerciseLiked(libId, libId !in settingsRepo.likedExercises.first())
    }
    fun toggleDislike(libId: String) = viewModelScope.launch {
        settingsRepo.setExerciseDisliked(libId, libId !in settingsRepo.dislikedExercises.first())
    }
    fun setRotationCadence(cadence: String, n: Int) = viewModelScope.launch {
        settingsRepo.setRotationCadence(cadence)
        if (cadence == "every_n") settingsRepo.setRotationEveryN(n)
        settingsRepo.setRotationCounter(0)
    }
    fun rerollProgram() = viewModelScope.launch {
        programRepository.reroll(
            buildParams(settingsRepo.daysPerWeek.first()),
            currentEquipment(),
            settingsRepo.likedExercises.first(),
            settingsRepo.dislikedExercises.first()
        )
        _statusMessage.value = "Re-rolled — same split, fresh exercises. Open Gym to see it."
    }

    // Goal/experience/problem-areas/priority/pins are staged config — applied when the user taps
    // Generate or Re-roll (avoids reshuffling the whole plan on every chip tap).
    fun setUserGoal(goal: String) = viewModelScope.launch { settingsRepo.setUserGoal(goal) }
    fun setUserSex(sex: String) = viewModelScope.launch { settingsRepo.setUserSex(sex) }
    fun setMaxDbWeightLb(lb: Double?) = viewModelScope.launch { settingsRepo.setMaxDbWeightLb(lb) }
    fun setExperience(level: String) = viewModelScope.launch { settingsRepo.setProgramExperience(level) }
    fun setProgramEmphasis(v: String) = viewModelScope.launch { settingsRepo.setProgramEmphasis(v) }
    fun toggleProblemArea(code: String) = viewModelScope.launch {
        settingsRepo.toggleProblemArea(code, code !in settingsRepo.problemAreas.first())
    }
    fun togglePriorityMuscle(code: String) = viewModelScope.launch {
        settingsRepo.togglePriorityMuscle(code, code !in settingsRepo.priorityMuscles.first())
    }
    fun togglePin(libId: String) = viewModelScope.launch {
        settingsRepo.togglePinned(libId, libId !in settingsRepo.pinnedExercises.first())
    }

    /**
     * Generate a fresh program from the chosen day-count + current equipment/likes/dislikes (Phase 2).
     * Guarded: regenerating discards any in-progress workout, so confirm before doing it (the whole
     * change — including the day-count write — only commits past the guard).
     */
    fun generateProgram(days: Int) = viewModelScope.launch {
        programChangeGuard.run {
            settingsRepo.setDaysPerWeek(days)
            programRepository.generate(
                buildParams(days),
                currentEquipment(), settingsRepo.likedExercises.first(), settingsRepo.dislikedExercises.first()
            )
            _statusMessage.value = "New $days-day program generated. Open Gym to see it."
        }
    }

    /** Regenerate the current split at reduced volume for a recovery week (Phase 4 periodization). */
    fun generateDeloadWeek() = viewModelScope.launch {
        programChangeGuard.run {
            val days = settingsRepo.daysPerWeek.first()
            programRepository.generate(
                buildParams(days).copy(deload = true),
                currentEquipment(), settingsRepo.likedExercises.first(), settingsRepo.dislikedExercises.first()
            )
            _statusMessage.value = "Deload week generated — lighter volume. Open Gym to see it."
        }
    }
    fun setAccentColorHex(hex: String) = viewModelScope.launch { settingsRepo.setAccentColorHex(hex) }
    fun setAccentEmphasis(level: String) = viewModelScope.launch { settingsRepo.setAccentEmphasis(level) }
    fun setTimezone(id: String) = viewModelScope.launch { settingsRepo.setTimezone(id) }
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
        val file = backupRepo.exportFullDataJson()
        _exportPath.value = file.absolutePath
    }
    fun exportSessionsCsv() = viewModelScope.launch {
        val file = backupRepo.exportSessionsCsv()
        _exportPath.value = file.absolutePath
    }
    fun exportPrsCsv() = viewModelScope.launch {
        val file = backupRepo.exportPrsCsv()
        _exportPath.value = file.absolutePath
    }
    fun clearExportPath() { _exportPath.value = null }

    // ── Complete DB backup & restore (the real safety net) ─────────────────────
    private val _statusMessage = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val statusMessage: StateFlow<String?> = _statusMessage.asStateFlow()
    fun clearStatusMessage() { _statusMessage.value = null }

    /** Set true once a restore lands; the UI shows "restarting" and relaunches the app. */
    private val _restoreSucceeded = kotlinx.coroutines.flow.MutableStateFlow(false)
    val restoreSucceeded: StateFlow<Boolean> = _restoreSucceeded.asStateFlow()

    fun backupDatabase(uri: android.net.Uri) = viewModelScope.launch {
        runCatching { backupRepo.backupToUri(uri) }
            .onSuccess { _statusMessage.value = "Backup saved." }
            .onFailure { _statusMessage.value = "Backup failed: ${it.message}" }
    }

    fun restoreDatabase(uri: android.net.Uri) = viewModelScope.launch {
        val ok = runCatching { backupRepo.restoreFromUri(uri) }.getOrDefault(false)
        if (ok) _restoreSucceeded.value = true
        else _statusMessage.value = "Restore failed — that file isn't a valid Forge backup (or was made by a newer version)."
    }

    /** Formatted date of the weekly auto-backup slot, or null when none exists yet (#86). */
    private val _autoBackupSavedAt = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val autoBackupSavedAt: StateFlow<String?> = _autoBackupSavedAt.asStateFlow()

    /** Refresh the auto-backup date — call when the data dialog opens (the worker may have written since). */
    fun refreshAutoBackupInfo() {
        _autoBackupSavedAt.value = backupRepo.autoBackupSavedAtMs()?.let { ms ->
            java.text.SimpleDateFormat("MMM d, yyyy", java.util.Locale.getDefault()).format(java.util.Date(ms))
        }
    }

    /** In-app restore from the weekly auto-backup slot — the recovery path for the local auto-backup (#86). */
    fun restoreAutoBackup() = viewModelScope.launch {
        val ok = runCatching { backupRepo.restoreFromAutoBackup() }.getOrDefault(false)
        if (ok) _restoreSucceeded.value = true
        else _statusMessage.value = "No auto-backup to restore yet."
    }

    fun exportCrashLogs(uri: android.net.Uri) = viewModelScope.launch {
        runCatching { backupRepo.exportCrashLogsToUri(uri) }
            .onSuccess { n ->
                _statusMessage.value =
                    if (n == 0) "No crash logs yet — nothing to export." else "Exported $n crash log(s)."
            }
            .onFailure { _statusMessage.value = "Crash log export failed: ${it.message}" }
    }
}
