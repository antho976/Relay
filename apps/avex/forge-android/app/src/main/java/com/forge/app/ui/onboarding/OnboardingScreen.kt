package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import com.forge.app.domain.units.parseToLb
import com.forge.app.ui.theme.ForgeMotion
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.material3.AlertDialog
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
    private val programRepository: com.forge.app.data.repo.ProgramRepository
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

    fun complete(
        name: String,
        useKg: Boolean,
        goal: String,
        bodyweightLb: Double?,
        daysPerWeek: Int = 4,
        equipment: Set<String> = emptySet(),
        cadence: String = "never",
        everyN: Int = 4,
        experience: String = "intermediate",
        problemAreas: Set<String> = emptySet(),
        seed: Long = System.nanoTime(),
        generate: Boolean = true,
        accentEmphasis: String = "off",
        plateWeightLb: Double = 15.0,
        frozenIds: Set<String>? = null,
        sex: String = ""
    ) {
        viewModelScope.launch {
            bodyweightLb?.let { bodyweightRepo.log(it) }
            settingsRepo.setAccentEmphasis(accentEmphasis)
            settingsRepo.setPlateWeightLb(plateWeightLb)
            settingsRepo.setUserSex(sex)
            if (generate) {
                settingsRepo.setDaysPerWeek(daysPerWeek)
                settingsRepo.setAvailableEquipment(equipment)
                settingsRepo.setFrozenExerciseIds(frozenIds)
                settingsRepo.setRotationCadence(cadence)
                if (cadence == "every_n") settingsRepo.setRotationEveryN(everyN)
                settingsRepo.setProgramExperience(experience)
                settingsRepo.setUserGoal(goal)
                problemAreas.forEach { settingsRepo.toggleProblemArea(it, true) }
                // Persist exactly the week shown in the preview (same seed + inputs).
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
            // Set ONBOARDING_DONE last — it flips the UI from onboarding to home, so the freshly
            // generated program is already live when the home screen first composes.
            settingsRepo.completeOnboarding(name, useKg, goal, bodyweightLb)
        }
    }
}

/** Onboarding pages (0-indexed); the last is the live preview. Related steps are grouped per page. */
private const val PAGE_COUNT = 6
private const val LAST_PAGE = PAGE_COUNT - 1

/** Step names for the page indicator — so a new user can see where they are and what's left,
 *  instead of guessing from anonymous dots. One per page, in order. */
private val ONBOARDING_STEP_NAMES = listOf(
    "About you", "Your goal", "Your gym", "Fine-tuning", "Style", "Your plan"
)

/** Most of the world lifts in kg; the US (and Liberia / Myanmar) use lb. Seed the onboarding unit
 *  from the device locale so a non-US user isn't forced to flip a toggle on the very first screen.
 *  An empty country (locale carries only a language, common on emulators / minimal setups) is
 *  uninformative — fall back to the app's historical lb default rather than guessing kg. */
private fun localeDefaultUseKg(): Boolean {
    val country = java.util.Locale.getDefault().country.uppercase(java.util.Locale.ROOT)
    if (country.isBlank()) return false
    return country !in setOf("US", "LR", "MM")
}

/**
 * Plausible adult bodyweight range, in lb (~27–454 kg) — guards the relative-strength denominator.
 * A fat-finger "1" instead of "100" kg would otherwise log a 1-kg bodyweight and render a ~50× ratio
 * on the Stats strength-standards card. Out-of-range input simply isn't recorded (treated as blank).
 */
internal const val MIN_BODYWEIGHT_LB = 60.0
internal const val MAX_BODYWEIGHT_LB = 1000.0

/** Parse a typed bodyweight (in the user's display unit) to a sane lb value, or null if blank,
 *  unparseable, or outside the plausible human range. Uses the shared [parseToLb] converter so the
 *  kg→lb factor can never drift from the rest of the app. Pure + testable. */
internal fun parseSaneBodyweightLb(input: String, useKg: Boolean): Double? {
    val lb = parseToLb(input, useKg) ?: return null
    return if (lb in MIN_BODYWEIGHT_LB..MAX_BODYWEIGHT_LB) lb else null
}

@Composable
fun OnboardingScreen(
    onFinished: () -> Unit,
    viewModel: OnboardingViewModel = hiltViewModel()
) {
    var page by remember { mutableIntStateOf(0) }
    var name by remember { mutableStateOf("") }
    var useKg by remember { mutableStateOf(localeDefaultUseKg()) }
    // Program-shaping choices start UNSELECTED — the user actively picks them; nothing is pre-highlighted.
    var goal by remember { mutableStateOf("") }
    var experience by remember { mutableStateOf("") }
    var bodyweightInput by remember { mutableStateOf("") }
    // Sex is optional and drives only the Stats strength standards — null until the user picks.
    var sex by remember { mutableStateOf<String?>(null) }
    var daysPerWeek by remember { mutableIntStateOf(0) }
    var equipment by remember { mutableStateOf(emptySet<String>()) }
    // Non-null when a curated preset (e.g. Developer's) is picked — locks the exercise pool.
    var frozenIds by remember { mutableStateOf<Set<String>?>(null) }
    var plateWeightLb by remember { mutableStateOf(15.0) }
    var problemAreas by remember { mutableStateOf(emptySet<String>()) }
    var cadence by remember { mutableStateOf("") }
    var everyN by remember { mutableIntStateOf(4) }
    var accentEmphasis by remember { mutableStateOf("off") }
    var previewSeed by remember { mutableLongStateOf(Random.nextLong()) }
    var showSkipConfirm by remember { mutableStateOf(false) }

    // Pure preview — recomputed whenever an input or the re-roll seed changes (shown on the last page).
    val previewDays = remember(previewSeed, daysPerWeek, equipment, frozenIds, goal, experience, problemAreas) {
        viewModel.buildPreview(daysPerWeek, equipment, goal, experience, problemAreas, frozenIds, previewSeed)
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 24.dp, vertical = 48.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Page indicator
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                repeat(PAGE_COUNT) { i ->
                    val active = i == page
                    val dotWidth by animateDpAsState(if (active) 20.dp else 8.dp, label = "dotWidth")
                    val dotColor by animateColorAsState(
                        if (active) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.outline.copy(alpha = 0.4f),
                        label = "dotColor"
                    )
                    Box(
                        Modifier
                            .size(width = dotWidth, height = 8.dp)
                            .clip(CircleShape)
                            .background(dotColor)
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            Text(
                "Step ${page + 1} of $PAGE_COUNT · ${ONBOARDING_STEP_NAMES.getOrElse(page) { "" }}",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(24.dp))

            // Page content — each page groups related steps in a scrolling column.
            AnimatedContent(
                targetState = page,
                modifier = Modifier.weight(1f),
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    (slideInHorizontally(ForgeMotion.enterTween()) { it * dir } + fadeIn(ForgeMotion.enterTween())) togetherWith
                        (slideOutHorizontally(ForgeMotion.exitTween()) { -it * dir } + fadeOut(ForgeMotion.exitTween()))
                },
                label = "onboarding_page"
            ) { p ->
                // The preview page scrolls itself — render it directly. Wrapping it in another
                // verticalScroll would nest two same-axis scrollables and crash Compose (the inner one
                // gets an infinite-height measure). Every other page needs the scroll wrapper.
                if (p == LAST_PAGE) {
                    StepPreview(days = previewDays, onRegenerate = { previewSeed = Random.nextLong() })
                } else {
                    Column(
                        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(28.dp)
                    ) {
                        when (p) {
                            0 -> {
                                StepName(name = name, onNameChange = { name = it })
                                StepBodyweight(input = bodyweightInput, useKg = useKg, onInputChange = { bodyweightInput = it })
                                StepSex(selected = sex, onSelect = { sex = it })
                                StepUnits(useKg = useKg, onToggle = { useKg = it })
                                Text(
                                    "Everything stays on your phone — no account, no sign-up. Forge has no internet access, so it can't send your data anywhere.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            1 -> {
                                StepGoal(selected = goal, onSelect = { goal = it })
                                StepExperience(selected = experience, onSelect = { experience = it })
                            }
                            2 -> {
                                StepDays(days = daysPerWeek, onChange = { daysPerWeek = it })
                                StepEquipment(
                                    selected = equipment,
                                    frozenIds = frozenIds,
                                    onToggle = { code ->
                                        equipment = if (code in equipment) equipment - code else equipment + code
                                        // Hand-editing equipment leaves any curated preset.
                                        frozenIds = null
                                    },
                                    onSelectPreset = { preset ->
                                        equipment = preset.equipment
                                        frozenIds = preset.frozenIds
                                    }
                                )
                                StepPlateWeight(plateWeightLb = plateWeightLb, onSet = { plateWeightLb = it })
                            }
                            3 -> {
                                StepProblemAreas(
                                    selected = problemAreas,
                                    onToggle = { code -> problemAreas = if (code in problemAreas) problemAreas - code else problemAreas + code }
                                )
                                StepCadence(cadence = cadence, everyN = everyN, onSet = { c, n -> cadence = c; everyN = n })
                            }
                            4 -> StepEmphasis(selected = accentEmphasis, onSelect = { accentEmphasis = it })
                        }
                    }
                }
            }

            // Gate "Next" on the pages whose choices the generator needs: a typed bodyweight must be a
            // positive number; goal + experience must be picked; days + equipment must be chosen (an
            // empty equipment set otherwise means "full gym" to the generator).
            val canAdvance = when (page) {
                0 -> bodyweightInput.isBlank() || parseSaneBodyweightLb(bodyweightInput, useKg) != null
                1 -> goal.isNotEmpty() && experience.isNotEmpty()
                2 -> daysPerWeek in 1..7 && equipment.isNotEmpty()
                else -> true
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (page > 0) {
                    TextButton(onClick = { page-- }) { Text("Back") }
                } else {
                    TextButton(onClick = { showSkipConfirm = true }) { Text("Skip onboarding") }
                }
                Button(
                    enabled = canAdvance,
                    onClick = {
                        if (page < LAST_PAGE) {
                            page++
                        } else {
                            val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
                            viewModel.complete(
                                name.trim(), useKg, goal, bwLb,
                                daysPerWeek, equipment, cadence.ifEmpty { "never" }, everyN, experience, problemAreas,
                                previewSeed, accentEmphasis = accentEmphasis, plateWeightLb = plateWeightLb,
                                frozenIds = frozenIds, sex = sex ?: ""
                            )
                            onFinished()
                        }
                    }
                ) {
                    Text(if (page < LAST_PAGE) "Next" else "Let's go")
                }
            }
        }

        if (showSkipConfirm) {
            AlertDialog(
                onDismissRequest = { showSkipConfirm = false },
                title = { Text("Skip setup?") },
                text = {
                    Text(
                        "You'll start with a basic bodyweight program and default settings — no plan " +
                            "tailored to your gym or goals. You can set your equipment and goal, and " +
                            "generate a personalized program, any time in Settings → Program."
                    )
                },
                confirmButton = {
                    TextButton(onClick = {
                        showSkipConfirm = false
                        val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
                        viewModel.complete(
                            name.trim(), useKg, "build_muscle", bwLb,
                            daysPerWeek = 4,
                            equipment = setOf(Equipment.BODYWEIGHT_ONLY.name),
                            experience = "intermediate", generate = true, sex = sex ?: ""
                        )
                        onFinished()
                    }) { Text("Skip anyway") }
                },
                dismissButton = {
                    TextButton(onClick = { showSkipConfirm = false }) { Text("Keep setting up") }
                }
            )
        }
    }
}
