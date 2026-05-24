package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlin.math.floor
import kotlin.math.round

// ─── Warmup Set Suggester (#10) ───────────────────────────────────────────────

/**
 * Given a working weight, suggests warmup sets at 40%, 60%, and 80%.
 */
@Composable
fun WarmupSuggesterDialog(
    workingWeightLb: Double?,
    onDismiss: () -> Unit
) {
    var input by remember { mutableStateOf(workingWeightLb?.toInt()?.toString() ?: "") }
    val working = input.toDoubleOrNull()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Warmup Sets") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text("Working weight (lb)") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true
                )
                if (working != null && working > 0) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(8.dp))
                            .padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text("SUGGESTED WARMUP", style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
                        listOf(
                            "Set 1" to 0.4,
                            "Set 2" to 0.6,
                            "Set 3" to 0.8
                        ).forEach { (label, pct) ->
                            val weight = roundToNearest(working * pct, 2.5)
                            val reps = when {
                                pct <= 0.4 -> 12
                                pct <= 0.6 -> 8
                                else -> 5
                            }
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(label, style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text("${weight.toInt()} lb × $reps",
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = FontWeight.SemiBold)
                                Text("${(pct * 100).toInt()}%",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

private fun roundToNearest(value: Double, increment: Double): Double =
    round(value / increment) * increment

// ─── Plate Calculator (#11) ───────────────────────────────────────────────────

private val STANDARD_PLATES = listOf(45.0, 35.0, 25.0, 10.0, 5.0, 2.5)
private const val BAR_WEIGHT = 45.0

/**
 * Shows which plates to load on each side for a given total weight.
 * Assumes a 45 lb bar (standard). User can toggle to a lighter bar.
 */
@Composable
fun PlateCalculatorDialog(
    initialWeightLb: Double? = null,
    onDismiss: () -> Unit
) {
    var input by remember { mutableStateOf(initialWeightLb?.toInt()?.toString() ?: "") }
    var useHeavyBar by remember { mutableStateOf(true) }
    val barLb = if (useHeavyBar) BAR_WEIGHT else 35.0
    val targetLb = input.toDoubleOrNull()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Plate Calculator") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text("Target weight (lb)") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Bar: ${barLb.toInt()} lb", style = MaterialTheme.typography.bodySmall)
                    TextButton(onClick = { useHeavyBar = !useHeavyBar }) {
                        Text(if (useHeavyBar) "Switch to 35 lb bar" else "Switch to 45 lb bar")
                    }
                }
                if (targetLb != null && targetLb > barLb) {
                    val perSide = (targetLb - barLb) / 2
                    val plates = calculatePlates(perSide)
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(8.dp))
                            .padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text("EACH SIDE (${perSide} lb):",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
                        if (plates.isEmpty()) {
                            Text("No standard plate combination found.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.error)
                        } else {
                            plates.forEach { (plate, count) ->
                                Row(modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween) {
                                    Text("${plate.toInt()} lb", style = MaterialTheme.typography.bodyMedium)
                                    Text("× $count", style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.SemiBold,
                                        color = MaterialTheme.colorScheme.primary)
                                }
                            }
                            HorizontalDivider()
                            val actualTotal = barLb + plates.sumOf { (p, c) -> p * c * 2 }
                            Text("Total: ${actualTotal.toInt()} lb",
                                style = MaterialTheme.typography.bodySmall,
                                color = if (actualTotal == targetLb) MaterialTheme.colorScheme.primary
                                        else MaterialTheme.colorScheme.error)
                        }
                    }
                } else if (targetLb != null && targetLb <= barLb) {
                    Text("Bar only (${barLb.toInt()} lb)", style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

private fun calculatePlates(perSide: Double): List<Pair<Double, Int>> {
    var remaining = perSide
    val result = mutableListOf<Pair<Double, Int>>()
    for (plate in STANDARD_PLATES) {
        val count = floor(remaining / plate).toInt()
        if (count > 0) {
            result.add(plate to count)
            remaining -= plate * count
        }
    }
    return if (remaining < 0.01) result else emptyList()
}
