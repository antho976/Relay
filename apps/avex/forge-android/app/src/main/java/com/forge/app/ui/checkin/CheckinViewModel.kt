package com.forge.app.ui.checkin

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.CheckinRepository
import com.forge.app.program.MuscleGroup
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The morning check-in (Coach v3 B1). Four taps, all optional, always skippable.
 *
 * Two surfaces, two flags. Once a day at first open the repository says to ask, and the top
 * [CheckinPromptBanner] carries the invitation ([prompting]); the sheet ([visible]) opens only on a
 * deliberate tap. Whether to ask at all stays in the repository so no surface re-implements it, and
 * it stops asking someone who never answers.
 */
@HiltViewModel
class CheckinViewModel @Inject constructor(
    private val checkinRepo: CheckinRepository,
    private val bodyweightRepo: BodyweightRepository
) : ViewModel() {

    data class UiState(
        /** The top banner is up — today's check-in is offered, nothing is blocked. */
        val prompting: Boolean = false,
        /** The sheet is up — asked for, never self-opened. */
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
            _state.value = _state.value.copy(
                prompting = runCatching { checkinRepo.shouldPrompt() }.getOrDefault(false),
                answeredToday = answered
            )
        }
    }

    /** Open the sheet — from the banner, or by hand once daily prompting has backed off. The fresh
     *  [UiState] also drops [UiState.prompting]: the invitation was taken. */
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
    fun setWeightText(v: String) = update { it.copy(weightText = v.filter { ch -> ch.isDigit() || ch == '.' }) }

    fun toggleMuscle(muscle: MuscleGroup) = update { s ->
        s.copy(soreMuscles = if (muscle in s.soreMuscles) s.soreMuscles - muscle else s.soreMuscles + muscle)
    }

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
                s.weightText.toDoubleOrNull()?.let { bodyweightRepo.log(it) }
            }
            _state.value = _state.value.copy(prompting = false, visible = false, answeredToday = true)
        }
    }

    /** Not today — from the banner's `×` or the sheet's own skip. Both record the dismissal, so the
     *  repository can learn to stop asking, and neither comes back before tomorrow. */
    fun skip() {
        viewModelScope.launch {
            runCatching { checkinRepo.skipToday() }
            _state.value = _state.value.copy(prompting = false, visible = false)
        }
    }

    private fun update(block: (UiState) -> UiState) {
        _state.value = block(_state.value)
    }
}
