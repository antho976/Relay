package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.theme.ForgeLastGreen
import kotlin.math.roundToInt

/** Bodyweight trend: current value, change since first logged, and a sparkline. */
@Composable
internal fun BodyweightCard(trend: List<Double>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("BODYWEIGHT · lb", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        if (trend.isEmpty()) {
            Text("No bodyweight logged yet.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        } else {
            val current = trend.last()
            val delta = if (trend.size >= 2) current - trend.first() else 0.0
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("${current.toInt()}", style = MaterialTheme.typography.displaySmall, color = onBg)
                Text("lb", style = MaterialTheme.typography.bodyMedium, color = muted, modifier = Modifier.padding(bottom = 6.dp))
                if (kotlin.math.abs(delta) >= 0.5) {
                    val sign = if (delta > 0) "+" else ""
                    Text("$sign${delta.toInt()}", style = MaterialTheme.typography.labelMedium,
                        color = muted, modifier = Modifier.padding(bottom = 6.dp))
                }
            }
            if (trend.size >= 2) {
                Spacer(Modifier.height(8.dp))
                Sparkline(
                    values = trend,
                    lineColor = accent,
                    minValue = trend.min(),
                    maxValue = trend.max(),
                    modifier = Modifier.fillMaxWidth().height(56.dp)
                )
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

// Generic ×bodyweight strength bands. Same scale across lifts — a simplification (real
// standards differ per lift), but honest for DB/machine work where no published tables exist.
private val TIER_CUTOFFS = listOf(0.4, 0.7, 1.1, 1.5) // entry ratio for Novice / Inter / Adv / Elite
private val TIER_SHORT = listOf("UNTR", "NOVI", "INTE", "ADVA", "ELIT")
private val TIER_FULL = listOf("Untrained", "Novice", "Intermediate", "Advanced", "Elite")

private fun tierIndex(ratio: Double): Int {
    TIER_CUTOFFS.forEachIndexed { i, cut -> if (ratio < cut) return i }
    return TIER_CUTOFFS.size
}

/** "Where you stand" — each lift's e1RM ÷ bodyweight mapped onto an Untrained→Elite scale. */
@Composable
internal fun StrengthStandardsCard(lifts: List<E1rmLift>, bodyweightLb: Double?, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val grey = muted.copy(alpha = 0.25f)
    val rated = if (bodyweightLb != null && bodyweightLb > 0)
        lifts.filter { it.currentE1rm > 0 }.take(5).map { it to it.currentE1rm / bodyweightLb } else emptyList()
    val avgIdx = if (rated.isNotEmpty()) rated.map { tierIndex(it.second) }.average().roundToInt().coerceIn(0, 4) else 0

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
        if (rated.isEmpty()) {
            Text("Log your bodyweight to see where you stand.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        } else {
            rated.forEach { (lift, ratio) ->
                val idx = tierIndex(ratio)
                Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(lift.exerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text("%.2f× BW".format(ratio), style = MaterialTheme.typography.labelMedium, color = onBg, fontWeight = FontWeight.SemiBold)
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
            val lowest = rated.minByOrNull { tierIndex(it.second) }
            if (lowest != null) {
                val idx = tierIndex(lowest.second)
                if (idx < TIER_CUTOFFS.size) {
                    Spacer(Modifier.height(16.dp))
                    Text("What's next", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Push ${lowest.first.exerciseName} from %.2f× to %.2f× BW to reach ${TIER_FULL[idx + 1]}.".format(lowest.second, TIER_CUTOFFS[idx]),
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

/** Top lifts expressed as a multiple of bodyweight (needs a logged bodyweight). */
@Composable
internal fun RelativeStrengthCard(records: List<PrRecord>, onBg: Color, muted: Color, outline: Color) {
    val rel = records.filter { it.relativeStrength != null }.sortedByDescending { it.relativeStrength }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("RELATIVE STRENGTH · × bodyweight", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        if (rel.isEmpty()) {
            Text("Log your bodyweight to see lifts as a multiple of it.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        } else {
            rel.take(8).forEach { r ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(r.exerciseName, style = MaterialTheme.typography.bodySmall, color = onBg, modifier = Modifier.weight(1f))
                    Text("%.2f× BW".format(r.relativeStrength ?: 0.0), style = MaterialTheme.typography.labelMedium, color = ForgeLastGreen, fontWeight = FontWeight.SemiBold)
                }
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}
