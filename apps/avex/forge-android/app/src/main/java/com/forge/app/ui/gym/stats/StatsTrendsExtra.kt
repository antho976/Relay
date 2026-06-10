package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.gym.stats.state.InsightFlag
import com.forge.app.ui.gym.stats.state.TrainingTimes
import com.forge.app.ui.gym.stats.state.WeeklyDuration
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts

private val DOW = listOf("M", "T", "W", "T", "F", "S", "S")

/** "When you train" — sessions per weekday with the best hour as the caption. */
@Composable
internal fun WhenYouTrainCard(times: TrainingTimes, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val counts = times.sessionsByDayOfWeek
    if (counts.all { it == 0 }) return
    val max = counts.max().coerceAtLeast(1)
    val progress = rememberDrawProgress(counts.sum())
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("When you train", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text(
            times.bestHourLabel?.let { "Most sets land in the $it." } ?: "Sessions by day of week.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth().height(72.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Bottom) {
            counts.forEachIndexed { i, count ->
                val tile = staggeredProgress(progress, i, 7)
                val frac = (count.toFloat() / max) * tile
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("$count", style = MaterialTheme.typography.labelSmall, color = if (count == max) onBg else muted, fontSize = 8.sp)
                    Box(
                        Modifier.fillMaxWidth().height((3 + 48 * frac).dp)
                            .background(
                                if (count == max) accent else onBg.copy(alpha = 0.6f),
                                RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp)
                            )
                    )
                    Text(DOW[i], style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/**
 * "How hard the weeks felt" — the revived effort distribution (#75): stacked weekly bars
 * of easy / just-right / hard / brutal exercise ratings.
 */
@Composable
internal fun EffortDistributionCard(weeks: List<WeeklyEffortCounts>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val rated = weeks.filter { it.total > 0 }
    if (rated.size < 2) return
    val maxTotal = rated.maxOf { it.total }.coerceAtLeast(1)
    val error = MaterialTheme.colorScheme.error
    val progress = rememberDrawProgress(rated.sumOf { it.total })
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("How hard the weeks felt", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("Effort ratings per week — a rising dark band is fatigue talking.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth().height(96.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Bottom) {
            rated.forEachIndexed { i, w ->
                val tile = staggeredProgress(progress, i, rated.size)
                val unit = 88f / maxTotal * tile
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.Bottom) {
                    // Stacked top-down: brutal darkest at the top of the bar.
                    if (w.brutal > 0) Box(Modifier.fillMaxWidth().height((unit * w.brutal).dp).background(error.copy(alpha = 0.85f)))
                    if (w.hard > 0) Box(Modifier.fillMaxWidth().height((unit * w.hard).dp).background(accent))
                    if (w.justRight > 0) Box(Modifier.fillMaxWidth().height((unit * w.justRight).dp).background(onBg.copy(alpha = 0.55f)))
                    if (w.easy > 0) Box(Modifier.fillMaxWidth().height((unit * w.easy).dp).background(muted.copy(alpha = 0.3f)))
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(rated.first().weekLabel, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text(rated.last().weekLabel, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        }
        Spacer(Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            EffortLegend("EASY", muted.copy(alpha = 0.3f), muted)
            EffortLegend("RIGHT", onBg.copy(alpha = 0.55f), muted)
            EffortLegend("HARD", accent, muted)
            EffortLegend("BRUTAL", error.copy(alpha = 0.85f), muted)
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
private fun EffortLegend(label: String, dot: Color, muted: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.width(8.dp).height(8.dp).background(dot, RoundedCornerShape(2.dp)))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}

/**
 * "How long it takes" — median session length per week, with the engine's
 * estimate-calibration insight as the caption when it has something to say.
 */
@Composable
internal fun DurationTrendCard(weeks: List<WeeklyDuration>, calibration: InsightFlag?, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (weeks.size < 2) return
    val values = weeks.map { it.medianMin.toDouble() }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("How long it takes", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text(
            "Median session length per week — holding around ${weeks.last().medianMin} min.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(12.dp))
        Sparkline(
            values = values, lineColor = onBg,
            minValue = values.min(), maxValue = values.max(),
            modifier = Modifier.fillMaxWidth().height(64.dp),
            progress = rememberDrawProgress(weeks.size)
        )
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(weeks.first().weekLabel, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text(weeks.last().weekLabel, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        }
        calibration?.let {
            Spacer(Modifier.height(10.dp))
            Text("${it.emoji} ${it.body}", style = MaterialTheme.typography.bodySmall, color = accent, fontStyle = FontStyle.Italic)
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}
