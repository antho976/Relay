package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.health.WearableBrand
import com.forge.app.domain.units.toDisplayWeight
import kotlin.math.roundToInt

/**
 * Steps 0–6: who you are, how you measure, and how you want to train. One decision per screen;
 * each page is `eyebrow → serif question → caption → content` (OnboardingPrimitives).
 * Steps 7–12 (gym, tuning, preview) live in OnboardingGymSteps.kt.
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

@Composable
internal fun StepWelcome(name: String, onNameChange: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("•", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.secondary)
            Text(
                "Avex",
                style = MaterialTheme.typography.bodyLarge,
                fontStyle = FontStyle.Italic,
                color = MaterialTheme.colorScheme.onBackground
            )
        }
        Spacer(Modifier.height(8.dp))
        StepTitle("What do you go by?")
        StepCaption("Optional, used only for the greeting.")
        OutlinedTextField(
            value = name, onValueChange = onNameChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Your name") },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
            singleLine = true
        )
        Spacer(Modifier.height(8.dp))
        BrandAside("Everything stays on your phone. No account, no sign-up, no internet access.")
    }
}

@Composable
internal fun StepUnits(
    useKg: Boolean,
    onWeightToggle: (Boolean) -> Unit,
    useMiles: Boolean,
    onDistanceToggle: (Boolean) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("About you")
        StepTitle("How do you measure?")
        StepCaption("You can change both anytime in Settings.")
        Spacer(Modifier.height(4.dp))
        StepSectionLabel("Weight")
        UnitSegment("Pounds · lb", "Kilograms · kg", secondSelected = useKg, onSelect = onWeightToggle)
        Spacer(Modifier.height(8.dp))
        StepSectionLabel("Distance")
        UnitSegment("Kilometers · km", "Miles · mi", secondSelected = useMiles, onSelect = onDistanceToggle)
    }
}

@Composable
internal fun StepBody(
    bodyweightInput: String,
    useKg: Boolean,
    onInputChange: (String) -> Unit,
    sex: String?,
    onSexSelect: (String) -> Unit
) {
    val unitLabel = if (useKg) "kg" else "lb"
    // A typed value that's blank-or-valid lets "Continue" proceed; anything outside the plausible
    // range would otherwise just disable it with no hint. Show why instead.
    val invalid = bodyweightInput.isNotBlank() && parseSaneBodyweightLb(bodyweightInput, useKg) == null
    val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, useKg).roundToInt()
    val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, useKg).roundToInt()
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("About you")
        StepTitle("About your body")
        StepCaption("Both optional. They scale the strength standards on Stats.")
        Spacer(Modifier.height(4.dp))
        StepSectionLabel("Bodyweight")
        OutlinedTextField(
            value = bodyweightInput, onValueChange = onInputChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("e.g. 170") },
            suffix = { Text(unitLabel) },
            isError = invalid,
            supportingText = if (invalid) {
                { Text("Enter a weight between $minDisp and $maxDisp $unitLabel, or leave it blank.") }
            } else null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true
        )
        Spacer(Modifier.height(8.dp))
        StepSectionLabel("Sex")
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ChoiceChip("Male", sex == "male", { onSexSelect("male") }, Modifier.weight(1f))
            ChoiceChip("Female", sex == "female", { onSexSelect("female") }, Modifier.weight(1f))
        }
        ChoiceChip("Prefer not to say", sex == "", { onSexSelect("") }, Modifier.fillMaxWidth())
    }
}

/**
 * Wearable options: the brand (label + source app from [WearableBrand]), how it feeds Health
 * Connect, the right-meta naming the one per-brand difference (Galaxy sends GPS routes, Fitbit
 * versions vary), the mono what-syncs readout, and the honest version caveat. Facts, not
 * plumbing — every Health Connect read is vendor-neutral.
 */
private data class WearableDetail(
    val brand: WearableBrand,
    val description: String,
    val meta: String?,
    val syncs: String?,
    val caveat: String?
)

private val WEARABLE_DETAILS = listOf(
    WearableDetail(
        WearableBrand.GALAXY,
        "Feeds Health Connect through Samsung Health.",
        meta = "Routes sync",
        syncs = "Sleep · steps · heart rate · workouts · GPS routes",
        caveat = "Signals depend on your watch and Samsung Health versions. Older models send fewer."
    ),
    WearableDetail(
        WearableBrand.PIXEL,
        "Feeds Health Connect through the Fitbit app.",
        meta = "Routes vary",
        syncs = "Sleep · steps · heart rate · workouts",
        caveat = "GPS routes and some signals depend on your Fitbit version. Older watches send fewer."
    ),
    WearableDetail(
        WearableBrand.NONE,
        "Any tracker that feeds Health Connect works too.",
        meta = null, syncs = null, caveat = null
    )
)

@Composable
internal fun StepWearable(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("About you")
        StepTitle("What's on your wrist?")
        StepCaption("Nothing connects yet. You grant each signal later in Settings.")
        Spacer(Modifier.height(2.dp))
        WEARABLE_DETAILS.forEach { detail ->
            OptionCard(
                label = detail.brand.label,
                description = detail.description,
                meta = detail.meta,
                selected = selected == detail.brand.key,
                onClick = { onSelect(detail.brand.key) }
            )
        }
        // The pick answers with what it sends (StepDays' split-readout idiom), plus the version
        // caveat — feature sets differ per watch generation and companion-app version.
        val detail = WEARABLE_DETAILS.firstOrNull { it.brand.key == selected }
        if (detail?.syncs != null) {
            Spacer(Modifier.height(4.dp))
            StepSectionLabel("What syncs")
            Text(
                detail.syncs.uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary
            )
            detail.caveat?.let { StepCaption(it) }
        }
    }
}

@Composable
internal fun StepPlanMode(selected: String, onSelect: (String) -> Unit) {
    // One coordinator for the card videos so they start — and therefore replay and freeze — together.
    val videoSync = remember { PlanModeSync(PLAN_MODE_DETAILS.count { planModeHasVideo(it.first) }) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Your plan")
        StepTitle("How do you want to train?")
        StepCaption("Nothing here is permanent. Change it later in Settings.")
        Spacer(Modifier.height(2.dp))
        PLAN_MODE_DETAILS.forEach { (key, label, desc) ->
            OptionCard(
                label = label,
                description = desc,
                selected = selected == key,
                // Maturity tags: custom/freestyle ship earlier-stage than the generated path.
                meta = when (key) {
                    PLAN_GENERATED -> "Recommended"
                    PLAN_CUSTOM -> "Beta"
                    else -> "Alpha"
                },
                onClick = { onSelect(key) },
                topContent = { PlanModeMedia(key, videoSync) }
            )
        }
    }
}

@Composable
internal fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Goals")
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
        StepEyebrow("Goals")
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
