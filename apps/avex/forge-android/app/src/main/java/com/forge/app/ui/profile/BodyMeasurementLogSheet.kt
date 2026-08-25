package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.parseToCm
import com.forge.app.ui.common.ForgePrimaryCapsule
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
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
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

    // Only fields the user actually changed from their seed — an untouched field is left as-is.
    fun isChanged(type: BodyMeasurementType): Boolean = inputs[type].orEmpty() != seeded[type].orEmpty()

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
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text("Log measurements", style = MaterialTheme.typography.headlineSmall, color = onBg)
            BodyMeasurementType.entries.forEach { type ->
                OutlinedTextField(
                    value = inputs[type].orEmpty(),
                    onValueChange = { v -> inputs[type] = filterDecimalInput(v) },
                    label = { Text("${type.label} ($unit)") },
                    singleLine = true,
                    isError = isInvalid(type),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.fillMaxWidth()
                )
            }
            Text(
                "One entry per measurement per day, saving replaces today's.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
            ForgePrimaryCapsule(
                label = "Save",
                onClick = { onSave(toSave) },
                enabled = canSave,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}
