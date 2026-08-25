package com.forge.app.ui.checkin

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.CheckinRepository
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
 * The morning check-in (Coach v3 B1). Four taps, all optional, always skippable.
 *
 * The sheet asks once per day at first open and stops asking someone who never answers — the
 * repository owns that rule so no surface can re-implement it differently.
 */
@HiltViewModel
class CheckinViewModel @Inject constructor(
    private val checkinRepo: CheckinRepository,
    private val bodyweightRepo: BodyweightRepository,
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
        /** Already answered today — the manual entry point says "update" rather than "log". */
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
            val shouldPrompt = runCatching { checkinRepo.shouldPrompt() }.getOrDefault(false)
            _state.update { it.copy(visible = shouldPrompt, answeredToday = answered) }
        }
    }

    /** Open it by hand — the coach surfaces this once daily prompting has backed off. */
    fun open() {
        viewModelScope.launch {
            val today = checkinRepo.today()
            _state.value = UiState(
                visible = true,
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
                s.weightText.toDoubleOrNull()?.let { bodyweightRepo.logWeightOnly(it) }
            }.onSuccess {
                _state.update { it.copy(visible = false, answeredToday = true) }
            }.onFailure { e ->
                if (e is CancellationException) throw e
                snackbar.show("Couldn't save your check-in. Try again.")
            }
        }
    }

    fun skip() {
        viewModelScope.launch {
            runCatching { checkinRepo.skipToday() }
                .onSuccess { _state.update { st -> st.copy(visible = false) } }
                .onFailure { e ->
                    if (e is CancellationException) throw e
                    snackbar.show("Couldn't skip today's check-in. Try again.")
                }
        }
    }

    private fun update(block: (UiState) -> UiState) {
        _state.value = block(_state.value)
    }
}
