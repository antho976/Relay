package com.forge.app.ui.onboarding

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.parseToLb
import com.forge.app.program.Equipment
import com.forge.app.program.ProgramGenerator
import com.forge.app.program.SplitTemplates
import com.forge.app.ui.theme.ForgeMotion
import kotlin.random.Random

/**
 * First run, rebuilt 2026-08-22 around one idea: **the plan is visible while you answer for it.**
 *
 * The old flow asked fifteen questions and showed the result on the last one — five of those
 * questions (name, units, body, watch, app lock) were settings asked before the product had proved
 * anything, and the progress bar had to guess its own denominator because the path length wasn't
 * known until question six. What replaced it:
 *
 * - **The fork leads.** Plan mode is the first screen, because it decides how much of the rest runs.
 * - **Only plan-shaping questions are in the path** — goal, experience, days, gym, gear. Every
 *   setting moved to one optional closing step ([StepExtras]) that lands *after* the week exists.
 * - **The week builds under the question** ([PlanLedger]), from the day-count on. It sits outside
 *   the page slider, so questions come and go while the plan stays put and animates its own values.
 * - **The rail is segmented** ([StepRail]) — one cell per step of the path actually taken, so the
 *   short custom / freestyle path visibly drops the four cells it will never run.
 *
 * Generated: mode → goal → experience → days → gym → gear → sore spots → week → extras (9). Sore
 * spots sits before the week on purpose: it shapes exercise selection, so asking after the week was
 * approved would have shaped a plan the user had already signed off.
 * Custom / freestyle: mode → goal → experience → extras (4) — they still pick a goal and an
 * experience because those steer the coach and Stats, and they have no plan to build here.
 *
 * The watch question left the flow entirely on 2026-08-23: its answer changed nothing the user
 * could see, and Settings → Wearable owns that setup because it needs the Health Connect grants.
 */
private const val PAGE_MODE = 0
private const val PAGE_GOAL = 1
private const val PAGE_EXPERIENCE = 2
private const val PAGE_DAYS = 3
private const val PAGE_GYM = 4
private const val PAGE_GEAR = 5
private const val PAGE_SPOTS = 6
private const val PAGE_WEEK = 7
private const val PAGE_EXTRAS = 8

private val GENERATED_PATH = listOf(
    PAGE_MODE, PAGE_GOAL, PAGE_EXPERIENCE, PAGE_DAYS, PAGE_GYM, PAGE_GEAR, PAGE_SPOTS, PAGE_WEEK, PAGE_EXTRAS
)
private val SHORT_PATH = listOf(PAGE_MODE, PAGE_GOAL, PAGE_EXPERIENCE, PAGE_EXTRAS)

/** The pages this plan mode actually walks. Before a mode is picked the flow can't leave
 *  [PAGE_MODE], so the generated (longest) path stands in and the rail only ever shortens. */
private fun pathFor(planMode: String): List<Int> =
    if (planMode == PLAN_CUSTOM || planMode == PLAN_FREESTYLE) SHORT_PATH else GENERATED_PATH

/** Most of the world lifts in kg; the US (and Liberia / Myanmar) use lb. Seed the onboarding unit
 *  from the device locale so a non-US user isn't forced to flip a toggle. An empty country (locale
 *  carries only a language, common on emulators / minimal setups) is uninformative — fall back to
 *  the app's historical lb default rather than guessing kg. */
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
internal fun parseSaneBodyweightLb(input: String, unit: WeightUnit): Double? {
    val lb = parseToLb(input, unit) ?: return null
    return if (lb in MIN_BODYWEIGHT_LB..MAX_BODYWEIGHT_LB) lb else null
}

/** Boolean-unit bridge, matching [parseToLb]'s own legacy overload. Stones has no boolean. */
internal fun parseSaneBodyweightLb(input: String, useKg: Boolean): Double? =
    parseSaneBodyweightLb(input, if (useKg) WeightUnit.KG else WeightUnit.LB)

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

    // A draft written by this same schema always points inside its own path, but coerce anyway so a
    // corrupt cursor restarts the flow instead of indexing off the end.
    var step by remember { mutableIntStateOf(draft?.let { it.step.coerceIn(0, pathFor(it.planMode).lastIndex) } ?: 0) }
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
    // App-lock opt-in (GYMAP-69) — advisory; never blocks the CTA.
    var appLock by remember { mutableStateOf(draft?.appLock ?: false) }
    // null until the user touches the coach toggle, so the default can follow the plan mode: a
    // generated plan is what the coach adapts, freestyle has nothing to adapt.
    var coachChoice by remember { mutableStateOf(draft?.coachChoice) }
    var daysPerWeek by remember { mutableIntStateOf(draft?.daysPerWeek ?: 0) }
    var equipment by remember { mutableStateOf(draft?.equipment ?: emptySet()) }
    // Non-null when a curated preset (e.g. Developer's) is picked — locks the exercise pool.
    var frozenIds by remember { mutableStateOf(draft?.frozenIds) }
    // Locale-aware default: a kg lifter's "not sure" default is a round kg plate (10 kg), an lb
    // lifter's stays 15 lb. Stored in lb; the extras step shows it in the user's unit.
    var plateWeightLb by remember {
        mutableStateOf(draft?.plateWeightLb ?: if (localeDefaultUseKg()) fromDisplayWeight(10.0, true) else 15.0)
    }
    var problemAreas by remember { mutableStateOf(draft?.problemAreas ?: emptySet()) }
    var cadence by remember { mutableStateOf(draft?.cadence ?: "") }
    var everyN by remember { mutableIntStateOf(draft?.everyN ?: 4) }
    var previewSeed by remember { mutableLongStateOf(draft?.previewSeed ?: Random.nextLong()) }
    var showSkipConfirm by remember { mutableStateOf(false) }

    val path = pathFor(planMode)
    // Changing plan mode is only reachable from step 0, so the cursor can never outrun its path —
    // getOrElse keeps a hypothetical mismatch on the first page rather than crashing.
    val page = path.getOrElse(step) { PAGE_MODE }
    val isLast = step >= path.lastIndex
    val isGenerated = planMode == PLAN_GENERATED
    val coachEnabled = coachChoice ?: isGenerated

    // Persist a resume draft on every answer change (conflated in the ViewModel); completion
    // removes it atomically, so a finished user never resumes into a stale setup.
    val snapshot = OnboardingDraft(
        step, planMode, name, useKg, useMilesChoice, distanceTouched, goal, experience,
        bodyweightInput, sex, daysPerWeek, equipment, frozenIds, plateWeightLb,
        problemAreas, cadence, everyN, previewSeed, appLock, coachChoice
    )
    LaunchedEffect(snapshot) { viewModel.saveDraft(snapshot) }

    // The split the day-count implies — known without any gear, which is what lets the ledger draw
    // its empty tracks a step before the exercises exist.
    val archetypes = remember(daysPerWeek) {
        if (daysPerWeek in 1..7) SplitTemplates.forDays(daysPerWeek) else emptyList()
    }
    // The volume that split plans to carry, before any gear filter — what lets the ledger draw a
    // real week one step before the exercises exist.
    val plannedSets = remember(daysPerWeek, experience) {
        if (daysPerWeek in 1..7) ProgramGenerator.plannedSetsPerDay(daysPerWeek, experience.ifBlank { "intermediate" })
        else emptyList()
    }
    // Pure preview — recomputed whenever an input or the re-roll seed changes. Null until there is
    // gear to build from: the ledger draws empty tracks rather than inventing a week (§12).
    val previewDays = remember(previewSeed, daysPerWeek, equipment, frozenIds, goal, experience, problemAreas) {
        if (equipment.isEmpty() || daysPerWeek !in 1..7) null
        else viewModel.buildPreview(daysPerWeek, equipment, goal, experience, problemAreas, frozenIds, previewSeed)
    }

    fun finish() {
        val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
        viewModel.complete(
            planMode = planMode, name = name.trim(), useKg = useKg,
            useMiles = if (distanceTouched) useMilesChoice else null,
            sex = sex ?: "", bodyweightLb = bwLb,
            goal = goal, daysPerWeek = daysPerWeek, equipment = equipment,
            cadence = cadence.ifEmpty { "never" }, everyN = everyN, experience = experience,
            problemAreas = problemAreas, seed = previewSeed,
            plateWeightLb = plateWeightLb, frozenIds = frozenIds, coachEnabled = coachEnabled,
            appLock = appLock
        )
        onFinished(planMode)
    }

    // System Back walks the flow, matching the ← in the chrome (Android: Back always works, and
    // there is exactly one back affordance per page). At the first step it falls through and leaves.
    BackHandler(enabled = step > 0) { step-- }

    // Gate the CTA on the answers the generator can't work without.
    val bodyweightOk = bodyweightInput.isBlank() || parseSaneBodyweightLb(bodyweightInput, useKg) != null
    val canAdvance = when (page) {
        PAGE_MODE -> planMode.isNotEmpty()
        PAGE_GOAL -> goal.isNotEmpty()
        PAGE_EXPERIENCE -> experience.isNotEmpty()
        PAGE_DAYS -> daysPerWeek in 1..7
        PAGE_GYM, PAGE_GEAR -> equipment.isNotEmpty()
        PAGE_EXTRAS -> bodyweightOk
        else -> true
    }
    // Say why the CTA is held rather than leaving a silently greyed button.
    val gateHint = when {
        page == PAGE_GYM && equipment.isEmpty() ->
            "Pick a setup. Avex can't build your plan without knowing your gear."
        page == PAGE_GEAR && equipment.isEmpty() ->
            "Keep at least one piece on. Avex can't build your plan with nothing to train on."
        else -> null
    }
    val ctaLabel = when {
        !isLast -> "Continue"
        planMode == PLAN_CUSTOM -> "Build my plan"
        planMode == PLAN_FREESTYLE -> "Start logging"
        else -> "Start training"
    }
    val onCta: () -> Unit = { if (isLast) finish() else step++ }

    OnboardingScaffold(
        step = step,
        total = path.size,
        onBack = if (step > 0) ({ step-- }) else null,
        onSkip = if (!isLast) ({ showSkipConfirm = true }) else null,
        gateHint = gateHint,
        // The plan under construction — outside the page slider, so it holds still while the
        // questions move past it. The week page shows the same mark at full size instead.
        ledger = {
            AnimatedVisibility(
                visible = page == PAGE_DAYS || page == PAGE_GYM || page == PAGE_GEAR || page == PAGE_SPOTS,
                enter = fadeIn(ForgeMotion.enterTween()) + expandVertically(ForgeMotion.enterTween()),
                exit = fadeOut(ForgeMotion.exitTween()) + shrinkVertically(ForgeMotion.exitTween())
            ) {
                Column {
                    Spacer(Modifier.height(20.dp))
                    // The week takes the room the question doesn't need. On the day-count step it IS
                    // the answer, so it stands tall; the gym and gear steps need their grids, so it
                    // compacts to make way. Animated, because the same bars are being resized.
                    val trackHeight by animateDpAsState(
                        if (page == PAGE_DAYS) 148.dp else 72.dp,
                        ForgeMotion.standardTween(),
                        label = "ledger_height"
                    )
                    PlanLedger(
                        archetypes = archetypes,
                        plannedSets = plannedSets,
                        days = previewDays,
                        trackHeight = trackHeight
                    )
                }
            }
        },
        bottomBar = {
            if (page == PAGE_WEEK) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlineCapsule("Re-roll", onClick = { previewSeed = Random.nextLong() })
                    PrimaryCapsule(ctaLabel, onClick = onCta, modifier = Modifier.weight(1f))
                }
            } else {
                PrimaryCapsule(ctaLabel, onClick = onCta, enabled = canAdvance, modifier = Modifier.fillMaxWidth())
            }
        }
    ) {
            AnimatedContent(
                targetState = step,
                modifier = Modifier.weight(1f),
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    (slideInHorizontally(ForgeMotion.enterTween()) { it * dir } + fadeIn(ForgeMotion.enterTween())) togetherWith
                        (slideOutHorizontally(ForgeMotion.exitTween()) { -it * dir } + fadeOut(ForgeMotion.exitTween()))
                },
                label = "onboarding_page"
            ) { s ->
                Column(modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    when (path.getOrElse(s) { PAGE_MODE }) {
                        PAGE_MODE -> StepPlanMode(selected = planMode, onSelect = { planMode = it })
                        PAGE_GOAL -> StepGoal(selected = goal, onSelect = { goal = it })
                        PAGE_EXPERIENCE -> StepExperience(selected = experience, onSelect = { experience = it })
                        PAGE_DAYS -> StepDays(days = daysPerWeek, onChange = { daysPerWeek = it })
                        PAGE_GYM -> StepGymPresets(
                            selected = equipment,
                            frozenIds = frozenIds,
                            onSelectPreset = { preset ->
                                equipment = preset.equipment
                                frozenIds = preset.frozenIds
                            }
                        )
                        PAGE_GEAR -> StepFineTune(
                            selected = equipment,
                            onToggle = { code ->
                                equipment = if (code in equipment) equipment - code else equipment + code
                                // Hand-editing equipment leaves any curated preset.
                                frozenIds = null
                            }
                        )
                        PAGE_SPOTS -> StepSoreSpots(
                            selected = problemAreas,
                            equipment = equipment,
                            frozenIds = frozenIds,
                            onToggle = { code ->
                                problemAreas = if (code in problemAreas) problemAreas - code else problemAreas + code
                            }
                        )
                        PAGE_WEEK -> StepWeek(archetypes = archetypes, plannedSets = plannedSets, days = previewDays.orEmpty())
                        else -> StepExtras(
                            generated = isGenerated,
                            useKg = useKg, onWeightUnit = { useKg = it },
                            useMiles = if (distanceTouched) useMilesChoice else !useKg,
                            onDistanceUnit = { useMilesChoice = it; distanceTouched = true },
                            name = name, onNameChange = { name = it },
                            bodyweightInput = bodyweightInput, onBodyweightChange = { bodyweightInput = it },
                            sex = sex, onSexSelect = { sex = it },
                            coachEnabled = coachEnabled, onCoachToggle = { coachChoice = it },
                            appLock = appLock, onAppLockToggle = { appLock = it },
                            plateWeightLb = plateWeightLb, onPlateWeight = { plateWeightLb = it },
                            cadence = cadence, everyN = everyN,
                            onCadence = { c, n -> cadence = c; everyN = n }
                        )
                    }
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
                        PLAN_CUSTOM -> "You'll start with no plan yet. Build your own from the home " +
                            "screen, or generate one anytime in Settings → Program."
                        PLAN_FREESTYLE -> "You'll start with no fixed plan, just logging your workouts. " +
                            "Switch to a plan anytime in Settings → Program."
                        else -> "You'll start with a basic bodyweight program and default settings, with " +
                            "no plan tailored to your gym or goals. Set your equipment and goal, and " +
                            "generate a personalized program, anytime in Settings → Program."
                    }
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    showSkipConfirm = false
                    val bwLb = parseSaneBodyweightLb(bodyweightInput, useKg)
                    // Honor a mode the user already picked; default to generated for the usual
                    // early skip where nothing was chosen yet.
                    val effectiveMode = planMode.ifEmpty { PLAN_GENERATED }
                    viewModel.complete(
                        planMode = effectiveMode, name = name.trim(), useKg = useKg, sex = sex ?: "",
                        useMiles = if (distanceTouched) useMilesChoice else null,
                        bodyweightLb = bwLb, goal = "build_muscle", daysPerWeek = 4,
                        equipment = setOf(Equipment.BODYWEIGHT_ONLY.name), experience = "intermediate",
                        // Skipping never reaches the coach row, so the mode's own default stands.
                        coachEnabled = coachChoice ?: (effectiveMode == PLAN_GENERATED),
                        appLock = appLock
                    )
                    onFinished(effectiveMode)
                }) { Text("Skip anyway") }
            },
            dismissButton = {
                TextButton(onClick = { showSkipConfirm = false }) { Text("Keep setting up") }
            }
        )
    }
}
