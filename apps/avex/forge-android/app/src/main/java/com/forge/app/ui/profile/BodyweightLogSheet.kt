package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.parseSaneBodyweightLb
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.roundToInt

/**
 * Quick-log sheet for bodyweight — lives on the Profile's BODYWEIGHT section (moved from the old
 * Stats Body tab 2026-07-01), and is the only place to record bodyweight after onboarding. Type
 * today's value (validated against the same sane range as onboarding) or, when Health Connect is
 * connected, pull the latest reading with one tap. Saving closes the sheet; importing keeps it open
 * so the result line is visible.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BodyweightLogSheet(
    latestLb: Double?,
    canImport: Boolean,
    message: String?,
    onSave: (Double) -> Unit,
    onImport: () -> Unit,
    onDismiss: () -> Unit
) {
    val useKg = LocalForgeSettings.current.useKg
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val sheetState = rememberModalBottomSheetState()
    // Keyed on latestLb so the prefill lands even if the flow emits the latest weight just AFTER the
    // sheet opens; also on useKg so flipping the unit while the sheet is open re-seeds the field in
    // the new unit instead of leaving a stale value that fails validation and locks Save.
    var input by remember(latestLb, useKg) { mutableStateOf(latestLb?.let { weightInputValue(it, useKg) } ?: "") }
    val parsed = parseSaneBodyweightLb(input, useKg)
    val invalid = input.isNotBlank() && parsed == null
    val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, useKg).roundToInt()
    val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, useKg).roundToInt()

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text("Log bodyweight", style = MaterialTheme.typography.headlineSmall, color = onBg)
            OutlinedTextField(
                value = input,
                onValueChange = { v ->
                    // Digits + at most ONE decimal point: collapse any extra dots so '7.5.2' can't slip
                    // through and surface as a misleading out-of-range error.
                    val f = v.filter { ch -> ch.isDigit() || ch == '.' }
                    val dot = f.indexOf('.')
                    input = if (dot < 0) f else f.substring(0, dot + 1) + f.substring(dot + 1).replace(".", "")
                },
                label = { Text("Weight (${unitLabel(useKg)})") },
                singleLine = true,
                isError = invalid,
                supportingText = {
                    Text(if (invalid) "Enter $minDisp–$maxDisp ${unitLabel(useKg)}." else "One entry per day, saving replaces today's.")
                },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.fillMaxWidth()
            )
            ForgePrimaryCapsule(
                label = "Save",
                onClick = { parsed?.let(onSave) },
                enabled = parsed != null,
                modifier = Modifier.fillMaxWidth()
            )
            if (canImport) {
                ForgeOutlineCapsule(
                    label = "Import latest from Health Connect",
                    onClick = onImport,
                    modifier = Modifier.fillMaxWidth()
                )
            }
            message?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
        }
    }
}
