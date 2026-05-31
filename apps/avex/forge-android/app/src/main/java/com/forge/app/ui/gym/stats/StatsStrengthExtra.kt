package com.forge.app.ui.gym.stats

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.RepMaxSet
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeWarning

/** "Where I'm going" — the top lift's estimated-1RM progression + a dashed projection. */
@Composable
internal fun E1rmChartCard(top: E1rmLift, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val projected = top.monthlyPct?.takeIf { it != 0.0 }?.let { top.currentE1rm * (1 + it / 100.0) }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Where I'm going", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        val deltaStr = if (top.delta >= 0) "+${top.delta.toInt()}" else "${top.delta.toInt()}"
        Text("${top.exerciseName} — $deltaStr lb since first session.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(12.dp))
        Column(Modifier.fillMaxWidth().border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(12.dp)).padding(16.dp)) {
            Text("ESTIMATED 1RM · LB", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Spacer(Modifier.height(12.dp))
            if (top.history.size >= 2) {
                E1rmProjectionChart(
                    history = top.history, projected = projected,
                    line = onBg, proj = accent, grid = outline.copy(alpha = 0.25f), muted = muted,
                    modifier = Modifier.fillMaxWidth().height(120.dp)
                )
                projected?.let {
                    Spacer(Modifier.height(10.dp))
                    Text("↗ PROJECTED ${it.toInt()} lb in ~4 wks at current rate", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                }
            } else {
                Text("Log this lift twice to see the curve.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
private fun E1rmProjectionChart(history: List<Double>, projected: Double?, line: Color, proj: Color, grid: Color, muted: Color, modifier: Modifier) {
    val combined = history + (projected?.let { listOf(it) } ?: emptyList())
    val minV = combined.min(); val maxV = combined.max(); val range = (maxV - minV).coerceAtLeast(1.0)
    Row(modifier) {
        Column(Modifier.fillMaxHeight().width(28.dp), verticalArrangement = Arrangement.SpaceBetween, horizontalAlignment = Alignment.End) {
            Text("${maxV.toInt()}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text("${((maxV + minV) / 2).toInt()}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text("${minV.toInt()}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        }
        Spacer(Modifier.width(6.dp))
        Canvas(Modifier.weight(1f).fillMaxHeight()) {
            fun yFor(v: Double) = size.height - ((v - minV) / range * size.height).toFloat()
            val dash = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 4.dp.toPx()))
            listOf(0.25f, 0.5f, 0.75f).forEach { f ->
                drawLine(grid, Offset(0f, size.height * f), Offset(size.width, size.height * f), 1.dp.toPx(), pathEffect = dash)
            }
            val total = combined.size
            val stepX = size.width / (total - 1).coerceAtLeast(1)
            val hist = Path()
            history.forEachIndexed { i, v -> val x = stepX * i; val y = yFor(v); if (i == 0) hist.moveTo(x, y) else hist.lineTo(x, y) }
            drawPath(hist, line, style = Stroke(3.dp.toPx()))
            history.forEachIndexed { i, v -> drawCircle(line, 2.5.dp.toPx(), Offset(stepX * i, yFor(v))) }
            projected?.let { pv ->
                val s = history.size - 1
                val p = Path(); p.moveTo(stepX * s, yFor(history.last())); p.lineTo(stepX * (total - 1), yFor(pv))
                drawPath(p, proj, style = Stroke(2.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 8f))))
                drawCircle(proj, 3.5.dp.toPx(), Offset(stepX * (total - 1), yFor(pv)))
            }
        }
    }
}

/** "Your big lifts" — e1RM, monthly rate, stall flag, and a mini trend per lift. */
@Composable
internal fun E1rmCard(lifts: List<E1rmLift>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Your big lifts", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("Estimated 1RM, change, and rate of overload.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(12.dp))
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text("LIFT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.weight(1f))
            Text("E1RM", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.width(44.dp))
            Text("/MO", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.width(48.dp))
            Text("TREND", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.width(44.dp))
        }
        HorizontalDivider(color = outline.copy(alpha = 0.2f), modifier = Modifier.padding(top = 4.dp))
        lifts.forEach { lift ->
            Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(lift.exerciseName, style = MaterialTheme.typography.bodySmall, color = onBg)
                    if (lift.stalling) {
                        Box(Modifier.border(0.5.dp, ForgeWarning.copy(alpha = 0.6f), RoundedCornerShape(3.dp)).padding(horizontal = 4.dp, vertical = 1.dp)) {
                            Text("STALL", style = MaterialTheme.typography.labelSmall, color = ForgeWarning, fontSize = 7.sp)
                        }
                    }
                }
                Text("${lift.currentE1rm.toInt()}", style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold, modifier = Modifier.width(44.dp))
                val moColor = when {
                    lift.stalling -> ForgeWarning
                    (lift.monthlyPct ?: 0.0) > 0 -> ForgeLastGreen
                    else -> muted
                }
                Text(lift.monthlyPct?.let { "${if (it >= 0) "+" else ""}${"%.1f".format(it)}%" } ?: "—",
                    style = MaterialTheme.typography.labelSmall, color = moColor, fontSize = 10.sp, modifier = Modifier.width(48.dp))
                Box(Modifier.width(44.dp).height(18.dp), contentAlignment = Alignment.Center) {
                    if (lift.history.size >= 2) {
                        Sparkline(values = lift.history, lineColor = onBg.copy(alpha = 0.6f), minValue = lift.history.min(), maxValue = lift.history.max(),
                            modifier = Modifier.fillMaxWidth().height(16.dp))
                    }
                }
            }
            HorizontalDivider(color = outline.copy(alpha = 0.1f))
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/** "How strong, how many reps" — best weight at each rep count, as bars. */
@Composable
internal fun RepMaxCard(repMaxes: RepMaxSet, onBg: Color, muted: Color, outline: Color) {
    if (repMaxes.entries.isEmpty()) return
    val maxW = repMaxes.entries.maxOf { it.weightLb }.coerceAtLeast(1.0)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("How strong, how many reps", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("${repMaxes.exerciseName} — best weight at each rep count.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth().height(120.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Bottom) {
            repMaxes.entries.forEach { e ->
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("${e.weightLb.toInt()}", style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp)
                    val frac = (e.weightLb / maxW).toFloat()
                    Box(Modifier.fillMaxWidth().height((8 + 80 * frac).dp).background(onBg.copy(alpha = 0.85f), RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp)))
                    Text(if (e.reps == 1) "1RM" else "${e.reps}r", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}
