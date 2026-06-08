package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import com.forge.app.ui.theme.ForgeMotion
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.program.Equipment
import com.forge.app.program.GeneratedDay
import com.forge.app.program.GenerationParams
import com.forge.app.program.ProblemArea
import com.forge.app.program.ProgramGenerator
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import kotlin.random.Random
import javax.inject.Inject

@HiltViewModel
class OnboardingViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder,
    private val programRepository: com.forge.app.data.repo.ProgramRepository
) : ViewModel() {

    /** Pure, side-effect-free week for the preview step — the same [seed] is persisted on finish. */
    fun buildPreview(
        daysPerWeek: Int, equipment: Set<String>, goal: String, experience: String,
        problemAreas: Set<String>, cardioDays: Int, seed: Long
    ): List<GeneratedDay> = ProgramGenerator.generate(
        GenerationParams(
            daysPerWeek = daysPerWeek, goal = goal, experience = experience, cardioDays = cardioDays,
            problemAreas = problemAreas.mapNotNull { ProblemArea.fromCode(it) }.toSet()
        ),
        equipment.mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet(),
        emptySet(), emptySet(), seed = seed
    )

    fun complete(
        name: String,
        useKg: Boolean,
        goal: String,
        bodyweightLb: Double?,
        withSampleData: Boolean = false,
        daysPerWeek: Int = 4,
        equipment: Set<String> = emptySet(),
        cadence: String = "never",
        everyN: Int = 4,
        experience: String = "intermediate",
        problemAreas: Set<String> = emptySet(),
        cardioDays: Int = 0,
        seed: Long = System.nanoTime(),
        generate: Boolean = true
    ) {
        viewModelScope.launch {
            bodyweightLb?.let { bodyweightRepo.log(it) }
            if (generate) {
                settingsRepo.setDaysPerWeek(daysPerWeek)
                settingsRepo.setAvailableEquipment(equipment)
                settingsRepo.setRotationCadence(cadence)
                if (cadence == "every_n") settingsRepo.setRotationEveryN(everyN)
                settingsRepo.setProgramExperience(experience)
                settingsRepo.setUserGoal(goal)
                settingsRepo.setCardioDaysPerWeek(cardioDays)
                problemAreas.forEach { settingsRepo.toggleProblemArea(it, true) }
                // Persist exactly the week shown in the preview (same seed + inputs).
                programRepository.generate(
                    GenerationParams(
                        daysPerWeek = daysPerWeek, goal = goal, experience = experience, cardioDays = cardioDays,
                        problemAreas = problemAreas.mapNotNull { ProblemArea.fromCode(it) }.toSet()
                    ),
                    equipment.mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet(),
                    emptySet(), emptySet(), seed = seed
                )
            }
            if (withSampleData) sampleDataSeeder.seed()
            // Set ONBOARDING_DONE last — it flips the UI from onboarding to home, so the freshly
            // generated program is already live when the home screen first composes.
            settingsRepo.completeOnboarding(name, useKg, goal, bodyweightLb)
        }
    }
}

/** Total onboarding steps (0-indexed); the last is the live preview. */
private const val STEP_COUNT = 11
private const val LAST_STEP = STEP_COUNT - 1

@Composable
fun OnboardingScreen(
    onFinished: () -> Unit,
    viewModel: OnboardingViewModel = hiltViewModel()
) {
    var step by remember { mutableIntStateOf(0) }
    var name by remember { mutableStateOf("") }
    var useKg by remember { mutableStateOf(false) }
    var goal by remember { mutableStateOf("get_stronger") }
    var experience by remember { mutableStateOf("intermediate") }
    var bodyweightInput by remember { mutableStateOf("") }
    var loadSampleData by remember { mutableStateOf(false) }
    var daysPerWeek by remember { mutableIntStateOf(4) }
    var equipment by remember { mutableStateOf(emptySet<String>()) }
    var problemAreas by remember { mutableStateOf(emptySet<String>()) }
    var cadence by remember { mutableStateOf("never") }
    var everyN by remember { mutableIntStateOf(4) }
    var cardioDays by remember { mutableIntStateOf(0) }
    var previewSeed by remember { mutableLongStateOf(Random.nextLong()) }

    // Pure preview — recomputed whenever an input or the re-roll seed changes.
    val previewDays = remember(previewSeed, daysPerWeek, equipment, goal, experience, problemAreas, cardioDays) {
        viewModel.buildPreview(daysPerWeek, equipment, goal, experience, problemAreas, cardioDays, previewSeed)
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 24.dp, vertical = 48.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Step indicator
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                repeat(STEP_COUNT) { i ->
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(
                                if (i == step) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.outline.copy(alpha = 0.4f)
                            )
                    )
                }
            }
            Spacer(Modifier.height(24.dp))

            // Step content (flexible; the preview step scrolls internally)
            AnimatedContent(
                targetState = step,
                modifier = Modifier.weight(1f),
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    (slideInHorizontally(ForgeMotion.enterTween()) { it * dir } + fadeIn(ForgeMotion.enterTween())) togetherWith
                        (slideOutHorizontally(ForgeMotion.exitTween()) { -it * dir } + fadeOut(ForgeMotion.exitTween()))
                },
                label = "onboarding_step"
            ) { s ->
                when (s) {
                    0 -> StepName(name = name, onNameChange = { name = it })
                    1 -> StepUnits(useKg = useKg, onToggle = { useKg = it })
                    2 -> StepGoal(selected = goal, onSelect = { goal = it })
                    3 -> StepExperience(selected = experience, onSelect = { experience = it })
                    4 -> StepBodyweight(
                        input = bodyweightInput, useKg = useKg,
                        onInputChange = { bodyweightInput = it },
                        sampleData = loadSampleData, onSampleDataToggle = { loadSampleData = it }
                    )
                    5 -> StepDays(days = daysPerWeek, onChange = { daysPerWeek = it })
                    6 -> StepEquipment(
                        selected = equipment,
                        onToggle = { code -> equipment = if (code in equipment) equipment - code else equipment + code },
                        onSetAll = { equipment = it }
                    )
                    7 -> StepProblemAreas(
                        selected = problemAreas,
                        onToggle = { code -> problemAreas = if (code in problemAreas) problemAreas - code else problemAreas + code }
                    )
                    8 -> StepCadence(cadence = cadence, everyN = everyN, onSet = { c, n -> cadence = c; everyN = n })
                    9 -> StepCardio(days = cardioDays, onSet = { cardioDays = it })
                    10 -> StepPreview(days = previewDays, onRegenerate = { previewSeed = Random.nextLong() })
                }
            }

            // Navigation
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (step > 0) {
                    TextButton(onClick = { step-- }) { Text("Back") }
                } else {
                    TextButton(onClick = {
                        viewModel.complete("", false, goal, null, generate = false)
                        onFinished()
                    }) { Text("Skip") }
                }
                Button(
                    onClick = {
                        if (step < LAST_STEP) {
                            step++
                        } else {
                            val bwLb = bodyweightInput.toDoubleOrNull()?.let { raw -> if (useKg) raw * 2.20462 else raw }
                            viewModel.complete(
                                name, useKg, goal, bwLb, loadSampleData,
                                daysPerWeek, equipment, cadence, everyN, experience, problemAreas, cardioDays, previewSeed
                            )
                            onFinished()
                        }
                    }
                ) {
                    Text(if (step < LAST_STEP) "Next" else "Let's go")
                }
            }
        }
    }
}
