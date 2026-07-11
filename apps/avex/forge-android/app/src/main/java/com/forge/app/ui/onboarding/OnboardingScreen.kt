package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
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
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.parseToLb
import com.forge.app.program.Equipment
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.ForgeMotion
import kotlin.random.Random

// Page indices — one light decision per screen (MacroFactor pacing). After the plan-mode step (4):
// "generated" continues through the plan-building pages (5-12); "custom"/"freestyle" stop at
// Experience (6), having picked a goal + experience (which steer the coach), then finish.
private const val PAGE_WELCOME = 0
private const val PAGE_UNITS = 1
private const val PAGE_BODY = 2
private const val PAGE_WEARABLE = 3
private const val PAGE_PLAN_MODE = 4
private const val PAGE_GOAL = 5
private const val PAGE_EXPERIENCE = 6
private const val PAGE_DAYS = 7
private const val PAGE_EQUIPMENT = 8
private const val PAGE_FINE_TUNE = 9
private const val PAGE_PLATE = 10
private const val PAGE_AREAS = 11
private const val PAGE_CADENCE = 12
private const val PAGE_PREVIEW = 13

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
    // Wait for the one-shot resume-draft read before composing — a fully killed app reopens setup
    // exactly where it left off. Until then only the theme gradient shows (a frame or two).
    val draftLoad by viewModel.draftLoad.collectAsState()
    val load = draftLoad
    if (load !is DraftLoad.Ready) return
    val draft = load.draft

    var page by remember { mutableIntStateOf(draft?.page?.coerceIn(0, PAGE_PREVIEW) ?: 0) }
    // Plan source: generated / custom (self-built) / freestyle (no plan, log freely).
    var planMode by remember { mutableStateOf(draft?.planMode ?: "") }
    var name by remember { mutableStateOf(draft?.name ?: "") }
    var useKg by remember { mutableStateOf(draft?.useKg ?: localeDefaultUseKg()) }
    // Distance unit tracks the weight unit (lb→miles, kg→km) until the user explicitly taps the
    // distance selector. Only an explicit pick is persisted, so leaving it untouched ties to the weight unit.
    var useMilesChoice by remember { mutableStateOf(draft?.useMilesChoice ?: false) }
    var distanceTouched by remember { mutableStateOf(draft?.distanceTouched ?: false) }
    // Program-shaping choices start UNSELECTED — the user actively picks them; nothing is pre-highlighted.
    var goal by remember { mutableStateOf(draft?.goal ?: "") }
    var experience by remember { mutableStateOf(draft?.experience ?: "") }
    var bodyweightInput by remember { mutableStateOf(draft?.bodyweightInput ?: "") }
    // Sex is optional and drives only the Stats strength standards — null until the user picks
    // (an explicit "Prefer not to say" stores "").
    var sex by remember { mutableStateOf(draft?.sex) }
    // Wearable brand (WearableBrand key) — advisory; tailors Settings → Recovery's sync pointers.
    var wearable by remember { mutableStateOf(draft?.wearable ?: "") }
    var daysPerWeek by remember { mutableIntStateOf(draft?.daysPerWeek ?: 0) }
    var equipment by remember { mutableStateOf(draft?.equipment ?: emptySet()) }
    // Non-null when a curated preset (e.g. Developer's) is picked — locks the exercise pool.
    var frozenIds by remember { mutableStateOf(draft?.frozenIds) }
    // Locale-aware default: a kg lifter's "not sure" default is a round kg plate (10 kg), an lb
    // lifter's stays 15 lb. Stored in lb; the plate step shows it in the user's unit.
    var plateWeightLb by remember {
        mutableStateOf(draft?.plateWeightLb ?: if (localeDefaultUseKg()) fromDisplayWeight(10.0, true) else 15.0)
    }
    var problemAreas by remember { mutableStateOf(draft?.problemAreas ?: emptySet()) }
    var cadence by remember { mutableStateOf(draft?.cadence ?: "") }
    var everyN by remember { mutableIntStateOf(draft?.everyN ?: 4) }
    var previewSeed by remember { mutableLongStateOf(draft?.previewSeed ?: Random.nextLong()) }
    var showSkipConfirm by remember { mutableStateOf(false) }
    // Coach opt-in is asked only on the no-plan / make-your-own paths (the generated path keeps it on).
    var showCoachAsk by remember { mutableStateOf(false) }

    // Persist a resume draft on every answer change (conflated in the ViewModel); completion
    // removes it atomically, so a finished user never resumes into a stale setup.
    val snapshot = OnboardingDraft(
        page, planMode, name, useKg, useMilesChoice, distanceTouched, goal, experience,
        bodyweightInput, sex, wearable, daysPerWeek, equipment, frozenIds, plateWeightLb,
        problemAreas, cadence, everyN, previewSeed
    )
    LaunchedEffect(snapshot) { viewModel.saveDraft(snapshot) }

    // The generated path walks all plan-building pages through the preview. Custom & freestyle don't
    // build a plan here, but they still pick a goal + experience (they steer the coach/Stats) — so
    // they finish after the Experience step. Before a mode is picked, the flow can't pass plan-mode.
    val isGenerated = planMode == PLAN_GENERATED
    val lastPage = when {
        isGenerated -> PAGE_PREVIEW
        planMode.isNotEmpty() -> PAGE_EXPERIENCE
        else -> PAGE_PLAN_MODE
    }
    // Progress denominator: until a mode is picked we don't know the path length, so assume the
    // LONGEST (generated). Committing to the short custom/freestyle path then only ever nudges the
    // rail FORWARD (the fraction grows), never backward. Once a short mode is committed, use its
    // real length so the rail still reaches 100% on finish.
    val progressTotal = if (planMode.isEmpty() || isGenerated) PAGE_PREVIEW + 1 else PAGE_EXPERIENCE + 1

    // Pure preview — recomputed whenever an input or the re-roll seed changes (shown on the last page).
    val previewDays = remember(previewSeed, daysPerWeek, equipment, frozenIds, goal, experience, problemAreas) {
        viewModel.buildPreview(daysPerWeek, equipment, goal, experience, problemAreas, frozenIds, previewSeed)
    }

    fun finish(coachEnabled: Boolean = true) {
        val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
        viewModel.complete(
            planMode = planMode, name = name.trim(), useKg = useKg,
            useMiles = if (distanceTouched) useMilesChoice else null,
            sex = sex ?: "", wearable = wearable, bodyweightLb = bwLb,
            goal = goal, daysPerWeek = daysPerWeek, equipment = equipment,
            cadence = cadence.ifEmpty { "never" }, everyN = everyN, experience = experience,
            problemAreas = problemAreas, seed = previewSeed,
            plateWeightLb = plateWeightLb, frozenIds = frozenIds, coachEnabled = coachEnabled
        )
        onFinished(planMode)
    }

    // No solid background — the theme's page gradient shows through, like every other screen.
    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 28.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Top chrome: ← back, the step rail, skip →. The rail is the only progress readout.
            val ctaFinishes = page == lastPage && planMode.isNotEmpty()
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                if (page > 0) {
                    Box(
                        modifier = Modifier
                            .size(48.dp)
                            .clickableLabeled("Back") { page-- },
                        contentAlignment = Alignment.Center
                    ) {
                        Text("←", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    }
                } else {
                    Spacer(Modifier.width(48.dp))
                }
                ProgressRail(
                    fraction = (page + 1).toFloat() / progressTotal,
                    modifier = Modifier.weight(1f)
                )
                if (!ctaFinishes) SkipLink { showSkipConfirm = true } else Spacer(Modifier.width(36.dp))
            }
            Spacer(Modifier.height(24.dp))

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
                    StepPreview(days = previewDays)
                } else {
                    Column(modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        when (p) {
                            PAGE_WELCOME -> StepWelcome(name = name, onNameChange = { name = it })
                            PAGE_UNITS -> StepUnits(
                                useKg = useKg, onWeightToggle = { useKg = it },
                                useMiles = if (distanceTouched) useMilesChoice else !useKg,
                                onDistanceToggle = { useMilesChoice = it; distanceTouched = true }
                            )
                            PAGE_BODY -> StepBody(
                                bodyweightInput = bodyweightInput, useKg = useKg,
                                onInputChange = { bodyweightInput = it },
                                sex = sex, onSexSelect = { sex = it }
                            )
                            PAGE_WEARABLE -> StepWearable(selected = wearable, onSelect = { wearable = it })
                            PAGE_PLAN_MODE -> StepPlanMode(selected = planMode, onSelect = { planMode = it })
                            PAGE_GOAL -> StepGoal(selected = goal, onSelect = { goal = it })
                            PAGE_EXPERIENCE -> StepExperience(selected = experience, onSelect = { experience = it })
                            PAGE_DAYS -> StepDays(days = daysPerWeek, onChange = { daysPerWeek = it })
                            PAGE_EQUIPMENT -> StepGymPresets(
                                selected = equipment,
                                frozenIds = frozenIds,
                                onSelectPreset = { preset ->
                                    equipment = preset.equipment
                                    frozenIds = preset.frozenIds
                                }
                            )
                            PAGE_FINE_TUNE -> StepFineTune(
                                selected = equipment,
                                onToggle = { code ->
                                    equipment = if (code in equipment) equipment - code else equipment + code
                                    // Hand-editing equipment leaves any curated preset.
                                    frozenIds = null
                                }
                            )
                            PAGE_PLATE -> StepPlateWeight(plateWeightLb = plateWeightLb, useKg = useKg, onSet = { plateWeightLb = it })
                            PAGE_AREAS -> StepProblemAreas(
                                selected = problemAreas,
                                onToggle = { code -> problemAreas = if (code in problemAreas) problemAreas - code else problemAreas + code }
                            )
                            PAGE_CADENCE -> StepCadence(cadence = cadence, everyN = everyN, onSet = { c, n -> cadence = c; everyN = n })
                        }
                    }
                }
            }
            Spacer(Modifier.height(16.dp))

            // Gate "Continue" on the pages whose choices the generator needs.
            val canAdvance = when (page) {
                PAGE_BODY -> bodyweightInput.isBlank() || parseSaneBodyweightLb(bodyweightInput, useKg) != null
                // Wearable is advisory-only (like sex) — never block Continue; "" just leaves it unset.
                PAGE_PLAN_MODE -> planMode.isNotEmpty()
                PAGE_GOAL -> goal.isNotEmpty()
                PAGE_EXPERIENCE -> experience.isNotEmpty()
                PAGE_DAYS -> daysPerWeek in 1..7
                PAGE_EQUIPMENT, PAGE_FINE_TUNE -> equipment.isNotEmpty()
                else -> true
            }
            // Explain why "Continue" is held on the gym pages — the generator can't build a plan with
            // no equipment selected, so this guards the gate instead of a silently greyed button.
            if ((page == PAGE_EQUIPMENT || page == PAGE_FINE_TUNE) && equipment.isEmpty()) {
                Text(
                    if (page == PAGE_EQUIPMENT) "Pick a setup. Avex can't build your plan without knowing your gear."
                    else "Keep at least one piece on. Avex can't build your plan with nothing to train on.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
                )
                Spacer(Modifier.height(10.dp))
            }
            val ctaLabel = when {
                !ctaFinishes -> "Continue"
                planMode == PLAN_CUSTOM -> "Build my plan"
                planMode == PLAN_FREESTYLE -> "Start logging"
                else -> "Let's go"
            }
            val onCta: () -> Unit = {
                when {
                    page < lastPage -> page++
                    // No-plan / make-your-own: ask about the coach before finishing.
                    planMode == PLAN_CUSTOM || planMode == PLAN_FREESTYLE -> showCoachAsk = true
                    else -> finish()
                }
            }
            if (page == PAGE_PREVIEW) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlineCapsule("Re-roll", onClick = { previewSeed = Random.nextLong() })
                    PrimaryCapsule(ctaLabel, onClick = onCta, modifier = Modifier.weight(1f))
                }
            } else {
                PrimaryCapsule(ctaLabel, onClick = onCta, enabled = canAdvance, modifier = Modifier.fillMaxWidth())
            }
        }

        if (showSkipConfirm) {
            AlertDialog(
                onDismissRequest = { showSkipConfirm = false },
                title = { Text("Skip setup?") },
                text = {
                    Text(
                        when (planMode) {
                            PLAN_CUSTOM -> "You'll skip the rest of setup and start with no plan yet. Build " +
                                "your own from the home screen, or generate one anytime in Settings → Program."
                            PLAN_FREESTYLE -> "You'll skip the rest of setup and start with no fixed plan, just " +
                                "logging your workouts. You can switch to a plan anytime in Settings → Program."
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
                        // generated for the usual early skip where nothing was chosen yet.
                        val effectiveMode = planMode.ifEmpty { PLAN_GENERATED }
                        viewModel.complete(
                            planMode = effectiveMode, name = name.trim(), useKg = useKg, sex = sex ?: "",
                            useMiles = if (distanceTouched) useMilesChoice else null,
                            // Honor a wearable picked before skipping, like the mode above.
                            wearable = wearable,
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
