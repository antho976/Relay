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
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Plan-mode keys chosen on the onboarding plan-mode step. */
const val PLAN_GENERATED = "generated"
const val PLAN_CUSTOM = "custom"
const val PLAN_FREESTYLE = "freestyle"

@HiltViewModel
class OnboardingViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val programRepository: ProgramRepository
) : ViewModel() {

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
     * - [PLAN_CUSTOM]: seed a sensible default split so the editor has real days to reshape.
     * - [PLAN_FREESTYLE]: seed the same default split (a safety net) but flip [setFreestyleMode] so the
     *   home leads with freestyle logging instead of day cards — the user keeps no fixed plan.
     */
    fun complete(
        planMode: String,
        name: String,
        useKg: Boolean,
        sex: String,
        bodyweightLb: Double?,
        // Generated-path inputs (ignored for custom/freestyle, which use defaults):
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
                    // the user constructs their days/exercises from scratch. Default generation inputs
                    // are still set so a later Settings → Generate works if they want it.
                    effectiveGoal = "build_muscle"
                    val fullGym = Equipment.entries.map { it.name }.toSet()
                    settingsRepo.setDaysPerWeek(4)
                    settingsRepo.setAvailableEquipment(fullGym)
                    settingsRepo.setFrozenExerciseIds(null)
                    settingsRepo.setRotationCadence("never")
                    settingsRepo.setProgramExperience("intermediate")
                    settingsRepo.setUserGoal("build_muscle")
                    programRepository.clearProgram()
                }
                else -> {
                    // freestyle: seed a default full-gym split under the hood as a fallback; freestyleMode
                    // hides it and the home leads with logging.
                    effectiveGoal = "build_muscle"
                    val fullGym = Equipment.entries.map { it.name }.toSet()
                    settingsRepo.setDaysPerWeek(4)
                    settingsRepo.setAvailableEquipment(fullGym)
                    settingsRepo.setFrozenExerciseIds(null)
                    settingsRepo.setRotationCadence("never")
                    settingsRepo.setProgramExperience("intermediate")
                    settingsRepo.setUserGoal("build_muscle")
                    generateProgram(4, "build_muscle", "intermediate", emptySet(), fullGym, null, seed)
                }
            }
            // Set ONBOARDING_DONE last — it flips the UI from onboarding to home, so the freshly
            // generated program is already live when the home screen first composes.
            settingsRepo.completeOnboarding(name, useKg, effectiveGoal, bodyweightLb)
        }
    }

    private suspend fun generateProgram(
        daysPerWeek: Int, goal: String, experience: String,
        problemAreas: Set<String>, equipment: Set<String>, frozenIds: Set<String>?, seed: Long
    ) {
        programRepository.generate(
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
