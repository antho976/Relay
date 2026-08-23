package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * The plan-shaping questions, in the order the generator needs them: the plan-mode fork first (it
 * decides how much of the flow even runs), then goal, experience and day-count. From the day-count
 * on, the [PlanLedger] under the question is live — see [OnboardingScreen] for the page order and
 * `OnboardingGymSteps.kt` for the gym half.
 *
 * Every page is `serif question → one caption → content`. Nothing here asks for a setting: the
 * optional answers (name, units, body, watch, lock, plate weight, sore spots, refresh) all moved to
 * the one closing step in `OnboardingExtras.kt` (2026-08-22), after the week exists.
 */

/** Goal options: key, label, what it changes, and the mono rep-range meta. */
private val GOAL_DETAILS = listOf(
    listOf("build_muscle", "Build muscle", "Balanced for size. The default pick.", "8-12 reps"),
    listOf("get_stronger", "Get stronger", "Heavier work on the big lifts.", "4-6 reps"),
    listOf("lose_weight", "Lose weight", "Higher reps with more conditioning.", "12-20 reps"),
    listOf("general_fitness", "General fitness", "Balanced, all-round training.", "8-15 reps")
)

/** Experience bands — non-overlapping, mapped to the generator's level keys. */
private val EXPERIENCE_DETAILS = listOf(
    listOf("beginner", "New to lifting", "A bit less volume, no advanced lifts yet.", "Under 6 mo"),
    listOf("intermediate", "Got the basics down", "Standard volume.", "6 mo to 2 yr"),
    listOf("advanced", "Experienced", "A touch more volume.", "2+ yr")
)

/** Plan source — generated, self-built, or no plan at all. Each card carries its live vignette. */
private val PLAN_MODE_DETAILS = listOf(
    Triple(PLAN_GENERATED, "Build me a plan", "Avex picks your exercises from your gear and goal."),
    Triple(PLAN_CUSTOM, "I'll make my own", "Set your goal, then build your plan from scratch."),
    Triple(PLAN_FREESTYLE, "Go with the flow", "No fixed plan. Log what you did, whenever.")
)

/**
 * The fork, and the first thing a new install shows. It leads because it decides the length of
 * everything after it: "build me a plan" walks the gym steps to a finished week, the other two
 * answer goal and experience (which steer the coach) and go straight to the closing step.
 *
 * Each card carries a vignette video of its mode — see `PlanModeMedia.kt`.
 */
@Composable
internal fun StepPlanMode(selected: String, onSelect: (String) -> Unit) {
    // One coordinator for the card videos so they start — and therefore loop and freeze — together.
    val videoSync = remember { PlanModeSync(PLAN_MODE_DETAILS.count { planModeHasVideo(it.first) }) }
    // Tapping a card plays its vignette again. The illustration IS the answer to the question, so
    // choosing an option should show you the answer rather than leave you on a frozen last frame.
    // Per-card, because picking one must not restart the two you didn't pick.
    val replays = remember { mutableStateMapOf<String, Int>() }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("How do you want to train?")
        StepCaption("Nothing here is permanent. Change it later in Settings.")
        Spacer(Modifier.height(2.dp))
        PLAN_MODE_DETAILS.forEach { (key, label, desc) ->
            OptionCard(
                label = label,
                description = desc,
                selected = selected == key,
                // Maturity tags: the generated path is the pick; custom has shipped (no tag);
                // freestyle is still earlier-stage but out of alpha now.
                meta = when (key) {
                    PLAN_GENERATED -> "Recommended"
                    PLAN_CUSTOM -> null
                    else -> "Beta"
                },
                onClick = {
                    replays[key] = (replays[key] ?: 0) + 1
                    onSelect(key)
                },
                topContent = { PlanModeMedia(key, videoSync, replays[key] ?: 0) }
            )
        }
    }
}

@Composable
internal fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("What's your main goal?")
        StepCaption("Same exercises, different loading. Switch it anytime.")
        Spacer(Modifier.height(2.dp))
        GOAL_DETAILS.forEach { (key, label, desc, meta) ->
            OptionCard(
                label = label, description = desc, meta = meta,
                icon = OnboardingIcons.forGoal(key),
                selected = selected == key,
                onClick = { onSelect(key) }
            )
        }
    }
}

@Composable
internal fun StepExperience(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("How long have you been training?")
        StepCaption("Sets your volume and which movements you're given.")
        Spacer(Modifier.height(2.dp))
        EXPERIENCE_DETAILS.forEach { (key, label, desc, meta) ->
            OptionCard(
                label = label, description = desc, meta = meta,
                selected = selected == key,
                onClick = { onSelect(key) }
            )
        }
    }
}

/**
 * The first step the [PlanLedger] answers. The split readout that used to sit here as a line of
 * text ("PUSH · PULL · LEGS") is gone: the ledger's meters ARE the split, labelled day by day, and
 * saying it twice broke §4.3's one-home rule.
 */
@Composable
internal fun StepDays(days: Int, onChange: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("How many days a week?")
        StepCaption("Your split follows, and it becomes your weekly target on Home.")
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            (1..7).forEach { n -> DayChip(n, days == n) { onChange(n) } }
        }
    }
}
