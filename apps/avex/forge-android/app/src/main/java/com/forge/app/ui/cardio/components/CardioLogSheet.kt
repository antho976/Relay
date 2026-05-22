package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType

/**
 * Modal entry form. Hides distance + effort when type = REST and shows rest-reason
 * chips in their place — keeps the sheet short for both flows. Form state is local
 * `remember`; the VM only sees the final values via [onLog].
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun CardioLogSheet(
    onDismiss: () -> Unit,
    onLog: (
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?
    ) -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var type by remember { mutableStateOf(CardioType.RUN) }
    var durationText by remember { mutableStateOf("") }
    var distanceText by remember { mutableStateOf("") }
    var effort by remember { mutableStateOf<CardioEffort?>(null) }
    var restReason by remember { mutableStateOf<CardioRestReason?>(null) }
    var note by remember { mutableStateOf("") }

    val durationInt = durationText.toIntOrNull() ?: 0
    val distanceDouble = distanceText.toDoubleOrNull()
    val canSubmit = if (type.isRest) restReason != null else durationInt > 0

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                "LOG CARDIO",
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.Black,
                style = MaterialTheme.typography.labelLarge
            )

            FieldLabel("Activity")
            ChipRow {
                CardioType.entries.forEach { t ->
                    FilterChip(
                        selected = type == t,
                        onClick = { type = t },
                        label = { Text(t.displayName) }
                    )
                }
            }

            if (!type.isRest) {
                FieldLabel("Duration (minutes)")
                OutlinedTextField(
                    value = durationText,
                    onValueChange = { durationText = it.filter(Char::isDigit).take(4) },
                    placeholder = { Text("e.g. 30") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.fillMaxWidth()
                )

                FieldLabel("Distance (km) — optional")
                OutlinedTextField(
                    value = distanceText,
                    onValueChange = { distanceText = sanitizeDecimal(it) },
                    placeholder = { Text("e.g. 5.2") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.fillMaxWidth()
                )

                FieldLabel("Effort — optional")
                ChipRow {
                    CardioEffort.entries.forEach { e ->
                        FilterChip(
                            selected = effort == e,
                            onClick = { effort = if (effort == e) null else e },
                            label = { Text(e.displayName) }
                        )
                    }
                }
            } else {
                FieldLabel("Reason")
                ChipRow {
                    CardioRestReason.entries.forEach { r ->
                        FilterChip(
                            selected = restReason == r,
                            onClick = { restReason = r },
                            label = { Text(r.displayName) }
                        )
                    }
                }
            }

            FieldLabel("Note — optional")
            OutlinedTextField(
                value = note,
                onValueChange = { note = it.take(200) },
                placeholder = { Text("How did it feel?") },
                singleLine = false,
                minLines = 2,
                maxLines = 4,
                modifier = Modifier.fillMaxWidth()
            )

            Button(
                onClick = {
                    onLog(
                        type,
                        if (type.isRest) 0 else durationInt,
                        if (type.isRest) null else distanceDouble,
                        if (type.isRest) null else effort,
                        if (type.isRest) restReason else null,
                        note.ifBlank { null }
                    )
                },
                enabled = canSubmit,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp, bottom = 16.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.primary
                )
            ) {
                Text(if (type.isRest) "Log rest day" else "Log cardio")
            }
        }
    }
}

@Composable
private fun FieldLabel(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontWeight = FontWeight.SemiBold
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChipRow(content: @Composable () -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
        modifier = Modifier.fillMaxWidth()
    ) {
        content()
    }
}

/** Allow a single dot and digits only; cap length so a long paste doesn't break parsing. */
private fun sanitizeDecimal(input: String): String {
    val filtered = input.filter { it.isDigit() || it == '.' }
    val firstDot = filtered.indexOf('.')
    val collapsed = if (firstDot == -1) filtered
                    else filtered.substring(0, firstDot + 1) + filtered.substring(firstDot + 1).replace(".", "")
    return collapsed.take(6)
}
