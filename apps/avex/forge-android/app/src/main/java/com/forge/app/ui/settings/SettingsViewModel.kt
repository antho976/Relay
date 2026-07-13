package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.importer.userMessage
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BackupRepository.RestoreOutcome
import com.forge.app.data.repo.ResetRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject

data class SettingsUiState(
    val amoledMode: Boolean = false,
    /** Weight display unit (GYMAP-72) — lb | kg | st. */
    val weightUnit: com.forge.app.domain.units.WeightUnit = com.forge.app.domain.units.WeightUnit.LB,
    val useMiles: Boolean = false,
    val useCm: Boolean = false,
    val compactSetLogging: Boolean = false,
    val noteTemplates: Set<String> = setOf("form felt: ", "energy: ", "pain/discomfort: ", "focus cue: "),
    val hiddenOverviewTiles: Set<String> = emptySet(),
    val overviewTileOrder: List<String> = listOf("gym", "cardio", "trophies"),
    val dateFormat: String = "MMM d, yyyy",
    val timeFormat24h: Boolean = false,
    val firstDayMonday: Boolean = true,
    val hapticStrength: String = "strong",
    /** Hold the screen awake during an active session (GYMAP-74). Default on. */
    val keepScreenOn: Boolean = true,
    /** Default rest bases (seconds) per movement type — the Session-settings rest override. */
    val restCompoundSeconds: Int = 180,
    val restIsolationSeconds: Int = 90,
    val quietHoursEnabled: Boolean = false,
    val quietHoursSchedule: com.forge.app.domain.notify.QuietHoursSchedule =
        com.forge.app.domain.notify.QuietHoursSchedule.default(),
    /** Daily training reminder (engagement) — opt-in, default off. */
    val trainingReminderEnabled: Boolean = false,
    val trainingReminderHour: Int = 18,
    /** Per-type notification opt-outs (N2) — both default on. */
    val weeklyRecapEnabled: Boolean = true,
    val restTimerAlertEnabled: Boolean = true,
    val privacyMode: Boolean = false,
    /** App lock (GYMAP-69): require a biometric / device-credential unlock to open the app. */
    val appLockEnabled: Boolean = false,
    /** Gallery lock (GYMAP-69): require an unlock to view the progress-photo gallery. */
    val galleryLockEnabled: Boolean = false,
    /** Background grace before the app re-locks, in seconds (0 = immediately). */
    val appLockTimeoutSec: Int = 0,
    val availableEquipment: Set<String> = emptySet(),
    /** Curated/frozen exercise pool (Developer's preset); null = ordinary equipment filtering. */
    val frozenExerciseIds: Set<String>? = null,
    val plateWeightLb: Double = 15.0,
    /** Heaviest dumbbell owned (lb); null = no ceiling (auto-coach Phase 0). */
    val maxDbWeightLb: Double? = null,
    val accentColorHex: String = "",
    /** When false the accent is disabled app-wide — monochrome, neutral highlights. */
    val accentEnabled: Boolean = true,
    /** Selected launcher-icon enum name; "" = default emblem. Rings the current choice in the picker. */
    val appIconKey: String = "",
    /** Theme the cold-launch Avex intro to the chosen app icon (default on); off = plain B&W Avex. */
    val themedLaunchIntro: Boolean = true,
    val timezone: String = java.util.TimeZone.getDefault().id,
    /** IANA zone ids the user has starred — pinned to the top of the timezone picker. */
    val favoriteTimezones: Set<String> = emptySet(),
    val daysPerWeek: Int = 4,
    val liked: Set<String> = emptySet(),
    val disliked: Set<String> = emptySet(),
    /** User-created exercises (deduped by name), so they can be liked/disliked on the same screen. */
    val customExercises: List<com.forge.app.data.repo.CustomExerciseRef> = emptyList(),
    /** After a "Make default" swap, offer to dislike the swapped-out exercise (default on). */
    val swapDislikePromptEnabled: Boolean = true,
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
    /** "Go with the flow": no fixed plan; the home leads with freestyle logging instead of day cards. */
    val freestyleMode: Boolean = false,
    /** Whether the Coach feature is surfaced (tab + banners). Off hides it until re-enabled. */
    val coachEnabled: Boolean = true,
    /** Current program's weekly sets per muscle (display name → sets), busiest first (Phase 6). */
    val weeklyVolume: List<Pair<String, Int>> = emptyList()
) {
    /** Legacy convenience — true only for kilograms. Prefer [weightUnit]. */
    val useKg: Boolean get() = weightUnit == com.forge.app.domain.units.WeightUnit.KG
}

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val resetRepo: ResetRepository,
    private val backupRepo: com.forge.app.data.repo.BackupRepository,
    private val storageRepo: com.forge.app.data.repo.StorageRepository,
    private val importRepo: com.forge.app.data.importer.WorkoutImportRepository,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder,
    private val photoRepo: com.forge.app.data.repo.ProgressPhotoRepository,
    private val pdfExport: com.forge.app.data.repo.PdfExportRepository,
    private val programRepository: com.forge.app.data.repo.ProgramRepository,
    private val programCustomizationRepo: com.forge.app.data.repo.ProgramCustomizationRepository,
    private val vacationRepo: com.forge.app.data.repo.VacationRepository,
    private val coachRepo: com.forge.app.data.repo.CoachRepository,
    private val reminderScheduler: com.forge.app.service.ReminderScheduler,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard,
) : ViewModel() {

    // ─── Coach (auto-coach Phase 4) ───────────────────────────────────────────

    // Trust feeds only the coach-settings "auto isn't active yet" note; the ledger, history and
    // undo render on the Coach tab (CoachViewModel owns them).
    private val _coachTrust = MutableStateFlow<List<com.forge.app.domain.coach.TypeTrust>>(emptyList())
    val coachTrust: StateFlow<List<com.forge.app.domain.coach.TypeTrust>> = _coachTrust.asStateFlow()

    // The coach's input feeds (check-ins, rest flags, sleep, resting HR) with their live/silent
    // state — the settings page's at-a-glance "is everything it needs switched on".
    private val _coachSignals = MutableStateFlow<List<com.forge.app.data.repo.RecoverySignal>>(emptyList())
    val coachSignals: StateFlow<List<com.forge.app.data.repo.RecoverySignal>> = _coachSignals.asStateFlow()

    /** Load the Coach page's data (called when the page opens — not part of the big combine). */
    fun loadCoachData() = viewModelScope.launch {
        runCatching { _coachTrust.value = coachRepo.trust() }
        runCatching { _coachSignals.value = coachRepo.coachLab().recoverySignals }
    }

    fun setCoachMode(mode: String) = viewModelScope.launch { settingsRepo.setCoachMode(mode) }

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

    // ─── Custom cardio activity types (GYMAP-37) ──────────────────────────────
    // A standalone StateFlow (not the big combine) — list data managed like vacations above.
    val customCardioTypes: StateFlow<List<com.forge.app.domain.cardio.CustomCardioType>> =
        settingsRepo.customCardioTypes
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun addCustomCardioType(type: com.forge.app.domain.cardio.CustomCardioType) =
        viewModelScope.launch { settingsRepo.addCustomCardioType(type) }

    fun updateCustomCardioType(type: com.forge.app.domain.cardio.CustomCardioType) =
        viewModelScope.launch { settingsRepo.updateCustomCardioType(type) }

    fun deleteCustomCardioType(code: String) =
        viewModelScope.launch { settingsRepo.deleteCustomCardioType(code) }

    val state: StateFlow<SettingsUiState> = combine(
        settingsRepo.amoledMode,
        settingsRepo.weightUnit,
        settingsRepo.dateFormat,
        settingsRepo.timeFormat24h,
        settingsRepo.firstDayMonday,
        settingsRepo.hapticStrength,
        settingsRepo.quietHoursEnabled,
        settingsRepo.quietHoursSchedule
    ) { values ->
        SettingsUiState(
            amoledMode = values[0] as Boolean,
            weightUnit = values[1] as com.forge.app.domain.units.WeightUnit,
            dateFormat = values[2] as String,
            timeFormat24h = values[3] as Boolean,
            firstDayMonday = values[4] as Boolean,
            hapticStrength = values[5] as String,
            quietHoursEnabled = values[6] as Boolean,
            quietHoursSchedule = values[7] as com.forge.app.domain.notify.QuietHoursSchedule
        )
    }.combine(settingsRepo.noteTemplates) { s, templates ->
        s.copy(noteTemplates = templates)
    }.combine(settingsRepo.hiddenOverviewTiles) { s, hidden ->
        s.copy(hiddenOverviewTiles = hidden)
    }.combine(settingsRepo.compactSetLogging) { s, v ->
        s.copy(compactSetLogging = v)
    }.combine(settingsRepo.keepScreenOn) { s, v ->
        s.copy(keepScreenOn = v)
    }.combine(settingsRepo.overviewTileOrder) { s, order ->
        s.copy(overviewTileOrder = order)
    }.combine(settingsRepo.privacyMode) { s, v ->
        s.copy(privacyMode = v)
    }.combine(settingsRepo.appLockEnabled) { s, v ->
        s.copy(appLockEnabled = v)
    }.combine(settingsRepo.galleryLockEnabled) { s, v ->
        s.copy(galleryLockEnabled = v)
    }.combine(settingsRepo.appLockTimeoutSec) { s, v ->
        s.copy(appLockTimeoutSec = v)
    }.combine(settingsRepo.trainingReminderEnabled) { s, v ->
        s.copy(trainingReminderEnabled = v)
    }.combine(settingsRepo.trainingReminderHour) { s, v ->
        s.copy(trainingReminderHour = v)
    }.combine(settingsRepo.weeklyRecapEnabled) { s, v ->
        s.copy(weeklyRecapEnabled = v)
    }.combine(settingsRepo.restTimerAlertEnabled) { s, v ->
        s.copy(restTimerAlertEnabled = v)
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
    }.combine(settingsRepo.accentEnabled) { s, v ->
        s.copy(accentEnabled = v)
    }.combine(settingsRepo.appIcon) { s, v ->
        s.copy(appIconKey = v)
    }.combine(settingsRepo.themedLaunchIntro) { s, v ->
        s.copy(themedLaunchIntro = v)
    }.combine(settingsRepo.timezone) { s, v ->
        s.copy(timezone = v)
    }.combine(settingsRepo.favoriteTimezones) { s, v ->
        s.copy(favoriteTimezones = v)
    }.combine(settingsRepo.daysPerWeek) { s, v ->
        s.copy(daysPerWeek = v)
    }.combine(settingsRepo.likedExercises) { s, v ->
        s.copy(liked = v)
    }.combine(settingsRepo.dislikedExercises) { s, v ->
        s.copy(disliked = v)
    }.combine(settingsRepo.swapDislikePromptEnabled) { s, v ->
        s.copy(swapDislikePromptEnabled = v)
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
    }.combine(settingsRepo.freestyleMode) { s, v ->
        s.copy(freestyleMode = v)
    }.combine(settingsRepo.coachEnabled) { s, v ->
        s.copy(coachEnabled = v)
    }.combine(settingsRepo.useMiles) { s, v ->
        s.copy(useMiles = v)
    }.combine(settingsRepo.useCm) { s, v ->
        s.copy(useCm = v)
    }.combine(programRepository.revision) { s, _ ->
        s.copy(weeklyVolume = computeWeeklyVolume())
    }.combine(programCustomizationRepo.observeCustomExercises()) { s, v ->
        s.copy(customExercises = v)
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
    fun setWeightUnit(u: com.forge.app.domain.units.WeightUnit) =
        viewModelScope.launch { settingsRepo.setWeightUnit(u) }
    fun setUseMiles(v: Boolean) = viewModelScope.launch { settingsRepo.setUseMiles(v) }
    fun setUseCm(v: Boolean) = viewModelScope.launch { settingsRepo.setUseCm(v) }
    fun setDateFormat(v: String) = viewModelScope.launch { settingsRepo.setDateFormat(v) }
    fun setTimeFormat24h(v: Boolean) = viewModelScope.launch { settingsRepo.setTimeFormat24h(v) }
    fun setFirstDayMonday(v: Boolean) = viewModelScope.launch { settingsRepo.setFirstDayMonday(v) }
    fun setHapticStrength(v: String) = viewModelScope.launch { settingsRepo.setHapticStrength(v) }
    fun setKeepScreenOn(v: Boolean) = viewModelScope.launch { settingsRepo.setKeepScreenOn(v) }
    fun setRestCompoundSeconds(s: Int) = viewModelScope.launch { settingsRepo.setRestCompoundSeconds(s) }
    fun setRestIsolationSeconds(s: Int) = viewModelScope.launch { settingsRepo.setRestIsolationSeconds(s) }
    fun addNoteTemplate(t: String) = viewModelScope.launch { settingsRepo.addNoteTemplate(t) }
    fun removeNoteTemplate(t: String) = viewModelScope.launch { settingsRepo.removeNoteTemplate(t) }
    fun setQuietHoursEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setQuietHoursEnabled(v) }
    fun setQuietWindow(day: java.time.DayOfWeek, start: Int, end: Int) =
        viewModelScope.launch { settingsRepo.setQuietWindow(day, start, end) }

    fun setTrainingReminderEnabled(v: Boolean) = viewModelScope.launch {
        settingsRepo.setTrainingReminderEnabled(v)
        reminderScheduler.apply(v, settingsRepo.trainingReminderHour.first())
    }
    fun setTrainingReminderHour(h: Int) = viewModelScope.launch {
        settingsRepo.setTrainingReminderHour(h)
        // Re-arm only if reminders are on; otherwise just persist the preferred time for later.
        if (settingsRepo.trainingReminderEnabled.first()) reminderScheduler.apply(true, h.coerceIn(0, 23))
    }
    fun setWeeklyRecapEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setWeeklyRecapEnabled(v) }
    fun setRestTimerAlertEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setRestTimerAlertEnabled(v) }

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
    // App / gallery lock (GYMAP-69). Enabling is gated on an available device credential in the UI
    // (Security page), so these persist the choice directly.
    fun setAppLockEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setAppLockEnabled(v) }
    fun setGalleryLockEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setGalleryLockEnabled(v) }
    fun setAppLockTimeoutSec(v: Int) = viewModelScope.launch { settingsRepo.setAppLockTimeoutSec(v) }
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
    /** Toggle "go with the flow" (no fixed plan; home leads with freestyle logging). */
    fun setFreestyleMode(v: Boolean) = viewModelScope.launch { settingsRepo.setFreestyleMode(v) }
    /** Show/hide the Coach feature (tab + banners). */
    fun setCoachEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setCoachEnabled(v) }
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

    /**
     * Toggle the like/dislike state for one or more ids at once. A library exercise passes its single
     * id; a custom exercise passes every `custom_…` id sharing its name so they flip together. The
     * toggle decision is made against the freshly persisted set inside the DataStore edit (not the UI
     * snapshot), so a rapid double-tap cycles the chip correctly instead of no-op'ing the second tap.
     */
    fun toggleExercisesLiked(ids: Set<String>) = viewModelScope.launch {
        settingsRepo.toggleExercisesLiked(ids)
    }
    fun toggleExercisesDisliked(ids: Set<String>) = viewModelScope.launch {
        settingsRepo.toggleExercisesDisliked(ids)
    }
    fun setSwapDislikePromptEnabled(enabled: Boolean) = viewModelScope.launch {
        settingsRepo.setSwapDislikePromptEnabled(enabled)
    }
    fun setRotationCadence(cadence: String, n: Int) = viewModelScope.launch {
        settingsRepo.setRotationCadence(cadence)
        if (cadence == "every_n") settingsRepo.setRotationEveryN(n)
        settingsRepo.setRotationCounter(0)
    }
    fun rerollProgram() = viewModelScope.launch {
        // Re-rolling produces a concrete plan, so a "go with the flow" user is now following one —
        // flip freestyle off, else the fresh plan stays hidden behind the freestyle home.
        val wasFreestyle = settingsRepo.freestyleMode.first()
        programRepository.reroll(
            buildParams(settingsRepo.daysPerWeek.first()),
            currentEquipment(),
            settingsRepo.likedExercises.first(),
            settingsRepo.dislikedExercises.first()
        )
        if (wasFreestyle) settingsRepo.setFreestyleMode(false)
        _statusMessage.value = if (wasFreestyle)
            "Re-rolled. You're now following a plan. Open Gym to see it."
        else
            "Re-rolled. Same split, fresh exercises. Open Gym to see it."
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
        // Generating gives the user a real plan — leave freestyle so the home actually surfaces it.
        val wasFreestyle = settingsRepo.freestyleMode.first()
        programChangeGuard.run {
            settingsRepo.setDaysPerWeek(days)
            programRepository.generate(
                buildParams(days),
                currentEquipment(), settingsRepo.likedExercises.first(), settingsRepo.dislikedExercises.first()
            )
            if (wasFreestyle) settingsRepo.setFreestyleMode(false)
            _statusMessage.value = if (wasFreestyle)
                "New $days-day program generated. You're now following a plan. Open Gym to see it."
            else
                "New $days-day program generated. Open Gym to see it."
        }
    }

    /** Regenerate the current split at reduced volume for a recovery week (Phase 4 periodization). */
    fun generateDeloadWeek() = viewModelScope.launch {
        val wasFreestyle = settingsRepo.freestyleMode.first()
        programChangeGuard.run {
            val days = settingsRepo.daysPerWeek.first()
            programRepository.generate(
                buildParams(days).copy(deload = true),
                currentEquipment(), settingsRepo.likedExercises.first(), settingsRepo.dislikedExercises.first()
            )
            if (wasFreestyle) settingsRepo.setFreestyleMode(false)
            _statusMessage.value = "Deload week generated at lighter volume. Open Gym to see it."
        }
    }
    fun setAccentColorHex(hex: String) = viewModelScope.launch { settingsRepo.setAccentColorHex(hex) }
    fun setAccentEnabled(enabled: Boolean) = viewModelScope.launch { settingsRepo.setAccentEnabled(enabled) }

    /** Persist the pick, which rings it in the picker immediately. The actual launcher-alias swap is
     *  deferred to a user-initiated app-background by [com.forge.app.MainActivity]'s onStop (gated on
     *  onUserLeaveHint): toggling the alias here, in the foreground, disables the alias backing the
     *  current task and closes the app on some OEMs. See [com.forge.app.appicon.AppIconManager]. */
    fun setAppIcon(icon: com.forge.app.appicon.AppIcon) = viewModelScope.launch {
        settingsRepo.setAppIcon(icon.name)
    }
    fun setThemedLaunchIntro(v: Boolean) = viewModelScope.launch { settingsRepo.setThemedLaunchIntro(v) }
    fun setTimezone(id: String) = viewModelScope.launch { settingsRepo.setTimezone(id) }
    fun toggleFavoriteTimezone(id: String) = viewModelScope.launch { settingsRepo.toggleFavoriteTimezone(id) }
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
    fun exportBodyweightCsv() = viewModelScope.launch {
        val file = backupRepo.exportBodyweightCsv()
        _exportPath.value = file.absolutePath
    }
    fun exportCardioCsv() = viewModelScope.launch {
        val file = backupRepo.exportCardioCsv()
        _exportPath.value = file.absolutePath
    }
    fun clearExportPath() { _exportPath.value = null }

    // ── Import from another gym app (#GYMAP-17) ────────────────────────────────
    /** A short message shown after an import finishes (success summary or a reason it didn't). */
    private val _importResult = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val importResult: StateFlow<String?> = _importResult.asStateFlow()
    fun clearImportResult() { _importResult.value = null }

    fun importData(uri: android.net.Uri) = viewModelScope.launch {
        val result = runCatching { importRepo.import(uri) }
            .getOrDefault(com.forge.app.data.importer.ImportResult.ReadError)
        _importResult.value = result.userMessage()
        // A successful merge changed the data set — refresh the backup nudge / db size readout, and
        // rescan the folder so its found-file counts reflect what's now imported.
        if (result is com.forge.app.data.importer.ImportResult.Success) {
            refreshAutoBackupInfo()
            scanImportFolder()
        }
    }

    /** True once the user has granted a folder (Downloads) for the Import screen to auto-scan. */
    val importFolderGranted: StateFlow<Boolean> =
        settingsRepo.importFolderUri.map { it != null }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), false)

    /** Gym-app exports found in the granted folder, newest first — the one-tap import list. */
    private val _foundImports =
        MutableStateFlow<List<com.forge.app.data.importer.FoundImport>>(emptyList())
    val foundImports: StateFlow<List<com.forge.app.data.importer.FoundImport>> = _foundImports.asStateFlow()

    private val _scanningImports = MutableStateFlow(false)
    val scanningImports: StateFlow<Boolean> = _scanningImports.asStateFlow()

    /** Persist the folder the user just granted, then scan it. */
    fun grantImportFolder(uri: android.net.Uri) = viewModelScope.launch {
        importRepo.rememberFolder(uri)
        scanImportFolder()
    }

    /** Scan the remembered folder for importable exports (no-op when no folder is granted yet). */
    fun scanImportFolder() = viewModelScope.launch {
        val folder = settingsRepo.importFolderUri.first()
            ?: run { _foundImports.value = emptyList(); return@launch }
        _scanningImports.value = true
        _foundImports.value =
            runCatching { importRepo.scanFolder(android.net.Uri.parse(folder)) }.getOrDefault(emptyList())
        _scanningImports.value = false
    }

    // ── Complete DB backup & restore (the real safety net) ─────────────────────
    private val _statusMessage = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val statusMessage: StateFlow<String?> = _statusMessage.asStateFlow()
    fun clearStatusMessage() { _statusMessage.value = null }

    /** Set true once a restore lands; the UI shows "restarting" and relaunches the app. */
    private val _restoreSucceeded = kotlinx.coroutines.flow.MutableStateFlow(false)
    val restoreSucceeded: StateFlow<Boolean> = _restoreSucceeded.asStateFlow()

    fun backupDatabase(uri: android.net.Uri) = viewModelScope.launch {
        runCatching { backupRepo.backupToUri(uri) }
            .onSuccess {
                _statusMessage.value = "Backup saved."
                // backupToUri cleared the failure marker; reflect that in the StateFlow so any
                // open "last auto-backup failed" notice disappears without reopening the dialog.
                refreshAutoBackupInfo()
            }
            .onFailure { _statusMessage.value = "Backup failed: ${it.message}" }
    }

    fun restoreDatabase(uri: android.net.Uri) = viewModelScope.launch {
        val outcome = runCatching { backupRepo.restoreFromUri(uri) }
            .getOrDefault(RestoreOutcome.IO_ERROR)
        if (outcome == RestoreOutcome.SUCCESS) _restoreSucceeded.value = true
        else _statusMessage.value = restoreFailureMessage(outcome)
    }

    /** Plain-English reason for a failed restore, so the user knows what to do next (E6). */
    private fun restoreFailureMessage(outcome: RestoreOutcome): String =
        when (outcome) {
            RestoreOutcome.NOT_A_BACKUP ->
                "That file isn't an Avex backup. Pick the .zip you exported from Avex."
            RestoreOutcome.NEWER_VERSION ->
                "That backup was made by a newer version of Avex. Update the app, then restore."
            RestoreOutcome.TOO_OLD ->
                "That backup is too old to restore safely (made before this app's schema)."
            RestoreOutcome.CORRUPT ->
                "That backup is corrupted or incomplete. Try a different copy or re-export it."
            RestoreOutcome.TOO_LARGE ->
                "That file is too large to be an Avex backup."
            RestoreOutcome.NO_BACKUP_FILE ->
                "No auto-backup to restore yet."
            RestoreOutcome.IO_ERROR ->
                "Couldn't read that file. Try again, or pick a different copy."
            RestoreOutcome.SUCCESS -> ""
        }

    /** Formatted date of the weekly auto-backup slot, or null when none exists yet (#86). */
    private val _autoBackupSavedAt = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val autoBackupSavedAt: StateFlow<String?> = _autoBackupSavedAt.asStateFlow()

    /** True when the weekly auto-backup worker gave up (e.g. storage full) and none has succeeded since. */
    private val _autoBackupFailed = kotlinx.coroutines.flow.MutableStateFlow(false)
    val autoBackupFailed: StateFlow<Boolean> = _autoBackupFailed.asStateFlow()

    /** True when there's data worth protecting but no backup exists yet — drives a Settings nudge (#5 P1). */
    private val _noBackupWarning = kotlinx.coroutines.flow.MutableStateFlow(false)
    val noBackupWarning: StateFlow<Boolean> = _noBackupWarning.asStateFlow()

    /** Summary of the live data a restore would overwrite, e.g. "12 sessions · 5 progress photos". */
    private val _restoreImpact = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    val restoreImpact: StateFlow<String?> = _restoreImpact.asStateFlow()

    /** On-disk database size, human-readable ("2.4 MB") — the Data dialog readout. */
    private val _dbSizeLabel = kotlinx.coroutines.flow.MutableStateFlow("")
    val dbSizeLabel: StateFlow<String> = _dbSizeLabel.asStateFlow()

    // ── Storage breakdown + cache clear (GYMAP-68) ─────────────────────────────
    /** The per-category on-disk footprint for the Storage page; null until first loaded. */
    private val _storage = kotlinx.coroutines.flow.MutableStateFlow<com.forge.app.data.repo.StorageBreakdown?>(null)
    val storage: StateFlow<com.forge.app.data.repo.StorageBreakdown?> = _storage.asStateFlow()

    /** (Re)measure on-disk storage — call when the Storage page opens or after clearing the cache. */
    fun refreshStorage() = viewModelScope.launch { _storage.value = storageRepo.breakdown() }

    /** Wipe the transient cache, then re-measure so the page reflects the freed space (the shrinking
     *  Cache row IS the feedback, DESIGN §13 — no success toast). */
    fun clearCache() = viewModelScope.launch {
        runCatching { storageRepo.clearCache() }
        _storage.value = storageRepo.breakdown()
    }

    /** Refresh the auto-backup date + failure state — call when the data dialog opens (the worker may have run since). */
    fun refreshAutoBackupInfo() = viewModelScope.launch {
        // All checks are File / DB reads — keep them off the main thread.
        data class Info(val savedAtMs: Long?, val failed: Boolean, val noBackup: Boolean, val impact: String, val dbSize: Long)
        val info = withContext(Dispatchers.IO) {
            Info(
                backupRepo.autoBackupSavedAtMs(),
                backupRepo.autoBackupFailed(),
                backupRepo.shouldWarnNoBackup(),
                backupRepo.restoreImpactSummary(),
                backupRepo.dbSizeBytes()
            )
        }
        _autoBackupSavedAt.value = info.savedAtMs?.let { formatMediumDate(it) }
        _autoBackupFailed.value = info.failed
        _noBackupWarning.value = info.noBackup
        _restoreImpact.value = info.impact
        _dbSizeLabel.value = formatBytes(info.dbSize)
    }

    // ── Progress-photo info (tasks 3 + 4: factory-reset warning + data-dialog stake indicator) ──────
    /** Count of saved progress photos — loaded once per data-dialog open and on VM init. */
    private val _photoCount = kotlinx.coroutines.flow.MutableStateFlow(0)
    val photoCount: StateFlow<Int> = _photoCount.asStateFlow()

    /** Timestamp (ms) of the most recently taken progress photo, or null when there are none. */
    private val _photoLastTakenMs = kotlinx.coroutines.flow.MutableStateFlow<Long?>(null)
    val photoLastTakenMs: StateFlow<Long?> = _photoLastTakenMs.asStateFlow()

    /** Refresh progress-photo stats — call when the data dialog opens so the count is fresh. */
    fun refreshPhotoInfo() = viewModelScope.launch {
        val photos = runCatching { photoRepo.photos() }.getOrDefault(emptyList())
        _photoCount.value = photos.size
        _photoLastTakenMs.value = photos.maxOfOrNull { it.takenAtMs }
    }

    /** In-app restore from the weekly auto-backup slot — the recovery path for the local auto-backup (#86). */
    fun restoreAutoBackup() = viewModelScope.launch {
        val outcome = runCatching { backupRepo.restoreFromAutoBackup() }
            .getOrDefault(RestoreOutcome.IO_ERROR)
        if (outcome == RestoreOutcome.SUCCESS) _restoreSucceeded.value = true
        else _statusMessage.value = restoreFailureMessage(outcome)
    }

    // ── Auto-backup config + manual "Back up now" (GYMAP-67) ───────────────────
    /** Whether the weekly auto-backup is on (default true) — the Backup page toggle. */
    val autoBackupEnabled: StateFlow<Boolean> = settingsRepo.autoBackupEnabled
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), true)

    /** The user-picked folder backups also write to, or null when internal-only. */
    val backupFolderUri: StateFlow<String?> = settingsRepo.backupFolderUri
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    fun setAutoBackupEnabled(v: Boolean) = viewModelScope.launch { settingsRepo.setAutoBackupEnabled(v) }

    /** Persist + take a write grant on the picked folder, then seed it with a backup right away so it
     *  isn't empty until the next weekly run. */
    fun setBackupFolder(uri: android.net.Uri) = viewModelScope.launch {
        backupRepo.rememberBackupFolder(uri)
        backupNow()
    }
    fun clearBackupFolder() = viewModelScope.launch { backupRepo.forgetBackupFolder() }

    /** Run a backup right now — to internal storage and the picked folder — the "Back up now" action. */
    fun backupNow() = viewModelScope.launch {
        val folder = settingsRepo.backupFolderUri.first()?.let { android.net.Uri.parse(it) }
        runCatching { backupRepo.autoBackup(folder) }
            .onSuccess { _statusMessage.value = "Backed up."; refreshAutoBackupInfo() }
            .onFailure { _statusMessage.value = "Backup failed: ${it.message}" }
    }

    fun exportCrashLogs(uri: android.net.Uri) = viewModelScope.launch {
        runCatching { backupRepo.exportCrashLogsToUri(uri) }
            .onSuccess { n ->
                _statusMessage.value =
                    if (n == 0) "No crash logs yet, nothing to export."
                    else "Exported $n crash log${if (n == 1) "" else "s"}."
            }
            .onFailure { _statusMessage.value = "Crash log export failed: ${it.message}" }
    }

    // ── In-app crash log viewer ────────────────────────────────────────────────
    // null = not loaded yet (show "Loading…"); emptyList = loaded, no crashes (show the all-clear).
    // Distinguishing the two avoids a false "No crashes recorded" flash before the IO read returns.
    private val _crashLogs = MutableStateFlow<List<Pair<String, String>>?>(null)
    val crashLogs: StateFlow<List<Pair<String, String>>?> = _crashLogs.asStateFlow()

    /** Load the most recent crash logs from filesDir/crashes — call when the viewer opens. */
    fun loadCrashLogs() = viewModelScope.launch {
        _crashLogs.value = withContext(Dispatchers.IO) {
            backupRepo.readRecentCrashLogs()
        }
    }
}
