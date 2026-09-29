package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.parseToCm
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.domain.units.filterDecimalInput

/**
 * Quick-log sheet for body measurements (GYMAP-52) — one field per type, each seeded with its latest
 * reading so a small edit round-trips. Blank fields are ignored; Save records every field that holds
 * a sane value (one entry per type per day, replacing today's). Mirrors [BodyweightLogSheet].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BodyMeasurementLogSheet(
    series: List<MeasurementSeries>,
    useCm: Boolean,
    onSave: (List<Pair<BodyMeasurementType, Double>>) -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val sheetState = rememberModalBottomSheetState()
    val unit = lengthUnitLabel(useCm)

    // The value each field is SEEDED with (latest reading, or "" if never logged). Kept separate from
    // the live edits so Save can skip untouched fields — otherwise opening the sheet to log one
    // measurement would silently re-record every other tracked measurement (with its old value) as a
    // fresh entry dated today, distorting each series' trend/delta. Re-keyed on the unit so flipping
    // cm/in re-seeds in the new unit instead of leaving stale values.
    val seeded = remember(series, useCm) {
        series.associate { s ->
            s.type to (s.entries.lastOrNull()?.let { lengthInputValue(it.valueCm, useCm) } ?: "")
        }
    }
    val inputs = remember(series, useCm) {
        mutableStateMapOf<BodyMeasurementType, String>().apply { putAll(seeded) }
    }

    fun parsedCm(type: BodyMeasurementType): Double? =
        inputs[type]?.takeIf { it.isNotBlank() }
            ?.let { parseToCm(it, useCm) }
            ?.takeIf { it in BodyMeasurementType.MIN_CM..BodyMeasurementType.MAX_CM }

    fun isInvalid(type: BodyMeasurementType): Boolean {
        val raw = inputs[type].orEmpty()
        return raw.isNotBlank() && parsedCm(type) == null
    }

    // Fields the user typed into, even if the text ended up equal to the seed: re-measuring 85 when
    // 85 was last logged is still today's reading, and retyping it must be able to record it.
    var touched by remember(series, useCm) { mutableStateOf(emptySet<BodyMeasurementType>()) }

    // Only fields the user actually changed or retyped — an untouched field is left as-is.
    fun isChanged(type: BodyMeasurementType): Boolean =
        type in touched || inputs[type].orEmpty() != seeded[type].orEmpty()

    val toSave = BodyMeasurementType.entries
        .filter { isChanged(it) }
        .mapNotNull { t -> parsedCm(t)?.let { t to it } }
    val canSave = toSave.isNotEmpty() && BodyMeasurementType.entries.none { isInvalid(it) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        // §5: a modal is a `surface` fill — M3 defaults to the unthemed `surfaceContainerLow`.
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        val types = BodyMeasurementType.entries
        val focusers = remember { types.map { FocusRequester() } }
        val focus = LocalFocusManager.current
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            Text("Log measurements", style = MaterialTheme.typography.headlineSmall, color = onBg)
            // ONE group of inline rows, a site per row, with its unit at the end. Next walks the
            // group top to bottom; the last row closes the keyboard.
            ForgeGroupSection(
                label = null,
                footer = { ForgeGroupCaption("One entry per measurement per day, saving replaces today's. Hold a site's row to remove a past reading.") }
            ) {
                ForgeRowGroup(*types.mapIndexed { i, type ->
                    @Composable {
                        val last = i == types.lastIndex
                        ForgeFieldRow(
                            label = type.label,
                            value = inputs[type].orEmpty(),
                            onValueChange = { v -> inputs[type] = filterDecimalInput(v); touched = touched + type },
                            placeholder = "0",
                            suffix = unit,
                            isError = isInvalid(type),
                            focusRequester = focusers[i],
                            keyboardOptions = KeyboardOptions(
                                keyboardType = KeyboardType.Decimal,
                                imeAction = if (last) ImeAction.Done else ImeAction.Next
                            ),
                            keyboardActions = KeyboardActions(
                                onNext = { focusers.getOrNull(i + 1)?.requestFocus() },
                                onDone = { focus.clearFocus() }
                            )
                        )
                    }
                }.toTypedArray())
            }
            ForgePrimaryCapsule(
                label = "Save",
                onClick = { onSave(toSave) },
                enabled = canSave,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}
