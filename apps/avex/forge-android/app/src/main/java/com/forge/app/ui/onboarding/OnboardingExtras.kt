@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import com.forge.app.ui.common.ForgeBlockRow
import com.forge.app.ui.common.ForgeChoiceRow
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.ForgeSwitchRow
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.settings.SettingsIcons
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The one closing step, and the only page in the flow that asks for more than one thing.
 *
 * Everything here used to be its own screen in front of the plan: the name, the units, the body
 * numbers, the lock, the plate weight, the refresh cadence (2026-08-22). They are settings, so they
 * come last, together, after the week exists, every one optional and every one repeated in Settings.
 * Leaving the page untouched and pressing the CTA is a complete answer.
 *
 * **Refreshed 2026-09-26 as a grouped settings sheet.** The previous draft set bare rows, two
 * outlined M3 text fields and four walls of capsule chips straight onto the page, which read as a
 * form from an older Android. Now each subject is one rounded group of connected segments (the
 * current Android settings shape: a large outer radius, a tight inner one, 2dp seams), text is typed
 * inline on its own row rather than into a floating-label box, and every one-of-few choice is a
 * sliding segmented control whose thumb carries the app's selectable formula. The explainers moved
 * into the row they explain, so no line floats between groups.
 *
 * Order is by subject and closes on the two switches: who you are, how you measure, what you load,
 * how often it re-rolls, then the two things you turn on.
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
            StepCaption("All optional. Every one of these also lives in Settings.")
        }

        AboutYouGroup(
            useKg = useKg, name = name, onNameChange = onNameChange,
            bodyweightInput = bodyweightInput, onBodyweightChange = onBodyweightChange,
            sex = sex, onSexSelect = onSexSelect
        )

        ExtrasGroup(
            "Units",
            { ForgeChoiceRow("Weight", listOf("lb", "kg"), if (useKg) 1 else 0) { onWeightUnit(it == 1) } },
            { ForgeChoiceRow("Distance", listOf("mi", "km"), if (useMiles) 0 else 1) { onDistanceUnit(it == 0) } }
        )

        TrainingGroup(
            generated = generated, useKg = useKg,
            plateWeightLb = plateWeightLb, onPlateWeight = onPlateWeight,
            cadence = cadence, everyN = everyN, onCadence = onCadence
        )

        ExtrasGroup(
            "Turn on",
            {
                ForgeSwitchRow(
                    label = "Weekly coach",
                    description = "Reads your logs and proposes small changes each week. You approve each one.",
                    checked = coachEnabled,
                    onToggle = onCoachToggle
                )
            },
            { AppLockRow(enabled = appLock, onToggle = onAppLockToggle) }
        )

        OfflineNote()
    }
}

// ── Groups ───────────────────────────────────────────────────────────────────

/** A mono anchor over one group of connected rows ([ForgeRowGroup]), with an optional footnote. */
@Composable
private fun ExtrasGroup(
    label: String,
    vararg rows: @Composable () -> Unit,
    footer: (@Composable () -> Unit)? = null
) = ForgeGroupSection(label, footer = footer) { ForgeRowGroup(*rows) }

@Composable
private fun AboutYouGroup(
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
    val weightFocus = remember { FocusRequester() }
    // "" is an answer (rather not say); null is no answer yet, so no thumb is drawn.
    val sexIndex = when (sex) { "male" -> 0; "female" -> 1; "" -> 2; else -> -1 }
    ExtrasGroup(
        "About you",
        {
            ForgeFieldRow(
                label = "Name",
                value = name,
                onValueChange = onNameChange,
                placeholder = "Optional",
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Words,
                    imeAction = ImeAction.Next
                ),
                keyboardActions = KeyboardActions(onNext = { weightFocus.requestFocus() })
            )
        },
        {
            ForgeFieldRow(
                label = "Bodyweight",
                value = bodyweightInput,
                onValueChange = onBodyweightChange,
                placeholder = if (useKg) "e.g. 77" else "e.g. 170",
                suffix = unitLabel,
                isError = invalid,
                focusRequester = weightFocus,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done)
            )
        },
        {
            ForgeChoiceRow("Sex", listOf("Male", "Female", "Rather not"), sexIndex) {
                onSexSelect(listOf("male", "female", "")[it])
            }
        },
        footer = {
            if (invalid) {
                Text(
                    "Enter a weight between $minDisp and $maxDisp $unitLabel, or leave it blank.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
            } else {
                StepCaption("Your name greets you on Home. Bodyweight and sex scale the strength standards on Stats.")
            }
        }
    )
}

@Composable
private fun TrainingGroup(
    generated: Boolean,
    useKg: Boolean,
    plateWeightLb: Double,
    onPlateWeight: (Double) -> Unit,
    cadence: String,
    everyN: Int,
    onCadence: (String, Int) -> Unit
) {
    // Plate denominations in the user's OWN unit — a kg lifter shouldn't translate lb plates in
    // their head. Stored in lb (onPlateWeight); kg values convert on the way in.
    val plates = if (useKg) listOf(1.25, 2.5, 5.0, 10.0, 15.0, 20.0, 25.0)
    else listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0)
    val plateIndex = plates.indexOfFirst { abs(plateWeightLb - fromDisplayWeight(it, useKg)) < 0.05 }
    val plateRow: @Composable () -> Unit = {
        ForgeBlockRow(
            label = "Plate weight",
            meta = if (useKg) "kg" else "lb",
            explainer = "For plate-loaded machines. Not sure? Keep the default."
        ) {
            ForgeSlidingSegments(
                options = plates.map { if (it % 1.0 == 0.0) "${it.toInt()}" else "$it" },
                selectedIndex = plateIndex,
                onSelect = { onPlateWeight(fromDisplayWeight(plates[it], useKg)) },
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
    // Only a generated plan has exercises to re-roll.
    if (!generated) {
        ExtrasGroup("Training", plateRow)
        return
    }
    val refresh = listOf(0, 4, 8, 12)
    val refreshIndex = if (cadence != "every_n") 0 else refresh.indexOf(everyN)
    ExtrasGroup(
        "Training",
        plateRow,
        {
            ForgeBlockRow(
                label = "Auto-refresh",
                meta = "workouts",
                explainer = "Fresh movements after this many workouts. Same split."
            ) {
                ForgeSlidingSegments(
                    options = refresh.map { if (it == 0) "Never" else "$it" },
                    selectedIndex = refreshIndex,
                    onSelect = { i ->
                        if (i == 0) onCadence("never", everyN) else onCadence("every_n", refresh[i])
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }
    )
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/**
 * The app-lock opt-in (GYMAP-69). With no screen lock there is no credential to prompt against, so
 * the row renders inert rather than tappable-but-dead, and says what would make it work.
 */
@Composable
private fun AppLockRow(enabled: Boolean, onToggle: (Boolean) -> Unit) {
    val context = LocalContext.current
    val canAuth = remember { BiometricAuthenticator.canAuthenticate(context) }
    ForgeSwitchRow(
        label = "Lock Avex",
        description = if (canAuth) "Ask for your fingerprint, face or phone PIN when you open the app."
        else "Set a screen lock on your phone to use this.",
        checked = enabled,
        onToggle = onToggle,
        enabled = canAuth
    )
}

/** The offline promise, said once, quietly, under the last group. */
@Composable
private fun OfflineNote() {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(SettingsIcons.Security, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
        Text(
            "Everything stays on your phone. No account, no sign-up, no internet access.",
            style = MaterialTheme.typography.bodySmall,
            color = muted
        )
    }
}
