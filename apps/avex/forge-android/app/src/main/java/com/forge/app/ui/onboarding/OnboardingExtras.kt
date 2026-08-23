@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.security.BiometricAuthenticator
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The one closing step, and the only page in the flow that asks for more than one thing.
 *
 * Everything here used to be its own screen in front of the plan: the name, the units, the body
 * numbers, the watch, the app lock, the plate weight, the refresh cadence — seven settings collected
 * before the app had shown the user anything (2026-08-22). They are settings, so they come last,
 * together, after the week exists, every one optional and every one repeated in Settings. Leaving
 * the page untouched and pressing the CTA is a complete answer.
 *
 * The watch question left this page entirely (2026-08-23). Picking a brand here changed nothing the
 * user could see — it only tailored the wording of Settings → Wearable's sync pointers — so it was a
 * question whose answer had no consequence, asked while the user was still in the flow. Wearable
 * setup is a Settings job that needs the Health Connect grants anyway, and that is where it lives.
 *
 * **Its register is a settings sheet, not eight more questions.** This page reads down a single
 * label-left spine wherever a control is small enough to sit beside its name — which is what a units
 * row is — and spends the full width only on what needs it: two text fields, two chip groups, two
 * switches. The first draft gave every one of them a full-width block of identical capsules and read
 * as a wall with no rank in it. The sore-spot question left for [StepSoreSpots] in the same pass.
 *
 * Order is by subject and closes on the two switches, so the page ends on a pair rather than
 * trailing off: who you are, how you measure, what you load, how often it re-rolls, then the two
 * things you turn on. The coach question came off an `AlertDialog` here too: a dialog
 * is for a decision that must interrupt (§12); an opt-in with a sane default is a row.
 */
@Composable
internal fun StepExtras(
    generated: Boolean,
    useKg: Boolean,
    onWeightUnit: (Boolean) -> Unit,
    useMiles: Boolean,
    onDistanceUnit: (Boolean) -> Unit,
    name: String,
    onNameChange: (String) -> Unit,
    bodyweightInput: String,
    onBodyweightChange: (String) -> Unit,
    sex: String?,
    onSexSelect: (String) -> Unit,
    coachEnabled: Boolean,
    onCoachToggle: (Boolean) -> Unit,
    appLock: Boolean,
    onAppLockToggle: (Boolean) -> Unit,
    plateWeightLb: Double,
    onPlateWeight: (Double) -> Unit,
    cadence: String,
    everyN: Int,
    onCadence: (String, Int) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StepTitle("Anything else?")
            StepCaption("Optional, all of it. Every one of these also lives in Settings.")
        }

        AboutYouSection(
            useKg = useKg, name = name, onNameChange = onNameChange,
            bodyweightInput = bodyweightInput, onBodyweightChange = onBodyweightChange,
            sex = sex, onSexSelect = onSexSelect
        )

        // Two either/or choices, each beside its own name. Stacked as full-width segments they read
        // as one 2x2 grid of four options with two of them lit, which is not what they are.
        ExtrasSection("Units") {
            ValueRow("Weight") {
                UnitChip("lb", !useKg) { onWeightUnit(false) }
                UnitChip("kg", useKg) { onWeightUnit(true) }
            }
            ValueRow("Distance") {
                UnitChip("mi", useMiles) { onDistanceUnit(true) }
                UnitChip("km", !useMiles) { onDistanceUnit(false) }
            }
        }

        PlateWeightSection(plateWeightLb = plateWeightLb, useKg = useKg, onSet = onPlateWeight)

        // Only a generated plan has exercises to re-roll.
        if (generated) {
            ExtrasSection("Auto-refresh") {
                StepCaption("Re-rolls your exercises after this many workouts. Same split, fresh movements.")
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    ChoiceChip("Never", cadence != "every_n", onClick = { onCadence("never", everyN) })
                    listOf(4, 8, 12).forEach { n ->
                        ChoiceChip("Every $n", cadence == "every_n" && everyN == n, onClick = { onCadence("every_n", n) })
                    }
                }
            }
        }

        // The page closes on its two switches, so the last thing before the CTA is a pair rather
        // than a fourth chip group.
        ExtrasSection("Turn on?") {
            ToggleRow(
                label = "Weekly coach",
                description = "Reads your logs and proposes small changes each week. You approve each one.",
                checked = coachEnabled,
                onToggle = onCoachToggle
            )
            AppLockRow(enabled = appLock, onToggle = onAppLockToggle)
        }

        BrandAside("Everything stays on your phone. No account, no sign-up, no internet access.")
    }
}

/** Mono anchor, air, content — the settings section rhythm (§7), no dividers. */
@Composable
private fun ExtrasSection(label: String, meta: String? = null, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel(label, meta)
        content()
    }
}

/**
 * The page's spine: a control's name on the left, the control itself on the right. Everything that
 * fits beside its label goes here, so the eye runs down one edge instead of down a stack of
 * full-width slabs. The label takes the slack, so it wraps rather than squeezing the control at
 * large font scales (§14).
 */
@Composable
private fun ValueRow(label: String, content: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f)
        )
        content()
    }
}

/** A unit capsule, sized so the two in a pair match whichever abbreviation is wider. */
@Composable
private fun UnitChip(label: String, selected: Boolean, onClick: () -> Unit) =
    ChoiceChip(label, selected, onClick, Modifier.widthIn(min = 60.dp))

@Composable
private fun AboutYouSection(
    useKg: Boolean,
    name: String,
    onNameChange: (String) -> Unit,
    bodyweightInput: String,
    onBodyweightChange: (String) -> Unit,
    sex: String?,
    onSexSelect: (String) -> Unit
) {
    val unitLabel = if (useKg) "kg" else "lb"
    // A typed value that is blank-or-valid lets the CTA proceed; anything outside the plausible
    // range would otherwise vanish silently on finish. Say why instead.
    val invalid = bodyweightInput.isNotBlank() && parseSaneBodyweightLb(bodyweightInput, useKg) == null
    val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, useKg).roundToInt()
    val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, useKg).roundToInt()
    ExtrasSection("About you") {
        StepCaption("Your name greets you on Home. Bodyweight and sex scale the strength standards on Stats.")
        OutlinedTextField(
            value = name, onValueChange = onNameChange,
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Name") },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
            singleLine = true
        )
        OutlinedTextField(
            value = bodyweightInput, onValueChange = onBodyweightChange,
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Bodyweight") },
            placeholder = { Text("e.g. 170") },
            suffix = { Text(unitLabel) },
            isError = invalid,
            supportingText = if (invalid) {
                { Text("Enter a weight between $minDisp and $maxDisp $unitLabel, or leave it blank.") }
            } else null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true
        )
        Spacer(Modifier.height(2.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            ChoiceChip("Male", sex == "male", onClick = { onSexSelect("male") })
            ChoiceChip("Female", sex == "female", onClick = { onSexSelect("female") })
            ChoiceChip("Prefer not to say", sex == "", onClick = { onSexSelect("") })
        }
    }
}

/**
 * The app-lock opt-in (GYMAP-69). Availability decides whether it can be turned on at all — with no
 * screen lock there is no credential to prompt against — so the row renders inert rather than
 * tappable-but-dead, and says what would make it work.
 */
@Composable
private fun AppLockRow(enabled: Boolean, onToggle: (Boolean) -> Unit) {
    val context = LocalContext.current
    val canAuth = remember { BiometricAuthenticator.canAuthenticate(context) }
    ToggleRow(
        label = "Lock Avex",
        description = if (canAuth) "Ask for your fingerprint, face or phone PIN when you open the app."
        else "Set a screen lock on your phone to use this.",
        checked = enabled,
        onToggle = onToggle,
        enabled = canAuth
    )
}

@Composable
private fun PlateWeightSection(plateWeightLb: Double, useKg: Boolean, onSet: (Double) -> Unit) {
    // Plate denominations in the user's OWN unit — a kg lifter shouldn't translate lb plates in
    // their head. Stored in lb (onSet); kg chips convert on the way in via the shared converter.
    val plates = if (useKg) listOf(1.25, 2.5, 5.0, 10.0, 15.0, 20.0, 25.0)
    else listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0)
    val unit = if (useKg) "kg" else "lb"
    // The unit rides the anchor, not every chip: six chips of "15 lb" wrapped onto a second row for
    // one orphan, and the six numbers say the same thing in one row.
    ExtrasSection("Plate weight", meta = unit) {
        StepCaption("For machines loaded by counting plates. Not sure? Keep the default.")
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            plates.forEach { value ->
                val lb = fromDisplayWeight(value, useKg)
                val label = if (value % 1.0 == 0.0) "${value.toInt()}" else "$value"
                ChoiceChip(label, abs(plateWeightLb - lb) < 0.05, onClick = { onSet(lb) })
            }
        }
    }
}
