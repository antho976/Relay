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
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.theme.ForgeLastGreen

/**
 * RPE distribution histogram + average. Shows how hard your sets actually land — a
 * cluster at 9–10 every session is a fatigue/over-reaching signal; mostly 6–7 means
 * there's room to push.
 */
@Composable
internal fun RpeCard(buckets: List<RpeBucket>, avg: Double?, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (buckets.isEmpty()) return
    val maxCount = buckets.maxOf { it.count }.coerceAtLeast(1)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("RPE DISTRIBUTION", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            avg?.let {
                Text("avg ${"%.1f".format(it)}", style = MaterialTheme.typography.labelMedium, color = accent)
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(
            modifier = Modifier.fillMaxWidth().height(72.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            buckets.forEach { b ->
                Column(
                    modifier = Modifier.weight(1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Text("${b.count}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                    val frac = b.count.toFloat() / maxCount
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .height((6 + 44 * frac).dp)
                            .background(accent.copy(alpha = 0.85f), RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                    )
                    Text(rpeLabel(b.rpe), style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp)
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

private fun rpeLabel(rpe: Double): String = if (rpe % 1.0 == 0.0) "${rpe.toInt()}" else "%.1f".format(rpe)

/** "Are you pushing hard enough?" — average RPE per session over time, vs the 7–9 sweet spot. */
@Composable
internal fun RpeTrendCard(avgPerSession: List<Double>, avg: Double?, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (avgPerSession.size < 2) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Are you pushing hard enough?", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("Average session RPE over time.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(12.dp))
        Column(Modifier.fillMaxWidth().border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(12.dp)).padding(16.dp)) {
            Text("AVG RPE / SESSION", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Spacer(Modifier.height(12.dp))
            Sparkline(
                values = avgPerSession, lineColor = onBg,
                minValue = 5.0, maxValue = 10.0,
                modifier = Modifier.fillMaxWidth().height(90.dp)
            )
            Spacer(Modifier.height(10.dp))
            val tag = avg?.let { "SWEET SPOT 7–9 · holding around ${"%.1f".format(it)} avg" } ?: "SWEET SPOT 7–9"
            Text(tag, style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/** "How you train" — last 12 weeks as stacked session blocks (green = session, grey = gap). */
@Composable
internal fun ConsistencyHeatmapCard(weeklyCounts: List<Int>, target: Int, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (weeklyCounts.isEmpty()) return
    val grey = muted.copy(alpha = 0.25f)
    val maxBlocks = (weeklyCounts.maxOrNull() ?: 1).coerceIn(3, 5)
    val avgPerWk = if (weeklyCounts.isNotEmpty()) weeklyCounts.average() else 0.0
    val firstHalf = weeklyCounts.take(weeklyCounts.size / 2).ifEmpty { listOf(0) }.average()
    val secondHalf = weeklyCounts.takeLast(weeklyCounts.size / 2).ifEmpty { listOf(0) }.average()
    val progressing = secondHalf >= firstHalf
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("How you train", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("CONSISTENCY · ${weeklyCounts.size} WEEKS · ${"%.1f".format(avgPerWk)} sess/wk", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(12.dp))
        Row(Modifier.fillMaxWidth().height(64.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom) {
            weeklyCounts.forEach { count ->
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    for (i in maxBlocks downTo 1) {
                        val filled = i <= count
                        val color = if (filled) (if (count >= target) ForgeLastGreen else accent) else grey
                        Box(Modifier.fillMaxWidth().height(9.dp).background(color, RoundedCornerShape(2.dp)))
                    }
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${weeklyCounts.size} WKS AGO", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text("THIS WEEK", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        }
        Spacer(Modifier.height(12.dp))
        Box(Modifier.fillMaxWidth().border(0.5.dp, (if (progressing) ForgeLastGreen else muted).copy(alpha = 0.5f), RoundedCornerShape(10.dp)).padding(14.dp)) {
            Column {
                Text(if (progressing) "PROGRESSING" else "HOLDING STEADY", style = MaterialTheme.typography.labelSmall, color = if (progressing) ForgeLastGreen else muted, fontSize = 8.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(4.dp))
                Text(
                    if (progressing) "Sessions are trending up — keep the load climbing."
                    else "Attendance is steady. Consistency is what drives the numbers.",
                    style = MaterialTheme.typography.bodySmall, color = onBg
                )
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}
