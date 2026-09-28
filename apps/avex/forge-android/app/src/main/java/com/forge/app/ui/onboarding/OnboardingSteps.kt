package com.forge.app.ui.onboarding

import com.forge.app.ui.common.ForgeChoice
import com.forge.app.ui.common.ForgeChoiceList
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSlidingSegments
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeWarning

/**
 * The plan-shaping questions, in the order the generator needs them: the plan-mode fork first (it
 * decides how much of the flow even runs), then goal, experience and day-count. The day-count page
 * draws the [PlanLedger] under its question — see [OnboardingScreen] for the page order and
 * `OnboardingGymSteps.kt` for the gym half.
 *
 * Every page is `serif question → one caption → content`. Nothing here asks for a setting: the
 * optional answers (name, units, body, watch, lock, plate weight, sore spots, refresh) all moved to
 * the one closing step in `OnboardingExtras.kt` (2026-08-22), after the week exists.
 */

/** Goal options: key, label, what it changes, and the mono rep-range meta. */
internal val GOAL_DETAILS = listOf(
    listOf("build_muscle", "Build muscle", "Balanced for size. The default pick.", "8-12 reps"),
    listOf("get_stronger", "Get stronger", "Heavier work on the big lifts.", "4-6 reps"),
    listOf("lose_weight", "Lose weight", "Higher reps with more conditioning.", "12-20 reps"),
    listOf("general_fitness", "General fitness", "Balanced, all-round training.", "8-15 reps")
)

/** Experience bands — non-overlapping, mapped to the generator's level keys. */
internal val EXPERIENCE_DETAILS = listOf(
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
        Spacer(Modifier.height(6.dp))
        ForgeChoiceList(
            choices = PLAN_MODE_DETAILS.map { (key, label, desc) ->
                // Maturity tags: the generated path is the pick; custom has shipped (no tag);
                // freestyle is still earlier-stage but out of alpha now.
                ForgeChoice(
                    key, label, desc,
                    meta = when (key) {
                        PLAN_GENERATED -> "Recommended"
                        PLAN_CUSTOM -> null
                        else -> "Beta"
                    }
                )
            },
            selected = selected,
            onSelect = { key ->
                replays[key] = (replays[key] ?: 0) + 1
                onSelect(key)
            },
            topContent = { PlanModeMedia(it.key, videoSync, replays[it.key] ?: 0) }
        )
    }
}

@Composable
internal fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("What's your main goal?")
        StepCaption("Same exercises, different loading. Switch it anytime.")
        Spacer(Modifier.height(6.dp))
        ForgeChoiceList(
            choices = GOAL_DETAILS.map { (key, label, desc, meta) ->
                ForgeChoice(key, label, desc, meta, OnboardingIcons.forGoal(key))
            },
            selected = selected,
            onSelect = onSelect
        )
    }
}

@Composable
internal fun StepExperience(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("How long have you been training?")
        StepCaption("Sets your volume and which movements you're given.")
        Spacer(Modifier.height(6.dp))
        ForgeChoiceList(
            choices = EXPERIENCE_DETAILS.map { (key, label, desc, meta) ->
                ForgeChoice(key, label, desc, meta, OnboardingIcons.forExperience(key))
            },
            selected = selected,
            onSelect = onSelect
        )
    }
}

/**
 * The first step the [PlanLedger] answers. The split readout that used to sit here as a line of
 * text ("PUSH · PULL · LEGS") is gone: the ledger's meters ARE the split, labelled day by day, and
 * saying it twice broke the one-home rule. The count is a sliding segmented control in its own
 * group, the same control every one-of-few answer uses in this flow.
 */
@Composable
internal fun StepDays(days: Int, experience: String, onChange: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("How many days a week?")
        StepCaption("Your split follows, and it becomes your weekly target on Home.")
        Spacer(Modifier.height(6.dp))
        val advice = daysAdvice(days, experience)
        ForgeGroupSection(
            label = null,
            footer = if (advice != null) ({ DaysAdvice(advice) }) else null
        ) {
            ForgeRowGroup({
                ForgeSlidingSegments(
                    options = (1..7).map { "$it" },
                    selectedIndex = if (days in 1..7) days - 1 else -1,
                    onSelect = { onChange(it + 1) },
                    modifier = Modifier.fillMaxWidth().padding(10.dp)
                )
            })
        }
    }
}

/**
 * What a day-count outside 3 to 5 costs you, or null inside it. Advice, never a gate: the CTA stays
 * live, because someone with two free evenings a week still gets a real plan. Each line names what
 * that count costs (the pace, the rest days) rather than repeating one cue.
 */
internal fun daysAdvice(days: Int, experience: String): String? = when (days) {
    1 -> "One day a week maintains more than it builds. Most people progress on 3 to 5."
    2 -> "Two days works, just slower. Most people progress fastest on 3 to 5."
    6 -> if (experience == "advanced") "Six days leaves one rest day. Fine if you recover well, but 3 to 5 suits most."
        else "Six days leaves one rest day, which is hard to recover from. 3 to 5 suits most people."
    7 -> "Seven days leaves no rest day to recover on. 3 to 5 suits most people."
    else -> null
}

/**
 * The off-range note under the day chips: a warning dot (the exception is the only thing that earns a
 * dot, §8) beside onBg prose, so the line reads under every accent without colouring body text (§14).
 */
@Composable
private fun DaysAdvice(text: String) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(top = 2.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Box(
            Modifier
                .padding(top = 6.dp)
                .size(7.dp)
                .background(ForgeWarning, CircleShape)
        )
        Text(
            text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onBackground
        )
    }
}
