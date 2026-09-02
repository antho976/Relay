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
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.domain.warmup.WarmupEngine
import com.forge.app.domain.warmup.WarmupExercise
import com.forge.app.domain.warmup.WarmupRampSet
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.theme.LocalForgeSettings
import java.util.Locale
import kotlin.math.floor

// Both tools work entirely in the user's display unit: the seed weight is converted in, the
// arithmetic (percentages, plate math) is unit-agnostic, and the labels read in kg or lb. Plate
// denominations and bar weights switch to the kg set when the user trains in kg.

/** "2.5" / "1.25" / "45" — a plate or bar weight with trailing zeros trimmed (no spurious ".0"). */
private fun trimWeight(v: Double): String =
    if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.2f", v).trimEnd('0').trimEnd('.')

// ─── Warmup Set Suggester (#10) ───────────────────────────────────────────────

/**
 * The suggester's seed text for a stored working weight: a plate count on a plate machine (stored
 * pounds ÷ the configured plate weight, matching [formatPlateCount] in the set row), the
 * display-unit weight otherwise. A non-positive plate weight cannot be divided by, so it falls
 * back to the pounds rather than to infinity.
 */
internal fun warmupSeedText(
    workingWeightLb: Double,
    isPlates: Boolean,
    plateLb: Double,
    weightUnit: com.forge.app.domain.units.WeightUnit
): String =
    if (isPlates) formatPlateCount(if (plateLb > 0.0) workingWeightLb / plateLb else workingWeightLb)
    else weightInputValue(workingWeightLb, weightUnit)

/**
 * The ramp for one lift, on demand from its own card.
 *
 * Shares [WarmupEngine] with the pre-session warmup, so the ladder here and the one in the gate are
 * the same ladder. It replaced a fixed 40/60/80%, which was too much warmup for a set of fifteen and
 * too little for a heavy triple, and which never knew the muscle had already been worked.
 *
 * The field stays in the user's display unit; the engine is handed the exercise's own storage scale
 * (a plate count on PLATES, pounds otherwise) and its results come back out the same way.
 */
@Composable
fun WarmupSuggesterDialog(
    exerciseName: String,
    unit: ExerciseUnit,
    isCompound: Boolean,
    targetReps: Int,
    muscleAlreadyWarm: Boolean,
    workingWeightLb: Double?,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onDismiss: () -> Unit
) {
    val isPlates = unit == ExerciseUnit.PLATES
    val plateLb = LocalForgeSettings.current.plateWeightLb
    // The field holds a plate COUNT on a plate machine, but the working weight arrives in stored
    // pounds like every other load, so it is divided by the configured plate weight here — the same
    // way the set input row seeds itself. Handing the pounds straight through read a 60 lb
    // (four-plate) set as sixty plates and built the ramp from that. Everything else is shown and
    // entered in kg, lb or stones.
    val seed = workingWeightLb?.let { warmupSeedText(it, isPlates, plateLb, weightUnit) }
    var input by remember { mutableStateOf(seed ?: "") }
    val typed = input.toDoubleOrNull()
    val workingStored = typed?.let { if (isPlates) it else fromDisplayWeight(it, weightUnit) }

    val ramp = remember(workingStored, unit, isCompound, targetReps, muscleAlreadyWarm, weightUnit) {
        if (workingStored == null || workingStored <= 0.0) emptyList()
        else WarmupEngine.rampFor(
            WarmupExercise(
                id = "suggester",
                name = exerciseName,
                muscle = MuscleGroup.CHEST, // unused by rampFor; the ladder depends on load and reps
                unit = unit,
                isCompound = isCompound,
                workingLoad = workingStored,
                targetReps = targetReps,
                loadStep = WarmupEngine.loadIncrement(unit, weightUnit.isMetric)
            ),
            alreadyWarm = muscleAlreadyWarm
        )
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = MaterialTheme.colorScheme.surface,
        title = { Text("Warm-up sets") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text(if (isPlates) "Working plates" else "Working weight (${unitLabel(weightUnit)})") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true
                )
                when {
                    workingStored == null || workingStored <= 0.0 -> Unit
                    // A real answer, not an empty panel: warm muscle plus light work needs no ramp,
                    // and saying so is the useful result.
                    ramp.isEmpty() -> Text(
                        if (muscleAlreadyWarm) "Already warm from earlier work. Go straight to your working set."
                        else "Light enough to start on your working set.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    else -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        ramp.forEachIndexed { index, set ->
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    "SET ${index + 1}",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    suggesterLoadLabel(set, isPlates, weightUnit),
                                    style = MaterialTheme.typography.titleMedium,
                                    color = MaterialTheme.colorScheme.onBackground
                                )
                                Text(
                                    "${set.percentOfWorking}% · ${set.restSeconds}S",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

/** One ramp row's load and reps, in the unit the user enters that exercise in. */
private fun suggesterLoadLabel(
    set: WarmupRampSet,
    isPlates: Boolean,
    weightUnit: com.forge.app.domain.units.WeightUnit
): String {
    val load = set.load ?: return "Bodyweight × ${set.reps}"
    val weight = if (isPlates) {
        val plates = load.toInt()
        if (plates == 1) "1 plate" else "$plates plates"
    } else {
        com.forge.app.domain.units.formatWeight(load, weightUnit)
    }
    return "$weight × ${set.reps}"
}


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
