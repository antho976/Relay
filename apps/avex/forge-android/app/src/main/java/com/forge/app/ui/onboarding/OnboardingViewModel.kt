package com.forge.app.ui.onboarding

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.program.Equipment
import com.forge.app.program.GeneratedDay
import com.forge.app.program.GenerationParams
import com.forge.app.program.ProblemArea
import com.forge.app.program.ProgramGenerator
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Plan-mode keys chosen on the onboarding plan-mode step. */
const val PLAN_GENERATED = "generated"
const val PLAN_CUSTOM = "custom"
const val PLAN_FREESTYLE = "freestyle"

/** The saved resume draft, tri-state: don't compose the flow until the one-shot read lands. */
internal sealed interface DraftLoad {
    data object Loading : DraftLoad
    data class Ready(val draft: OnboardingDraft?) : DraftLoad
}

@HiltViewModel
class OnboardingViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val programRepository: ProgramRepository
) : ViewModel() {

    private val _draftLoad = MutableStateFlow<DraftLoad>(DraftLoad.Loading)
    internal val draftLoad: StateFlow<DraftLoad> = _draftLoad

    private val pendingDraft = MutableStateFlow<OnboardingDraft?>(null)

    /** Flipped off the moment [complete] runs so a conflated save can't resurrect the draft the
     *  atomic completion write just removed. */
    @Volatile
    private var draftWritesEnabled = true

    init {
        viewModelScope.launch {
            _draftLoad.value = DraftLoad.Ready(settingsRepo.onboardingDraft()?.let(OnboardingDraft::fromJson))
        }
        // Conflated autosave: rapid changes (typing a name) collapse into one write ~250ms after
        // the last keystroke — collectLatest cancels the stale snapshots.
        viewModelScope.launch {
            pendingDraft.filterNotNull().collectLatest { draft ->
                delay(250)
                if (draftWritesEnabled) settingsRepo.saveOnboardingDraft(draft.toJson())
            }
        }
    }

    /** Queue the current answers for persistence (see init) — cheap to call on every change. */
    internal fun saveDraft(draft: OnboardingDraft) {
        pendingDraft.value = draft
    }

    /** Pure, side-effect-free week for the preview step — the same [seed] is persisted on finish. */
    fun buildPreview(
        daysPerWeek: Int, equipment: Set<String>, goal: String, experience: String,
        problemAreas: Set<String>, frozenIds: Set<String>?, seed: Long
    ): List<GeneratedDay> = ProgramGenerator.generate(
        GenerationParams(
            daysPerWeek = daysPerWeek, goal = goal, experience = experience,
            problemAreas = problemAreas.mapNotNull { ProblemArea.fromCode(it) }.toSet(),
            frozenIds = frozenIds
        ),
        equipment.mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet(),
        emptySet(), emptySet(), seed = seed
    )

    /**
     * Finish onboarding. [planMode] drives what program (if any) is built:
     * - [PLAN_GENERATED]: persist the previewed week (all generator inputs supplied).
     * - [PLAN_CUSTOM]: clear the program so the builder opens blank — the user constructs their days.
     * - [PLAN_FREESTYLE]: clear the program too (no fixed plan) and flip [setFreestyleMode] so the home
     *   leads with freestyle logging instead of day cards.
     */
    fun complete(
        planMode: String,
        name: String,
        useKg: Boolean,
        sex: String,
        bodyweightLb: Double?,
        /** Explicit cardio distance choice; null = leave tied to the weight unit (lb→miles, kg→km). */
        useMiles: Boolean? = null,
        // [goal] + [experience] are collected on EVERY path (they steer the coach's rep-range/volume
        // suggestions). The rest of these are generated-only; custom/freestyle default them.
        goal: String = "build_muscle",
        daysPerWeek: Int = 4,
        equipment: Set<String> = emptySet(),
        cadence: String = "never",
        everyN: Int = 4,
        experience: String = "intermediate",
        problemAreas: Set<String> = emptySet(),
        seed: Long = System.nanoTime(),
        plateWeightLb: Double = 15.0,
        frozenIds: Set<String>? = null,
        coachEnabled: Boolean = true
    ) {
        // Stop the resume-draft autosaver before the completion write removes the draft.
        draftWritesEnabled = false
        viewModelScope.launch {
            bodyweightLb?.let { bodyweightRepo.log(it) }
            settingsRepo.setPlateWeightLb(plateWeightLb)
            settingsRepo.setUserSex(sex)
            settingsRepo.setFreestyleMode(planMode == PLAN_FREESTYLE)
            settingsRepo.setCoachEnabled(coachEnabled)

            val effectiveGoal: String
            when (planMode) {
                PLAN_GENERATED -> {
                    effectiveGoal = goal
                    settingsRepo.setDaysPerWeek(daysPerWeek)
                    settingsRepo.setAvailableEquipment(equipment)
                    settingsRepo.setFrozenExerciseIds(frozenIds)
                    settingsRepo.setRotationCadence(cadence)
                    if (cadence == "every_n") settingsRepo.setRotationEveryN(everyN)
                    settingsRepo.setProgramExperience(experience)
                    settingsRepo.setUserGoal(goal)
                    problemAreas.forEach { settingsRepo.toggleProblemArea(it, true) }
                    // Persist exactly the week shown in the preview (same seed + inputs).
                    generateProgram(daysPerWeek, goal, experience, problemAreas, equipment, frozenIds, seed)
                }
                PLAN_CUSTOM -> {
                    // Build-your-own starts with a genuinely EMPTY plan — the builder opens blank and
                    // the user constructs their days/exercises from scratch. Goal + experience ARE
                    // collected (they steer the coach's rep-range/volume suggestions on the plan you
                    // build); the rest default so a later Settings → Generate works if you want it.
                    effectiveGoal = goal.ifBlank { "build_muscle" }
                    val fullGym = Equipment.entries.map { it.name }.toSet()
                    settingsRepo.setDaysPerWeek(4)
                    settingsRepo.setAvailableEquipment(fullGym)
                    settingsRepo.setFrozenExerciseIds(null)
                    settingsRepo.setRotationCadence("never")
                    settingsRepo.setProgramExperience(experience.ifBlank { "intermediate" })
                    settingsRepo.setUserGoal(effectiveGoal)
                    programRepository.clearProgram()
                }
                else -> {
                    // freestyle: no fixed plan. Clear the program (same as custom) so there's a genuine
                    // "no program" state to lead with logging — no hidden split to leak into the home
                    // counter, coach, reminders or Stats. Goal + experience are still collected so Stats
                    // and the coach have them if the user later switches to following a plan.
                    effectiveGoal = goal.ifBlank { "build_muscle" }
                    val fullGym = Equipment.entries.map { it.name }.toSet()
                    settingsRepo.setDaysPerWeek(4)
                    settingsRepo.setAvailableEquipment(fullGym)
                    settingsRepo.setFrozenExerciseIds(null)
                    settingsRepo.setRotationCadence("never")
                    settingsRepo.setProgramExperience(experience.ifBlank { "intermediate" })
                    settingsRepo.setUserGoal(effectiveGoal)
                    programRepository.clearProgram()
                }
            }
            // Set ONBOARDING_DONE last — it flips the UI from onboarding to home, so the freshly
            // generated program is already live when the home screen first composes.
            settingsRepo.completeOnboarding(name, useKg, effectiveGoal, bodyweightLb, useMiles)
        }
    }

    private suspend fun generateProgram(
        daysPerWeek: Int, goal: String, experience: String,
        problemAreas: Set<String>, equipment: Set<String>, frozenIds: Set<String>?, seed: Long
    ) {
        programRepository.generate(
            // Onboarding doesn't collect emphasis / priorityMuscles / pinned / dbMaxLb (the dumbbell
            // ceiling) — they take their no-op defaults (balanced / none / none / no ceiling) and are
            // refined later in Settings → Program. MUST stay identical to [buildPreview] so the saved
            // week matches the previewed one (same seed + inputs).
            GenerationParams(
                daysPerWeek = daysPerWeek, goal = goal, experience = experience,
                problemAreas = problemAreas.mapNotNull { ProblemArea.fromCode(it) }.toSet(),
                frozenIds = frozenIds
            ),
            equipment.mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet(),
            emptySet(), emptySet(), seed = seed
        )
    }
}
