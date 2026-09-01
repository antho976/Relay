package com.forge.app.ui.checkin

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.CheckinRepository
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.onboarding.parseSaneBodyweightLb
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.SnackbarController
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject
import com.forge.app.domain.units.filterDecimalInput

/**
 * The daily check-in (Coach v3 B1). Four taps, all optional, opened from its notification.
 */
@HiltViewModel
class CheckinViewModel @Inject constructor(
    private val checkinRepo: CheckinRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val settingsRepo: SettingsRepository,
    private val snackbar: SnackbarController
) : ViewModel() {

    data class UiState(
        val visible: Boolean = false,
        val sleepQuality: Int? = null,
        val soreness: Int? = null,
        val stress: Int? = null,
        val motivation: Int? = null,
        val sick: Boolean = false,
        val soreMuscles: Set<MuscleGroup> = emptySet(),
        val weightText: String = "",
        /**
         * The unit the weight field is in. The field used to say only "Weight" and hand its number
         * straight to `logWeightOnly(weightLb)`, so a kg user typing 80 logged 80 POUNDS — a 44 kg
         * reading on their trend line, and a relative-strength denominator wrong by a factor of two.
         */
        val weightUnit: WeightUnit = WeightUnit.LB,
        /** Already answered today, so saving an opened historical state updates the same row. */
        val answeredToday: Boolean = false
    ) {
        /** The muscle picker only appears once soreness is real; one tap shouldn't open a menu. */
        val askWhichMuscles: Boolean get() = (soreness ?: 0) >= 4
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val answered = checkinRepo.today()?.hasAnswers == true
            _state.update { it.copy(answeredToday = answered) }
        }
        // Live, so the field's label follows a unit change made while the app is open.
        viewModelScope.launch {
            settingsRepo.weightUnit.collect { unit -> _state.update { it.copy(weightUnit = unit) } }
        }
    }

    /** Open today's check-in from its notification. */
    fun open() {
        viewModelScope.launch {
            val today = checkinRepo.today()
            _state.value = UiState(
                visible = true,
                // open() rebuilds the whole state, so carry the unit rather than resetting to lb.
                weightUnit = _state.value.weightUnit,
                sleepQuality = today?.sleepQuality,
                soreness = today?.soreness,
                stress = today?.stress,
                motivation = today?.motivation,
                sick = today?.sick == true,
                soreMuscles = today?.soreMuscles.orEmpty().split(",")
                    .mapNotNull { code -> MuscleGroup.entries.firstOrNull { it.code == code.trim() } }
                    .toSet(),
                answeredToday = today?.hasAnswers == true
            )
        }
    }

    fun setSleep(v: Int) = update { it.copy(sleepQuality = v) }
    fun setSoreness(v: Int) = update { it.copy(soreness = v) }
    fun setStress(v: Int) = update { it.copy(stress = v) }
    fun setMotivation(v: Int) = update { it.copy(motivation = v) }
    fun setSick(v: Boolean) = update { it.copy(sick = v) }
    fun setWeightText(v: String) = update { it.copy(weightText = filterDecimalInput(v)) }

    fun toggleMuscle(muscle: MuscleGroup) = update { s ->
        s.copy(soreMuscles = if (muscle in s.soreMuscles) s.soreMuscles - muscle else s.soreMuscles + muscle)
    }

    /**
     * Only a successful write closes the sheet and records the day as answered.
     *
     * The Result used to be discarded: on any failure the sheet closed, answeredToday flipped true —
     * the app's own record that the user has answered, which backs the prompt off — and nothing had
     * been written. runCatching swallows CancellationException too, so backing out of the sheet
     * mid-write did the same. A morning weigh-in entered here would vanish, taking the day's
     * bodyweight trend and readiness signal with it, and the user would never be asked again.
     * AdaptationRepository.snapshotOrEmpty and OverviewViewModel already re-throw cancellation for
     * exactly this reason.
     */
    fun save() {
        val s = _state.value
        viewModelScope.launch {
            runCatching {
                checkinRepo.save(
                    sleepQuality = s.sleepQuality,
                    soreness = s.soreness,
                    stress = s.stress,
                    motivation = s.motivation,
                    sick = s.sick,
                    // Muscles only mean something alongside a real soreness answer.
                    soreMuscles = if (s.askWhichMuscles) s.soreMuscles else emptySet()
                )
                // Morning is weigh-in time; logging it here saves a trip to the profile.
                //
                // Through the same parse the profile's weigh-in sheet and onboarding use, in the
                // user's display unit and bounded to a plausible adult weight. toDoubleOrNull()
                // meant this field was pounds no matter what the rest of the app was set to, and
                // accepted any number at all — a mis-typed "8" logged an 8 lb bodyweight straight
                // into the strength-standards denominator.
                parseSaneBodyweightLb(s.weightText, s.weightUnit)
                    ?.let { bodyweightRepo.logWeightOnly(it) }
            }.onSuccess {
                _state.update { it.copy(visible = false, answeredToday = true) }
            }.onFailure { e ->
                if (e is CancellationException) throw e
                snackbar.show("Couldn't save your check-in. Try again.")
            }
        }
    }

    fun close() = _state.update { it.copy(visible = false) }

    private fun update(block: (UiState) -> UiState) {
        _state.value = block(_state.value)
    }
}
