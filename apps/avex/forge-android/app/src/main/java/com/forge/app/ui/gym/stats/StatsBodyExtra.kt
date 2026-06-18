package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.gym.stats.components.CountUpText
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.state.BodyweightPoint
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.parseSaneBodyweightLb
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/** Bodyweight trend on a real time axis: current value counting up, dated sparkline, delta.
 *  Tapping anywhere on the card opens the quick-log sheet ([onLogWeight]). */
@Composable
internal fun BodyweightCard(points: List<BodyweightPoint>, onBg: Color, muted: Color, accent: Color, outline: Color, onLogWeight: () -> Unit = {}) {
    val useKg = LocalForgeSettings.current.useKg
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(onClickLabel = "Log bodyweight") { onLogWeight() }
            .padding(horizontal = 24.dp)
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("BODYWEIGHT · ${unitLabel(useKg)}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Text("＋ LOG", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp, letterSpacing = 1.sp)
        }
        Spacer(Modifier.height(8.dp))
        if (points.isEmpty()) {
            Text(
                "No bodyweight on file. Tap to log it — relative strength unlocks with it.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
            )
        } else {
            val current = points.last().weightLb
            val prev = points.getOrNull(points.size - 2)?.weightLb
            val delta = if (points.size >= 2) current - points.first().weightLb else 0.0
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                CountUpText(
                    value = toDisplayWeight(current, useKg),
                    fromValue = prev?.let { toDisplayWeight(it, useKg) },
                    style = MaterialTheme.typography.displaySmall,
                    color = onBg
                )
                Text(unitLabel(useKg), style = MaterialTheme.typography.bodyMedium, color = muted, modifier = Modifier.padding(bottom = 6.dp))
                val magDisplay = kotlin.math.abs(toDisplayWeight(delta, useKg))
                if (magDisplay >= 0.5) {
                    // Show one decimal for sub-1-unit changes instead of truncating to "0".
                    val mag = magDisplay
                    val magStr = if (mag % 1.0 == 0.0) "${mag.toInt()}" else String.format(java.util.Locale.US, "%.1f", mag)
                    val sign = if (delta > 0) "+" else "-"
                    Text("$sign$magStr since first log", style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 10.sp, modifier = Modifier.padding(bottom = 6.dp))
                }
            }
            if (points.size >= 2) {
                val values = points.map { it.weightLb }
                Spacer(Modifier.height(8.dp))
                Sparkline(
                    values = values,
                    lineColor = accent,
                    minValue = values.min(),
                    maxValue = values.max(),
                    modifier = Modifier.fillMaxWidth().height(56.dp),
                    progress = rememberDrawProgress(points.size)
                )
                Spacer(Modifier.height(4.dp))
                val fmt = SimpleDateFormat("MMM d", Locale.getDefault())
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(fmt.format(Date(points.first().recordedAt)).uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                    Text(fmt.format(Date(points.last().recordedAt)).uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

// Generic ×bodyweight strength bands. Same scale across lifts — a simplification (real
// standards differ per lift), but honest for DB/machine work where no published tables exist.
// Sex-aware: women's bodyweight-relative standards run lower than men's (~0.7×), so an
// unspecified/male user gets the original bands and a female user gets the scaled-down set.
private val TIER_CUTOFFS_MALE = listOf(0.4, 0.7, 1.1, 1.5) // entry ratio for Novice / Inter / Adv / Elite
private val TIER_CUTOFFS_FEMALE = listOf(0.3, 0.5, 0.8, 1.1)
private val TIER_SHORT = listOf("UNTR", "NOVI", "INTE", "ADVA", "ELIT")
private val TIER_FULL = listOf("Untrained", "Novice", "Intermediate", "Advanced", "Elite")
/** A lift needs at least this many sessions behind its e1RM before its tier is shown (vs "calibrating"). */
private const val MIN_SESSIONS_FOR_TIER = 3

private fun tierCutoffs(sex: String): List<Double> =
    if (sex == "female") TIER_CUTOFFS_FEMALE else TIER_CUTOFFS_MALE

private fun tierIndex(ratio: Double, sex: String): Int {
    val cutoffs = tierCutoffs(sex)
    cutoffs.forEachIndexed { i, cut -> if (ratio < cut) return i }
    return cutoffs.size
}

/** "Where you stand" — each lift's e1RM ÷ bodyweight mapped onto an Untrained→Elite scale. */
@Composable
internal fun StrengthStandardsCard(lifts: List<E1rmLift>, bodyweightLb: Double?, sex: String, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val grey = muted.copy(alpha = 0.25f)
    val useKg = LocalForgeSettings.current.useKg
    val bw = bodyweightLb ?: 0.0
    val hasBodyweight = bw > 0
    // A tier only locks in once a lift has a few sessions behind its e1RM — one fluke set shouldn't
    // read as "Advanced". With bodyweight but too little history per lift, we show a calibrating state.
    val eligible = if (hasBodyweight) lifts.filter { it.currentE1rm > 0 } else emptyList()
    val rated = eligible.filter { it.history.size >= MIN_SESSIONS_FOR_TIER }.take(5).map { it to it.currentE1rm / bw }
    val avgIdx = if (rated.isNotEmpty()) rated.map { tierIndex(it.second, sex) }.average().roundToInt().coerceIn(0, 4) else 0

    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("Where you stand", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
            if (rated.isNotEmpty()) {
                Box(Modifier.border(0.5.dp, ForgeLastGreen.copy(alpha = 0.6f), RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 3.dp)) {
                    Text(TIER_FULL[avgIdx].uppercase(), style = MaterialTheme.typography.labelSmall, color = ForgeLastGreen, fontSize = 8.sp, letterSpacing = 0.5.sp)
                }
            }
        }
        Spacer(Modifier.height(4.dp))
        Text("Lifts as a multiple of bodyweight, on the strength scale.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        if (!hasBodyweight) {
            Text("Log your bodyweight to see where you stand.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        } else if (rated.isEmpty()) {
            Text("Still calibrating — a few more sessions of your main lifts and your tier locks in.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        } else {
            rated.forEach { (lift, ratio) ->
                val idx = tierIndex(ratio, sex)
                // Tier is otherwise conveyed ONLY by which bar segment is green — useless to a
                // colour-blind user (~8% of men) and to TalkBack. Name it in text + a merged spoken node.
                Column(
                    Modifier.fillMaxWidth().padding(vertical = 8.dp)
                        .semantics(mergeDescendants = true) {
                            contentDescription = "${lift.exerciseName}, %.2f times bodyweight, ${TIER_FULL[idx]} tier".format(ratio)
                        }
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(lift.exerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text("%.2f× BW · ${TIER_FULL[idx]}".format(ratio), style = MaterialTheme.typography.labelMedium, color = onBg, fontWeight = FontWeight.SemiBold)
                    }
                    Spacer(Modifier.height(6.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        for (i in 0..4) {
                            Box(Modifier.weight(1f).height(6.dp).background(if (i == idx) ForgeLastGreen else grey, RoundedCornerShape(3.dp)))
                        }
                    }
                    Spacer(Modifier.height(4.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        TIER_SHORT.forEachIndexed { i, t ->
                            Text(t, style = MaterialTheme.typography.labelSmall, color = if (i == idx) ForgeLastGreen else muted,
                                fontSize = 8.sp, textAlign = TextAlign.Center, modifier = Modifier.weight(1f))
                        }
                    }
                }
            }
            // What's next — push the weakest lift toward its next tier
            val lowest = rated.minByOrNull { tierIndex(it.second, sex) }
            if (lowest != null) {
                val idx = tierIndex(lowest.second, sex)
                if (idx < tierCutoffs(sex).size) {
                    Spacer(Modifier.height(16.dp))
                    Text("What's next", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
                    Spacer(Modifier.height(8.dp))
                    // The next-tier cutoff is bodyweight-relative; multiply by the user's bodyweight
                    // so they also see the concrete e1RM target in their unit, not just a ratio.
                    val targetLb = tierCutoffs(sex)[idx] * (bodyweightLb ?: 0.0)
                    Text(
                        "Push ${lowest.first.exerciseName} from %.2f× to %.2f× BW (≈ ${formatWeight(targetLb, useKg)}) to reach ${TIER_FULL[idx + 1]}.".format(lowest.second, tierCutoffs(sex)[idx]),
                        style = MaterialTheme.typography.bodySmall, color = onBg
                    )
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/**
 * Quick-log sheet for the Body tab — the only place to record bodyweight after onboarding. Type
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
    // Keyed on latestLb so the prefill lands even if the stats flow emits the latest weight just
    // AFTER the sheet opens (otherwise the field would be stuck blank for that open).
    var input by remember(latestLb) { mutableStateOf(latestLb?.let { weightInputValue(it, useKg) } ?: "") }
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
                    Text(if (invalid) "Enter $minDisp–$maxDisp ${unitLabel(useKg)}." else "One entry per day — saving replaces today's.")
                },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.fillMaxWidth()
            )
            Button(onClick = { parsed?.let(onSave) }, enabled = parsed != null, modifier = Modifier.fillMaxWidth()) {
                Text("Save")
            }
            if (canImport) {
                TextButton(onClick = onImport, modifier = Modifier.fillMaxWidth()) {
                    Text("Import latest from Health Connect")
                }
            }
            message?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
        }
    }
}
