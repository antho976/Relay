package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.theme.LocalForgeSettings

private val SET_COL_W = 44.dp
private val REPS_COL_W = 56.dp
private val DELTA_COL_W = 88.dp

/**
 * Input row for the next set. When [nextSetNumber] is provided the layout
 * matches the set-table columns and the LOG SET button is rendered full-width below.
 */
@Composable
fun SetInputRow(
    prefillWeight: String?,
    suggestedWeight: String? = null,
    suggestionReason: String? = null,
    priorSets: List<LoggedSet> = emptyList(),
    nextSetNumber: Int? = null,
    priorSetForActiveRow: LoggedSet? = null,
    onSubmit: (weightText: String, reps: Int) -> Unit,
    modifier: Modifier = Modifier
) {
    var weight by rememberSaveable(prefillWeight) { mutableStateOf(prefillWeight.orEmpty()) }
    var reps by rememberSaveable { mutableStateOf("") }
    val useKg = LocalForgeSettings.current.useKg

    fun onWeightChange(new: String) {
        val match = Regex("""^([0-9]*\.?[0-9]+)\s*[xX]\s*([0-9]+)$""").matchEntire(new.trim())
        if (match != null) { weight = match.groupValues[1]; reps = match.groupValues[2] }
        else weight = new
    }

    val canSubmit = remember(weight, reps) {
        weight.isNotBlank() && reps.toIntOrNull()?.let { it > 0 } == true
    }

    val prRepsHint = remember(weight, priorSets) {
        val weightLb = weight.trim().toDoubleOrNull() ?: return@remember null
        repsNeededForPr(priorSets, weightLb)
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val bg = MaterialTheme.colorScheme.background

    if (nextSetNumber != null) {
        // ── Ledger-style table input row ─────────────────────────────────────
        Column(modifier = modifier.fillMaxWidth()) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                // Set number
                Box(modifier = Modifier.width(SET_COL_W).padding(bottom = 4.dp)) {
                    Text(
                        "%02d".format(nextSetNumber),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                }

                // Weight input
                Column(modifier = Modifier.weight(1f)) {
                    Text("WEIGHT${if (useKg) " · KG" else " · LB"}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    Spacer(Modifier.height(2.dp))
                    UnderlineNumberField(
                        value = weight,
                        onValueChange = ::onWeightChange,
                        placeholder = "0",
                        keyboardType = KeyboardType.Text,
                        imeAction = ImeAction.Next,
                        supportingText = prRepsHint?.let { "$it for PR" }
                    )
                }

                // Reps input
                Box(modifier = Modifier.width(REPS_COL_W + 8.dp).padding(start = 8.dp)) {
                    Column {
                        Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                        Spacer(Modifier.height(2.dp))
                        UnderlineNumberField(
                            value = reps,
                            onValueChange = { new -> if (new.all { it.isDigit() }) reps = new },
                            placeholder = "—",
                            keyboardType = KeyboardType.Number,
                            imeAction = ImeAction.Done
                        )
                    }
                }

                // Prior set hint ("try 45 × 10")
                Box(modifier = Modifier.width(DELTA_COL_W), contentAlignment = Alignment.BottomEnd) {
                    priorSetForActiveRow?.let { prior ->
                        val priorDisplay = prior.weightLb?.let { formatWeight(it, useKg) } ?: prior.weightText
                        Text(
                            "try $priorDisplay × ${prior.reps}",
                            style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.35f),
                            fontSize = 9.sp,
                            textAlign = TextAlign.End
                        )
                    }
                }
            }

            Spacer(Modifier.height(12.dp))

            // Full-width primary CTA
            Button(
                onClick = {
                    val r = reps.toIntOrNull() ?: return@Button
                    onSubmit(weight.trim(), r)
                    reps = ""
                },
                enabled = canSubmit,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(50),
                contentPadding = PaddingValues(vertical = 14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = onBg,
                    contentColor = bg,
                    disabledContainerColor = onBg.copy(alpha = 0.25f),
                    disabledContentColor = bg.copy(alpha = 0.5f)
                )
            ) {
                Text(
                    "LOG SET $nextSetNumber →",
                    style = MaterialTheme.typography.labelLarge
                )
            }
        }
    } else {
        // ── Legacy compact layout (kept for any other call sites) ────────────
        Column(modifier = modifier.fillMaxWidth()) {
            if (suggestedWeight != null) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = 2.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    val hint = buildString {
                        append("Try: $suggestedWeight lb")
                        if (!suggestionReason.isNullOrBlank()) append(" · $suggestionReason")
                    }
                    Text(hint, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary.copy(alpha = 0.75f), modifier = Modifier.weight(1f))
                }
            }
            Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("WEIGHT · LB", style = MaterialTheme.typography.labelSmall, color = muted)
                    UnderlineNumberField(weight, ::onWeightChange, "0", KeyboardType.Text, ImeAction.Next, supportingText = prRepsHint?.let { "$it for PR" })
                }
                Column(modifier = Modifier.weight(0.7f).padding(start = 16.dp)) {
                    Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted)
                    UnderlineNumberField(reps, { new -> if (new.all { it.isDigit() }) reps = new }, "0", KeyboardType.Number, ImeAction.Done)
                }
                Button(
                    onClick = {
                        val r = reps.toIntOrNull() ?: return@Button
                        onSubmit(weight.trim(), r)
                        reps = ""
                    },
                    enabled = canSubmit,
                    shape = RoundedCornerShape(50),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = onBg,
                        contentColor = bg,
                        disabledContainerColor = onBg.copy(alpha = 0.35f),
                        disabledContentColor = bg.copy(alpha = 0.7f)
                    )
                ) { Text("Log set →", style = MaterialTheme.typography.labelSmall) }
            }
            if (prefillWeight != null && weight.isBlank()) {
                Text(
                    "Use last: $prefillWeight",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(top = 2.dp)
                )
            }
        }
    }
}

@Composable
private fun UnderlineNumberField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    imeAction: ImeAction,
    modifier: Modifier = Modifier,
    supportingText: String? = null
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Column(modifier = modifier) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = MaterialTheme.typography.headlineMedium.copy(color = onBg),
            singleLine = true,
            cursorBrush = SolidColor(accent),
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.headlineMedium, color = muted.copy(alpha = 0.4f))
                    }
                    inner()
                }
            }
        )
        HorizontalDivider(modifier = Modifier.padding(top = 2.dp), thickness = 1.dp, color = outline.copy(alpha = 0.5f))
        if (supportingText != null) {
            Text(supportingText, style = MaterialTheme.typography.labelSmall, color = accent, modifier = Modifier.padding(top = 2.dp))
        }
    }
}

private fun repsNeededForPr(history: List<LoggedSet>, weightLb: Double): Int? {
    val maxRepsAtOrAbove = history
        .filter { it.weightLb != null && it.weightLb >= weightLb }
        .maxOfOrNull { it.reps }
    return maxRepsAtOrAbove?.let { it + 1 }
}
