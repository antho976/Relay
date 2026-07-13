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
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import java.util.Locale
import kotlin.math.floor
import kotlin.math.round

// Both tools work entirely in the user's display unit: the seed weight is converted in, the
// arithmetic (percentages, plate math) is unit-agnostic, and the labels read in kg or lb. Plate
// denominations and bar weights switch to the kg set when the user trains in kg.

/** "2.5" / "1.25" / "45" — a plate or bar weight with trailing zeros trimmed (no spurious ".0"). */
private fun trimWeight(v: Double): String =
    if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.2f", v).trimEnd('0').trimEnd('.')

// ─── Warmup Set Suggester (#10) ───────────────────────────────────────────────

/**
 * Given a working weight, suggests warmup sets at 40%, 60%, and 80%.
 */
@Composable
fun WarmupSuggesterDialog(
    workingWeightLb: Double?,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onDismiss: () -> Unit
) {
    // Warmup percentages step in plate-friendly increments, so this stays a kg-or-lb view (stones has
    // no plate denominations) — stones users see the lb figures they'd actually load.
    val metric = weightUnit.isMetric
    val unit = unitLabel(metric)
    var input by remember { mutableStateOf(workingWeightLb?.let { weightInputValue(it, metric) } ?: "") }
    val working = input.toDoubleOrNull()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Warmup Sets") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text("Working weight ($unit)") },
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
                                Text("${trimWeight(weight)} $unit × $reps",
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

private val STANDARD_PLATES_LB = listOf(45.0, 35.0, 25.0, 10.0, 5.0, 2.5)
private val STANDARD_PLATES_KG = listOf(25.0, 20.0, 15.0, 10.0, 5.0, 2.5, 1.25)

/**
 * Shows which plates to load on each side for a given total weight. Uses a 45 lb / 20 kg bar by
 * default (toggle to the lighter 35 lb / 15 kg bar), with the matching plate denominations.
 */
@Composable
fun PlateCalculatorDialog(
    initialWeightLb: Double? = null,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onDismiss: () -> Unit
) {
    // Plates are physically kg or lb — stones has no denomination, so a stones user calculates in the
    // lb figures they'd actually load. Metric = kg; everything else uses the lb bar + plates.
    val metric = weightUnit.isMetric
    val unit = unitLabel(metric)
    val plateSet = if (metric) STANDARD_PLATES_KG else STANDARD_PLATES_LB
    val heavyBar = if (metric) 20.0 else 45.0
    val lightBar = if (metric) 15.0 else 35.0
    var input by remember { mutableStateOf(initialWeightLb?.let { weightInputValue(it, metric) } ?: "") }
    var useHeavyBar by remember { mutableStateOf(true) }
    val bar = if (useHeavyBar) heavyBar else lightBar
    val target = input.toDoubleOrNull()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Plate Calculator") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text("Target weight ($unit)") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Bar: ${trimWeight(bar)} $unit", style = MaterialTheme.typography.bodySmall)
                    TextButton(onClick = { useHeavyBar = !useHeavyBar }) {
                        Text(if (useHeavyBar) "Switch to ${trimWeight(lightBar)} $unit bar"
                             else "Switch to ${trimWeight(heavyBar)} $unit bar")
                    }
                }
                if (target != null && target > bar) {
                    val perSide = (target - bar) / 2
                    val plates = calculatePlates(perSide, plateSet)
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(8.dp))
                            .padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text("EACH SIDE (${trimWeight(perSide)} $unit):",
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
                                    Text("${trimWeight(plate)} $unit", style = MaterialTheme.typography.bodyMedium)
                                    Text("× $count", style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.SemiBold,
                                        color = MaterialTheme.colorScheme.primary)
                                }
                            }
                            HorizontalDivider()
                            val actualTotal = bar + plates.sumOf { (p, c) -> p * c * 2 }
                            // Float accumulation can leave actualTotal a hair off an exact target;
                            // compare with a small epsilon so a correct load isn't painted as an error.
                            Text("Total: ${trimWeight(actualTotal)} $unit",
                                style = MaterialTheme.typography.bodySmall,
                                color = if (kotlin.math.abs(actualTotal - target) < 0.01) MaterialTheme.colorScheme.primary
                                        else MaterialTheme.colorScheme.error)
                        }
                    }
                } else if (target != null && target <= bar) {
                    Text("Bar only (${trimWeight(bar)} $unit)", style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

private fun calculatePlates(perSide: Double, plateSet: List<Double>): List<Pair<Double, Int>> {
    var remaining = perSide
    val result = mutableListOf<Pair<Double, Int>>()
    for (plate in plateSet) {
        val count = floor(remaining / plate).toInt()
        if (count > 0) {
            result.add(plate to count)
            remaining -= plate * count
        }
    }
    return if (remaining < 0.01) result else emptyList()
}
