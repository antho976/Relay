@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.theme.OnboardingType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
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
 * Redrawn 2026-10-21 to the "Open panel" designs: no boxes around the groups, a mono head over each
 * subject, the name on the left of each row and its answer on the right, typed answers in small
 * outlined fields and one-of-few answers as tight outlined choices.
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
    onCadence: (String, Int) -> Unit,
    watchConnectable: Boolean = false,
    watchConnected: Boolean = false,
    onConnectWatch: () -> Unit = {},
    canLock: Boolean = rememberCanLock()
) {
    Column {
        StepHeader("Anything else?", "All optional. Every one of these also lives in Settings.")
        Spacer(Modifier.height(28.dp))

        AboutYou(
            useKg = useKg, name = name, onNameChange = onNameChange,
            bodyweightInput = bodyweightInput, onBodyweightChange = onBodyweightChange,
            sex = sex, onSexSelect = onSexSelect
        )

        Spacer(Modifier.height(32.dp))
        StepSectionLabel("Units")
        Spacer(Modifier.height(12.dp))
        ChoiceLine("Weight", listOf("lb", "kg"), if (useKg) 1 else 0) { onWeightUnit(it == 1) }
        Spacer(Modifier.height(10.dp))
        ChoiceLine("Distance", listOf("mi", "km"), if (useMiles) 0 else 1) { onDistanceUnit(it == 0) }
        Spacer(Modifier.height(12.dp))
        Note("Body weight and sex scale the strength standards on Stats.")

        Spacer(Modifier.height(32.dp))
        Training(
            generated = generated, useKg = useKg,
            plateWeightLb = plateWeightLb, onPlateWeight = onPlateWeight,
            cadence = cadence, everyN = everyN, onCadence = onCadence
        )

        Spacer(Modifier.height(32.dp))
        StepSectionLabel("Turn on")
        Spacer(Modifier.height(8.dp))
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            OnbSwitchRow(
                label = "Weekly coach",
                description = "Reads your logs and proposes small changes each week. You approve each one.",
                checked = coachEnabled,
                onToggle = onCoachToggle
            )
            // With no screen lock there is no credential to prompt against, so the row renders inert
            // rather than tappable-but-dead, and says what would make it work (GYMAP-69).
            OnbSwitchRow(
                label = "Lock Avex",
                description = if (canLock) "Ask for your fingerprint, face or phone PIN when you open the app."
                else "Set a screen lock on your phone to use this.",
                checked = appLock,
                onToggle = onAppLockToggle,
                enabled = canLock
            )
            if (watchConnectable) {
                // Opens Health Connect's own screen, where each reading is its own switch. Once
                // granted the row reads as on; turning it off lives in Settings → Watch & health data.
                OnbSwitchRow(
                    label = "Watch or ring",
                    description = if (watchConnected) "Connected. Sleep, heart rate and steps show on Home and Stats."
                    else "Sleep, heart rate and steps through Health Connect, for recovery calls and your Body stats.",
                    checked = watchConnected,
                    onToggle = { if (!watchConnected) onConnectWatch() }
                )
            }
        }

        Spacer(Modifier.height(28.dp))
        OfflineNote()
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun rememberCanLock(): Boolean {
    val context = LocalContext.current
    return remember { BiometricAuthenticator.canAuthenticate(context) }
}

@Composable
private fun AboutYou(
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
    // "" is an answer (rather not say); null is no answer yet, so nothing is picked.
    val sexIndex = when (sex) { "male" -> 0; "female" -> 1; "" -> 2; else -> -1 }

    StepSectionLabel("About you")
    Spacer(Modifier.height(12.dp))
    FieldLine(
        label = "Name",
        value = name,
        onValueChange = onNameChange,
        placeholder = "Optional",
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Next),
        keyboardActions = KeyboardActions(onNext = { weightFocus.requestFocus() })
    )
    Spacer(Modifier.height(10.dp))
    FieldLine(
        label = "Body weight",
        value = bodyweightInput,
        onValueChange = onBodyweightChange,
        placeholder = if (useKg) "e.g. 77" else "e.g. 170",
        suffix = unitLabel,
        isError = invalid,
        focusRequester = weightFocus,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done)
    )
    if (invalid) {
        Spacer(Modifier.height(8.dp))
        Text(
            "Enter a weight between $minDisp and $maxDisp $unitLabel, or leave it blank.",
            style = OpenType.bodyS,
            color = MaterialTheme.colorScheme.error
        )
    }
    Spacer(Modifier.height(10.dp))
    ChoiceLine("Sex", listOf("Male", "Female", "Rather not"), sexIndex) {
        onSexSelect(listOf("male", "female", "")[it])
    }
}

@Composable
private fun Training(
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
    StepSectionLabel("Training")
    Spacer(Modifier.height(12.dp))
    ChoiceLine(
        "Plate weight",
        plates.map { if (it % 1.0 == 0.0) "${it.toInt()}" else "$it" },
        plateIndex
    ) { onPlateWeight(fromDisplayWeight(plates[it], useKg)) }
    Spacer(Modifier.height(8.dp))
    // The plates are in the weight unit above; said here because the choices carry bare numbers.
    Note("In ${if (useKg) "kg" else "lb"}, for plate-loaded machines. Not sure? Keep the default.")
    // Only a generated plan has exercises to re-roll.
    if (generated) {
        val refresh = listOf(0, 4, 8, 12)
        val refreshIndex = if (cadence != "every_n") 0 else refresh.indexOf(everyN)
        Spacer(Modifier.height(16.dp))
        ChoiceLine("Auto-refresh", refresh.map { if (it == 0) "Never" else "$it" }, refreshIndex) { i ->
            if (i == 0) onCadence("never", everyN) else onCadence("every_n", refresh[i])
        }
        Spacer(Modifier.height(8.dp))
        Note("Fresh movements after this many workouts. Same split.")
    }
}

/** A row's name on the left, set in the row-title voice. */
@Composable
private fun LineLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier = modifier, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
}

/**
 * A one-of-few answer: the name on the left at its own width, the options as tight outlined choices
 * on the right. When the options outgrow the row (seven kilo plates, or a large font) they wrap
 * onto a second line, still right-aligned, instead of squeezing the name.
 */
@Composable
private fun ChoiceLine(
    label: String,
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit
) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        LineLabel(label, Modifier.padding(end = 12.dp))
        FlowRow(
            Modifier.weight(1f),
            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            options.forEachIndexed { i, option ->
                OnbChip(option, selected = i == selectedIndex, onClick = { onSelect(i) })
            }
        }
    }
}

/** A typed answer: the name on the left, a 160dp outlined field on the right. */
@Composable
private fun FieldLine(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    suffix: String? = null,
    isError: Boolean = false,
    focusRequester: FocusRequester? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default
) {
    val shape = RoundedCornerShape(4.dp)
    val ink = MaterialTheme.colorScheme.onBackground
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        LineLabel(label, Modifier.weight(1f))
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            singleLine = true,
            textStyle = OnboardingType.field.copy(color = ink),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions,
            modifier = Modifier
                .width(160.dp)
                .semantics { contentDescription = label }
                .then(if (focusRequester != null) Modifier.focusRequester(focusRequester) else Modifier),
            decorationBox = { inner ->
                Row(
                    Modifier
                        .heightIn(min = 40.dp)
                        .border(1.dp, if (isError) MaterialTheme.colorScheme.error else OpenColors.Rule, shape)
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Box(Modifier.weight(1f)) {
                        if (value.isEmpty()) {
                            Text(
                                if (suffix != null) "$placeholder $suffix" else placeholder,
                                style = OnboardingType.field,
                                color = OpenColors.Glyph
                            )
                        }
                        inner()
                    }
                    if (suffix != null && value.isNotEmpty()) {
                        Text(suffix, style = OnboardingType.field, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        )
    }
}

/** A one-line explainer under a group. */
@Composable
private fun Note(text: String) {
    Text(text, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** The offline promise, said once, quietly, under the last group. */
@Composable
private fun OfflineNote() {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(OpenIcons.Lock, contentDescription = null, tint = muted, modifier = Modifier.size(16.dp))
        Text(
            "Everything stays on your phone. No account, no sign-up, no internet access.",
            style = OpenType.bodyS,
            color = muted
        )
    }
}
