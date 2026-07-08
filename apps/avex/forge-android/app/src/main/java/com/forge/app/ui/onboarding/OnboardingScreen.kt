package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import com.forge.app.domain.units.fromDisplayWeight
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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.forge.app.program.Equipment
import kotlin.random.Random

// Page indices. After the plan-mode step (1): "generated" continues through the plan-building pages
// (2-5); "custom"/"freestyle" stop one step later, at Goals (2), to pick a goal + experience (which
// steer the coach), then finish.
private const val PAGE_ABOUT = 0
private const val PAGE_PLAN_MODE = 1
private const val PAGE_GOALS = 2
private const val PAGE_GYM = 3
private const val PAGE_TUNING = 4
private const val PAGE_PREVIEW = 5

/** Step names for the progress label — indexed by page. Only 0-2 are reached off the generated path. */
private val ONBOARDING_STEP_NAMES = listOf(
    "About you", "Your plan", "Goals", "Your gym", "Fine-tuning", "Your week"
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

/**
 * @param onFinished invoked with the chosen plan mode ([PLAN_GENERATED] / [PLAN_CUSTOM] /
 *   [PLAN_FREESTYLE]) so the host can route the first screen (e.g. custom → editor, freestyle → home).
 */
@Composable
fun OnboardingScreen(
    onFinished: (String) -> Unit,
    viewModel: OnboardingViewModel = hiltViewModel()
) {
    var page by remember { mutableIntStateOf(0) }
    // Plan source: generated / custom (self-built) / freestyle (no plan, log freely).
    var planMode by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var useKg by remember { mutableStateOf(localeDefaultUseKg()) }
    // Distance unit tracks the weight unit (lb→miles, kg→km) until the user explicitly taps the
    // distance toggle. Only an explicit pick is persisted, so leaving it untouched ties to the weight unit.
    var useMilesChoice by remember { mutableStateOf(false) }
    var distanceTouched by remember { mutableStateOf(false) }
    // Program-shaping choices start UNSELECTED — the user actively picks them; nothing is pre-highlighted.
    var goal by remember { mutableStateOf("") }
    var experience by remember { mutableStateOf("") }
    var bodyweightInput by remember { mutableStateOf("") }
    // Sex is optional and drives only the Stats strength standards — null until the user picks
    // (an explicit "Prefer not to say" stores "").
    var sex by remember { mutableStateOf<String?>(null) }
    var daysPerWeek by remember { mutableIntStateOf(0) }
    var equipment by remember { mutableStateOf(emptySet<String>()) }
    // Non-null when a curated preset (e.g. Developer's) is picked — locks the exercise pool.
    var frozenIds by remember { mutableStateOf<Set<String>?>(null) }
    // Locale-aware default (#2): a kg lifter's "not sure" default is a round kg plate (10 kg), an lb
    // lifter's stays 15 lb. Stored in lb; the plate step shows it in the user's unit.
    var plateWeightLb by remember { mutableStateOf(if (localeDefaultUseKg()) fromDisplayWeight(10.0, true) else 15.0) }
    var problemAreas by remember { mutableStateOf(emptySet<String>()) }
    var cadence by remember { mutableStateOf("") }
    var everyN by remember { mutableIntStateOf(4) }
    var previewSeed by remember { mutableLongStateOf(Random.nextLong()) }
    var showSkipConfirm by remember { mutableStateOf(false) }
    // Coach opt-in is asked only on the no-plan / make-your-own paths (the generated path keeps it on).
    var showCoachAsk by remember { mutableStateOf(false) }

    // The generated path walks all plan-building pages through the preview. Custom & freestyle don't
    // build a plan here, but they still pick a goal + experience on the Goals page (so the coach/Stats
    // have them) — so they finish one step past the plan-mode step. Before a mode is picked, the bar
    // assumes the short path.
    val isGenerated = planMode == PLAN_GENERATED
    val lastPage = when {
        isGenerated -> PAGE_PREVIEW
        planMode.isNotEmpty() -> PAGE_GOALS
        else -> PAGE_PLAN_MODE
    }
    // Progress denominator: until a mode is picked we don't know the path length, so assume the LONGEST
    // (generated). Committing to the short custom/freestyle path then only ever nudges the bar FORWARD
    // (the fraction grows), never backward — picking a mode on the plan-mode page felt jumpy otherwise.
    // Once a short mode is committed, use its real length so the bar still reaches 100% on finish.
    val progressTotal = if (planMode.isEmpty() || isGenerated) PAGE_PREVIEW + 1 else PAGE_GOALS + 1

    // Pure preview — recomputed whenever an input or the re-roll seed changes (shown on the last page).
    val previewDays = remember(previewSeed, daysPerWeek, equipment, frozenIds, goal, experience, problemAreas) {
        viewModel.buildPreview(daysPerWeek, equipment, goal, experience, problemAreas, frozenIds, previewSeed)
    }

    fun finish(coachEnabled: Boolean = true) {
        val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
        viewModel.complete(
            planMode = planMode, name = name.trim(), useKg = useKg,
            useMiles = if (distanceTouched) useMilesChoice else null,
            sex = sex ?: "", bodyweightLb = bwLb,
            goal = goal, daysPerWeek = daysPerWeek, equipment = equipment,
            cadence = cadence.ifEmpty { "never" }, everyN = everyN, experience = experience,
            problemAreas = problemAreas, seed = previewSeed,
            plateWeightLb = plateWeightLb, frozenIds = frozenIds, coachEnabled = coachEnabled
        )
        onFinished(planMode)
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 24.dp, vertical = 32.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Slim progress bar + small step label.
            val progress by animateFloatAsState((page + 1).toFloat() / progressTotal, label = "progress")
            LinearProgressIndicator(
                progress = { progress },
                modifier = Modifier.fillMaxWidth().height(4.dp),
                trackColor = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)
            )
            Spacer(Modifier.height(8.dp))
            Text(
                ONBOARDING_STEP_NAMES.getOrElse(page) { "" },
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(16.dp))

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
                // verticalScroll would nest two same-axis scrollables and crash Compose. Every other
                // page needs the scroll wrapper.
                if (p == PAGE_PREVIEW) {
                    StepPreview(days = previewDays, onRegenerate = { previewSeed = Random.nextLong() })
                } else {
                    Column(
                        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(20.dp)
                    ) {
                        when (p) {
                            PAGE_ABOUT -> {
                                StepName(name = name, onNameChange = { name = it })
                                StepBodyweight(input = bodyweightInput, useKg = useKg, onInputChange = { bodyweightInput = it })
                                StepSex(selected = sex, onSelect = { sex = it })
                                StepUnits(useKg = useKg, onToggle = { useKg = it })
                                StepDistanceUnits(
                                    useMiles = if (distanceTouched) useMilesChoice else !useKg,
                                    onToggle = { useMilesChoice = it; distanceTouched = true }
                                )
                                Text(
                                    "Everything stays on your phone. No account, no sign-up. Avex has no internet access, so it can't send your data anywhere.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            PAGE_PLAN_MODE -> StepPlanMode(selected = planMode, onSelect = { planMode = it })
                            PAGE_GOALS -> {
                                StepGoal(selected = goal, onSelect = { goal = it })
                                StepExperience(selected = experience, onSelect = { experience = it })
                            }
                            PAGE_GYM -> {
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
                                StepPlateWeight(plateWeightLb = plateWeightLb, useKg = useKg, onSet = { plateWeightLb = it })
                            }
                            PAGE_TUNING -> {
                                StepProblemAreas(
                                    selected = problemAreas,
                                    onToggle = { code -> problemAreas = if (code in problemAreas) problemAreas - code else problemAreas + code }
                                )
                                StepCadence(cadence = cadence, everyN = everyN, onSet = { c, n -> cadence = c; everyN = n })
                            }
                        }
                    }
                }
            }

            // Gate "Next" on the pages whose choices the generator needs.
            val canAdvance = when (page) {
                PAGE_ABOUT -> bodyweightInput.isBlank() || parseSaneBodyweightLb(bodyweightInput, useKg) != null
                PAGE_PLAN_MODE -> planMode.isNotEmpty()
                PAGE_GOALS -> goal.isNotEmpty() && experience.isNotEmpty()
                PAGE_GYM -> daysPerWeek in 1..7 && equipment.isNotEmpty()
                else -> true
            }
            // Explain why "Next" is held on the gym page — the generator can't build a plan with no
            // equipment selected, so this guards the skip rather than leaving the button silently greyed.
            if (page == PAGE_GYM && equipment.isEmpty()) {
                Text(
                    "Pick at least one. Avex can't build your plan without knowing what you can train with.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
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
                        when {
                            page < lastPage -> page++
                            // No-plan / make-your-own: ask about the coach before finishing.
                            planMode == PLAN_CUSTOM || planMode == PLAN_FREESTYLE -> showCoachAsk = true
                            else -> finish()
                        }
                    }
                ) {
                    Text(
                        when {
                            page < lastPage -> "Next"
                            planMode == PLAN_CUSTOM -> "Build my plan"
                            planMode == PLAN_FREESTYLE -> "Start logging"
                            else -> "Let's go"
                        }
                    )
                }
            }
        }

        if (showSkipConfirm) {
            AlertDialog(
                onDismissRequest = { showSkipConfirm = false },
                title = { Text("Skip setup?") },
                text = {
                    Text(
                        when (planMode) {
                            PLAN_CUSTOM -> "You'll skip the rest of setup and start with no plan yet — build " +
                                "your own from the home screen, or generate one anytime in Settings → Program."
                            PLAN_FREESTYLE -> "You'll skip the rest of setup and start with no fixed plan — just " +
                                "log your workouts. You can switch to a plan anytime in Settings → Program."
                            else -> "You'll start with a basic bodyweight program and default settings, with no " +
                                "plan tailored to your gym or goals. You can set your equipment and goal, and " +
                                "generate a personalized program, any time in Settings → Program."
                        }
                    )
                },
                confirmButton = {
                    TextButton(onClick = {
                        showSkipConfirm = false
                        val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
                        // Honor a mode the user already picked (forward → Back → Skip); default to
                        // generated for the usual page-0 skip where nothing was chosen yet.
                        val effectiveMode = planMode.ifEmpty { PLAN_GENERATED }
                        viewModel.complete(
                            planMode = effectiveMode, name = name.trim(), useKg = useKg, sex = sex ?: "",
                            bodyweightLb = bwLb, goal = "build_muscle", daysPerWeek = 4,
                            equipment = setOf(Equipment.BODYWEIGHT_ONLY.name), experience = "intermediate",
                            // Skipping bypasses the coach-ask dialog — keep coach ON only for the generated
                            // default; custom/freestyle (where the dialog would have asked) default to OFF.
                            coachEnabled = effectiveMode == PLAN_GENERATED
                        )
                        onFinished(effectiveMode)
                    }) { Text("Skip anyway") }
                },
                dismissButton = {
                    TextButton(onClick = { showSkipConfirm = false }) { Text("Keep setting up") }
                }
            )
        }

        if (showCoachAsk) {
            AlertDialog(
                onDismissRequest = { showCoachAsk = false },
                title = { Text("Want a coach?") },
                text = {
                    Text(
                        "The coach quietly watches your training and suggests small tweaks each week. " +
                            "It's optional. If you'd rather just train and log, leave it off. You can turn " +
                            "it on or off anytime in Settings → Program."
                    )
                },
                confirmButton = {
                    TextButton(onClick = { showCoachAsk = false; finish(coachEnabled = true) }) {
                        Text("Yes, use the coach")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showCoachAsk = false; finish(coachEnabled = false) }) {
                        Text("No coach")
                    }
                }
            )
        }
    }
}
