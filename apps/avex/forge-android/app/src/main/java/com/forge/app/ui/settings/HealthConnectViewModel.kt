package com.forge.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyFatRepository
import com.forge.app.data.repo.BodyweightRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
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
    private val bodyweightRepo: BodyweightRepository,
    private val bodyFatRepo: BodyFatRepository,
    private val leanMassRepo: com.forge.app.data.repo.LeanMassRepository,
    private val clock: Clock
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        /** A usable Health Connect provider is installed. */
        val available: Boolean = false,
        /** Provider present but needs a Play-store update before it works. */
        val needsUpdate: Boolean = false,
        /** Every recovery permission is granted. */
        val granted: Boolean = false,
        /** Bodyweight READ permission is granted — Avex may import a scale value (HC-2). */
        val weightGranted: Boolean = false,
        /** Write-back opt-in: mirror each weigh-in to Health Connect (HC-3). */
        val writeBodyweight: Boolean = false,
        /** Body-fat READ permission is granted — Avex may import a scale's body-fat % (GYMAP-62). */
        val bodyFatGranted: Boolean = false,
        /** Write-back opt-in: mirror each body-fat entry to Health Connect (GYMAP-62). */
        val writeBodyFat: Boolean = false,
        /** Active-calorie WRITE permission is granted — Avex may write session calories (HC-4). */
        val calorieGranted: Boolean = false,
        /** Write opt-in: mirror each finished session's estimated active calories to HC (HC-4). */
        val writeCalories: Boolean = false,
        /** Session WRITE permission is granted — Avex may write finished workouts to HC (W0). */
        val sessionGranted: Boolean = false,
        /** Write opt-in: mirror each finished gym + cardio session to Health Connect (W0). */
        val writeSessions: Boolean = false,
        /** Steps READ permission is granted — Avex may read a watch's step counts for the cardio graph. */
        val stepsGranted: Boolean = false,
        /** Exercise-session READ permission is granted — Avex may find watch sessions to offer GPS routes. */
        val exerciseGranted: Boolean = false,
        /** Heart-rate READ permission is granted — watch workouts show their HR graph + stats (W5). */
        val watchWorkoutGranted: Boolean = false,
        /** Lean-body-mass READ permission is granted — the watch's BIA reading can import (W6). */
        val leanMassGranted: Boolean = false,
        /** The user's watch ([com.forge.app.domain.health.WearableBrand] key; "" = never picked).
         *  Advisory — tailors the page's setup pointers, never gates a read. */
        val wearableBrand: String = "",
        /** Per-signal "is data actually arriving" reading (null until the probe resolves) — lets each
         *  granted row read "receiving" vs "nothing yet" instead of a bare "ON". */
        val signalFlow: HealthConnectManager.SignalFlow? = null,
        /** Transient one-tap-import result line, cleared on the next refresh. */
        val importMessage: String? = null,
        /** Transient one-tap-import result line for the body-fat row (GYMAP-62). */
        val bodyFatImportMessage: String? = null,
        /** Transient one-tap-import result line for the muscle-mass row (W6). */
        val leanMassImportMessage: String? = null
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    /** Permissions the recovery launcher should request (sleep + resting HR). */
    val permissions: Set<String> get() = manager.permissions

    /** Permissions the bodyweight launcher should request (read + write WeightRecord). */
    val weightPermissions: Set<String> get() = manager.weightPermissions

    /** Permissions the body-fat launcher should request (read + write BodyFatRecord). */
    val bodyFatPermissions: Set<String> get() = manager.bodyFatPermissions

    /** Permissions the calorie launcher should request (write ActiveCaloriesBurned). */
    val caloriePermissions: Set<String> get() = manager.caloriePermissions

    /** Permissions the workout-session launcher should request (write ExerciseSessionRecord, W0). */
    val sessionWritePermissions: Set<String> get() = manager.sessionWritePermissions

    /** Permissions the steps launcher should request (read StepsRecord). */
    val stepsPermissions: Set<String> get() = manager.stepsPermissions

    /** Permissions the GPS-routes launcher should request (read ExerciseSessionRecord). */
    val exercisePermissions: Set<String> get() = manager.exercisePermissions

    /** Permissions the watch-workouts launcher should request (read HR + distance + calories, W5). */
    val watchWorkoutPermissions: Set<String> get() = manager.watchWorkoutPermissions

    /** HRV read (W6) — requested WITH the recovery set by the sleep row (one concept, one row). */
    val hrvPermissions: Set<String> get() = manager.hrvPermissions

    /** Permissions the muscle-mass launcher should request (read LeanBodyMass, W6). */
    val leanMassPermissions: Set<String> get() = manager.leanMassPermissions

    init { refresh() }

    fun refresh() = viewModelScope.launch {
        val available = manager.isAvailable
        val granted = if (available) manager.hasAllPermissions() else false
        val weightGranted = if (available) manager.canReadWeight() else false
        val bodyFatGranted = if (available) manager.canReadBodyFat() else false
        val calorieGranted = if (available) manager.canWriteActiveCalories() else false
        val sessionGranted = if (available) manager.canWriteExerciseSessions() else false
        val stepsGranted = if (available) manager.canReadSteps() else false
        val exerciseGranted = if (available) manager.canReadExercise() else false
        val watchWorkoutGranted = if (available) manager.canReadHeartRate() else false
        val leanMassGranted = if (available) manager.canReadLeanMass() else false
        val writeBodyweight = settingsRepo.hcWriteBodyweight.first()
        val writeBodyFat = settingsRepo.hcWriteBodyFat.first()
        val writeCalories = settingsRepo.hcWriteCalories.first()
        val writeSessions = settingsRepo.hcWriteSessions.first()
        val wearableBrand = settingsRepo.wearableBrand.first()
        _state.update { it.copy(
            loading = false,
            available = available,
            needsUpdate = manager.needsUpdate,
            granted = granted,
            weightGranted = weightGranted,
            writeBodyweight = writeBodyweight,
            bodyFatGranted = bodyFatGranted,
            writeBodyFat = writeBodyFat,
            calorieGranted = calorieGranted,
            writeCalories = writeCalories,
            sessionGranted = sessionGranted,
            writeSessions = writeSessions,
            stepsGranted = stepsGranted,
            exerciseGranted = exerciseGranted,
            watchWorkoutGranted = watchWorkoutGranted,
            leanMassGranted = leanMassGranted,
            wearableBrand = wearableBrand
            // importMessage + signalFlow preserved (copy, not a fresh UiState): the import line so a
            // just-shown result isn't wiped by a lifecycle refresh; the prior reading so rows keep
            // "receiving"/"nothing yet" until the fresh probe below resolves, instead of flickering to "ON".
        ) }
        // Then probe what's actually arriving (a cheap single-row read per granted type) and fill the
        // reading in. Separate step so the grant state — and the whole page — renders without waiting on it.
        val flow = if (available) manager.probeSignalFlow(clock.nowMs()) else null
        _state.update { it.copy(signalFlow = flow) }

        // First time bodyweight READ is granted, backfill the WHOLE Health Connect weight history once
        // (GYMAP-63) — runs after the page has rendered so it never blocks the grant state. A pref gate
        // keeps it to a single pass; the repo's per-day dedup preserves any typed/previously-synced days.
        if (weightGranted && !settingsRepo.hcWeightHistoryImported.first()) {
            val imported = bodyweightRepo.importHistoryFromHealthConnect()
            // Latch ONLY on a successful read (non-null). A transient failure right after the grant
            // returns null → we leave the flag unset and retry on the next refresh (GYMAP-63).
            if (imported != null) {
                settingsRepo.setHcWeightHistoryImported(true)
                if (imported > 0) {
                    _state.update { it.copy(
                        importMessage = "Imported $imported day${if (imported == 1) "" else "s"} of weight history."
                    ) }
                }
            }
        }
    }

    fun setWearableBrand(key: String) = viewModelScope.launch {
        settingsRepo.setWearableBrand(key)
        _state.update { it.copy(wearableBrand = key) }
    }

    fun setWriteBodyweight(value: Boolean) = viewModelScope.launch {
        settingsRepo.setHcWriteBodyweight(value)
        _state.update { it.copy(writeBodyweight = value) }
    }

    fun setWriteCalories(value: Boolean) = viewModelScope.launch {
        settingsRepo.setHcWriteCalories(value)
        _state.update { it.copy(writeCalories = value) }
    }

    fun setWriteSessions(value: Boolean) = viewModelScope.launch {
        settingsRepo.setHcWriteSessions(value)
        _state.update { it.copy(writeSessions = value) }
    }

    fun setWriteBodyFat(value: Boolean) = viewModelScope.launch {
        settingsRepo.setHcWriteBodyFat(value)
        _state.update { it.copy(writeBodyFat = value) }
    }

    fun importNow() = viewModelScope.launch {
        val imported = bodyweightRepo.importLatestFromHealthConnect()
        _state.update { it.copy(
            importMessage = if (imported != null) "Imported your latest weight." else "No newer weight in Health Connect."
        ) }
    }

    fun importBodyFatNow() = viewModelScope.launch {
        val imported = bodyFatRepo.importLatestFromHealthConnect()
        _state.update { it.copy(
            bodyFatImportMessage = if (imported != null) "Imported your latest body fat." else "No newer body fat in Health Connect."
        ) }
    }

    fun importLeanMassNow() = viewModelScope.launch {
        val imported = leanMassRepo.importLatestFromHealthConnect()
        _state.update { it.copy(
            leanMassImportMessage = if (imported != null) "Imported your latest muscle mass." else "No newer muscle mass in Health Connect."
        ) }
    }
}
