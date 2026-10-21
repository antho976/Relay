package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.program.Equipment
import com.forge.app.program.GenerationParams
import com.forge.app.program.ProgramGenerator
import com.forge.app.program.SplitTemplates
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.onboarding.OnbPrimaryButton
import com.forge.app.ui.onboarding.OnbSecondaryButton
import com.forge.app.ui.onboarding.OnboardingScaffold
import com.forge.app.ui.onboarding.PLAN_GENERATED
import com.forge.app.ui.onboarding.SoFarAnswers
import com.forge.app.ui.onboarding.SoFarPage
import com.forge.app.ui.onboarding.StepDays
import com.forge.app.ui.onboarding.StepExperience
import com.forge.app.ui.onboarding.StepExtras
import com.forge.app.ui.onboarding.StepFineTune
import com.forge.app.ui.onboarding.StepGoal
import com.forge.app.ui.onboarding.StepGymPresets
import com.forge.app.ui.onboarding.StepPlanMode
import com.forge.app.ui.onboarding.StepSoreSpots
import com.forge.app.ui.onboarding.StepWeek
import com.forge.app.ui.onboarding.StepWeekdays
import com.forge.app.ui.onboarding.planSoFar
import com.forge.app.ui.onboarding.weekSets
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.OpenIcons
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.After
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Golden screenshots of the first run, redrawn 2026-10-21 to the Relay onboarding designs: every one
 * of the ten steps of the generated path, as a whole page (chrome, question, answers, plan so far and
 * the action), at the design's 412dp and at 384dp under a 1.3x font.
 *
 * Motion is off so the plan-mode vignettes hold their finished frame and the pages are stable.
 *
 * Record or re-record:  ./gradlew :app:recordRoborazziDebug --tests '*OnboardingScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class OnboardingScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    @Before fun still() { ForgeMotion.durationScale = 0f }
    @After fun moving() { ForgeMotion.durationScale = 1f }

    private fun shoot(
        name: String,
        fontScale: Float = 1f,
        scrollTo: String? = null,
        scrollToDescription: String? = null,
        content: @Composable () -> Unit
    ) {
        compose.setContent {
            ForgeTheme {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    CompositionLocalProvider(
                        LocalDensity provides Density(LocalDensity.current.density, fontScale = fontScale)
                    ) { content() }
                }
            }
        }
        if (scrollTo != null) {
            compose.onAllNodes(hasScrollAction()).onFirst().performScrollToNode(hasText(scrollTo, substring = true))
        }
        if (scrollToDescription != null) {
            compose.onAllNodes(hasScrollAction()).onFirst()
                .performScrollToNode(hasContentDescription(scrollToDescription, substring = true))
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    /** One whole page of the flow, framed as [OnboardingScreen] frames it. */
    @Composable
    private fun Page(
        step: Int,
        soFar: String?,
        cta: String = "Continue",
        enabled: Boolean = true,
        reroll: Boolean = false,
        gateHint: String? = null,
        body: @Composable () -> Unit
    ) {
        OnboardingScaffold(
            step = step,
            total = 10,
            onBack = if (step > 0) ({}) else null,
            onSkip = if (step < 9) ({}) else null,
            planSoFar = soFar,
            gateHint = gateHint,
            bottomBar = {
                if (reroll) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        OnbSecondaryButton("Re-roll", OpenIcons.Refresh, onClick = {})
                        OnbPrimaryButton(cta, onClick = {}, modifier = Modifier.weight(1f))
                    }
                } else {
                    OnbPrimaryButton(cta, onClick = {}, enabled = enabled, modifier = Modifier.fillMaxWidth())
                }
            }
        ) {
            Column(Modifier.weight(1f).fillMaxSize().verticalScroll(rememberScrollState())) { body() }
        }
    }

    // ── The fake answers: a 5-day build-muscle plan on a full gym ────────────────────────────────

    private val days = 5
    private val archetypes = SplitTemplates.forDays(days)
    private val planned = ProgramGenerator.plannedSetsPerDay(days, "intermediate", "build_muscle", 60)
    private val allGear = Equipment.entries.map { it.name }.toSet()
    private val someGear = Equipment.entries.filterIndexed { i, _ -> i % 3 != 2 }.map { it.name }.toSet()
    private val week = ProgramGenerator.generate(
        GenerationParams(daysPerWeek = days, goal = "build_muscle", experience = "intermediate", sessionMinutes = 60),
        Equipment.entries.toSet(), emptySet(), emptySet(), seed = 7L
    )
    private val answers = SoFarAnswers(
        planMode = PLAN_GENERATED, goal = "build_muscle", experience = "intermediate", daysPerWeek = days,
        sessionMinutes = 60, fixedDays = true, weekdays = setOf(0, 1, 3, 4, 5), equipment = allGear,
        dayNames = archetypes.map { it.name }, weekSets = weekSets(archetypes, planned, week).sum()
    )

    @Composable private fun Mode() = Page(0, planSoFar(SoFarPage.MODE, answers)) {
        StepPlanMode(selected = PLAN_GENERATED, onSelect = {})
    }

    @Composable private fun Goal() = Page(1, planSoFar(SoFarPage.GOAL, answers)) {
        StepGoal(selected = "build_muscle", onSelect = {})
    }

    @Composable private fun Experience() = Page(2, planSoFar(SoFarPage.EXPERIENCE, answers)) {
        StepExperience(selected = "intermediate", onSelect = {})
    }

    @Composable private fun Days(count: Int = days) = Page(3, planSoFar(SoFarPage.DAYS, answers.copy(daysPerWeek = count))) {
        val split = SplitTemplates.forDays(count)
        StepDays(
            days = count, experience = "intermediate", onChange = {},
            sessionMinutes = 60, onSessionMinutes = {},
            archetypes = split,
            plannedSets = ProgramGenerator.plannedSetsPerDay(count, "intermediate", "build_muscle", 60)
        )
    }

    @Composable private fun Weekdays(picked: Set<Int> = answers.weekdays) = Page(
        4, planSoFar(SoFarPage.WEEKDAYS, answers),
        enabled = picked.size == days,
        gateHint = if (picked.size == days) null else "Pick $days days. ${picked.size} picked so far."
    ) {
        StepWeekdays(daysPerWeek = days, fixedDays = true, weekdays = picked, onFixedDays = {}, onToggleWeekday = {})
    }

    @Composable private fun Gym() = Page(5, planSoFar(SoFarPage.GYM, answers)) {
        val everything = equipmentPresets.first()
        StepGymPresets(selected = everything.equipment, frozenIds = everything.frozenIds, onSelectPreset = {})
    }

    @Composable private fun Gear() = Page(6, planSoFar(SoFarPage.GEAR, answers.copy(equipment = someGear))) {
        StepFineTune(selected = someGear, onToggle = {})
    }

    @Composable private fun Spots() = Page(7, planSoFar(SoFarPage.SPOTS, answers.copy(problemAreas = setOf("shoulders")))) {
        StepSoreSpots(selected = setOf("shoulders"), equipment = allGear, frozenIds = null, onToggle = {})
    }

    @Composable private fun Week() = Page(8, planSoFar(SoFarPage.WEEK, answers), reroll = true) {
        StepWeek(archetypes = archetypes, plannedSets = planned, days = week)
    }

    @Composable private fun Extras(bodyweight: String = "") = Page(
        9, planSoFar(SoFarPage.EXTRAS, answers), cta = "Start training", enabled = bodyweight.isBlank() || bodyweight == "170"
    ) {
        StepExtras(
            generated = true, useKg = false, onWeightUnit = {}, useMiles = true, onDistanceUnit = {},
            name = "", onNameChange = {}, bodyweightInput = bodyweight, onBodyweightChange = {},
            sex = "male", onSexSelect = {}, coachEnabled = true, onCoachToggle = {},
            appLock = false, onAppLockToggle = {}, plateWeightLb = 15.0, onPlateWeight = {},
            cadence = "never", everyN = 4, onCadence = { _, _ -> },
            watchConnectable = true, canLock = true
        )
    }

    // ── 412dp, the design's width ─────────────────────────────────────────────────────────────────

    @Test fun mode() = shoot("onboarding-01-mode") { Mode() }
    @Test fun goal() = shoot("onboarding-02-goal") { Goal() }
    @Test fun experience() = shoot("onboarding-03-experience") { Experience() }
    @Test fun days() = shoot("onboarding-04-days") { Days() }
    @Test fun weekdays() = shoot("onboarding-05-weekdays") { Weekdays() }
    @Test fun gym() = shoot("onboarding-06-gym") { Gym() }
    @Test fun gear() = shoot("onboarding-07-gear") { Gear() }
    @Test fun spots() = shoot("onboarding-08-spots") { Spots() }
    @Test fun week() = shoot("onboarding-09-week") { Week() }
    @Test fun extras() = shoot("onboarding-10-extras") { Extras() }
    @Test fun extrasBelow() = shoot("onboarding-10-extras-below", scrollTo = "Everything stays") { Extras() }

    /** Seven days: the advice line, and seven bars sharing the row. */
    @Test fun daysSeven() = shoot("onboarding-04-days-seven") { Days(7) }

    /** Days not answered yet: zero, no knob, and the action held. */
    @Test fun daysUnanswered() = shoot("onboarding-04-days-unanswered") {
        Page(3, planSoFar(SoFarPage.DAYS, answers.copy(daysPerWeek = 0, sessionMinutes = null)), enabled = false) {
            StepDays(days = 0, experience = "intermediate", onChange = {})
        }
    }

    /** Weekdays short of the day count: the hint says why the action is held. */
    @Test fun weekdaysShort() = shoot("onboarding-05-weekdays-short") { Weekdays(setOf(0, 2, 4)) }

    /** A body weight outside the plausible range: the field and the line under it say so. */
    @Test fun extrasInvalid() = shoot("onboarding-10-extras-invalid") { Extras(bodyweight = "17") }

    // ── 384dp (Antho's phone) at a 1.3x font ─────────────────────────────────────────────────────

    @Config(qualifiers = NARROW) @Test fun modeNarrow() = shoot("onboarding-01-mode-384-130", 1.3f) { Mode() }
    @Config(qualifiers = NARROW) @Test fun goalNarrow() = shoot("onboarding-02-goal-384-130", 1.3f) { Goal() }
    @Config(qualifiers = NARROW) @Test fun experienceNarrow() = shoot("onboarding-03-experience-384-130", 1.3f) { Experience() }
    @Config(qualifiers = NARROW) @Test fun daysNarrow() = shoot("onboarding-04-days-384-130", 1.3f) { Days() }
    @Config(qualifiers = NARROW) @Test fun daysNarrowBelow() =
        shoot("onboarding-04-days-384-130-below", 1.3f, scrollToDescription = "Your week") { Days(7) }
    @Config(qualifiers = NARROW) @Test fun weekdaysNarrow() = shoot("onboarding-05-weekdays-384-130", 1.3f) { Weekdays() }
    @Config(qualifiers = NARROW) @Test fun gymNarrow() = shoot("onboarding-06-gym-384-130", 1.3f) { Gym() }
    @Config(qualifiers = NARROW) @Test fun gearNarrow() = shoot("onboarding-07-gear-384-130", 1.3f) { Gear() }
    @Config(qualifiers = NARROW) @Test fun spotsNarrow() = shoot("onboarding-08-spots-384-130", 1.3f) { Spots() }
    @Config(qualifiers = NARROW) @Test fun weekNarrow() = shoot("onboarding-09-week-384-130", 1.3f) { Week() }
    @Config(qualifiers = NARROW) @Test fun extrasNarrow() = shoot("onboarding-10-extras-384-130", 1.3f) { Extras() }
    @Config(qualifiers = NARROW) @Test fun extrasNarrowBelow() =
        shoot("onboarding-10-extras-384-130-below", 1.3f, scrollTo = "Everything stays") { Extras() }

    /** 384dp at the default font: the seven-day week and the gear grid, the two tightest rows. */
    @Config(qualifiers = NARROW) @Test fun daysSevenNarrow() = shoot("onboarding-04-days-seven-384") { Days(7) }
    @Config(qualifiers = NARROW) @Test fun gearNarrowDefault() = shoot("onboarding-07-gear-384") { Gear() }

    private companion object {
        const val NARROW = "w384dp-h832dp-normal-long-notround-any-xxhdpi-keyshidden-nonav"
    }
}
