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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.RepRangeDist
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeWarning

/** Rough per-muscle volume landmarks (MEV = minimum effective, MAV = max adaptive). */
private fun mevMav(m: MuscleGroup): Pair<Int, Int> = when (m) {
    MuscleGroup.CHEST -> 10 to 22
    MuscleGroup.BACK -> 10 to 20
    MuscleGroup.QUADS -> 8 to 18
    MuscleGroup.HAMSTRINGS -> 6 to 16
    MuscleGroup.GLUTES -> 8 to 16
    MuscleGroup.SHOULDERS -> 8 to 16
    MuscleGroup.CALVES -> 8 to 16
    MuscleGroup.CORE -> 6 to 16
    else -> 6 to 14 // biceps, triceps, rear delts
}

/** "Balance check" — flags when one muscle dominates the week's volume. */
@Composable
internal fun BalanceCheckCard(rows: List<MuscleVolume>, onBg: Color, muted: Color, outline: Color) {
    if (rows.isEmpty()) return
    val total = rows.sumOf { it.volumeLb }.coerceAtLeast(1.0)
    val top = rows.maxByOrNull { it.volumeLb } ?: return
    val pct = (top.volumeLb / total * 100).toInt()
    val skewed = pct >= 50 && rows.size >= 2
    val border = if (skewed) ForgeWarning else ForgeLastGreen
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().border(0.5.dp, border.copy(alpha = 0.5f), RoundedCornerShape(10.dp)).padding(14.dp),
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Box(Modifier.padding(top = 4.dp).height(8.dp).width(8.dp).background(border, RoundedCornerShape(4.dp)))
            Column {
                Text("BALANCE CHECK", style = MaterialTheme.typography.labelSmall, color = border, fontSize = 8.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(4.dp))
                Text(
                    if (skewed) "${top.muscle.displayName} is $pct% of weekly volume. Spread work to other muscle groups to even it out."
                    else "Volume is reasonably balanced across muscle groups this week.",
                    style = MaterialTheme.typography.bodySmall, color = onBg
                )
            }
        }
        Spacer(Modifier.height(20.dp))
    }
}

/**
 * "Are you doing enough?" — weekly sets per muscle against its MEV/MAV range, with a status
 * word (below maintenance / productive / high) and a fill bar.
 */
@Composable
internal fun VolumeLandmarkCard(items: List<MuscleSetCount>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Are you doing enough?", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("10–20 sets / muscle / week is the productive range.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        items.forEach { m ->
            val (mev, mav) = mevMav(m.muscle)
            val statusColor: Color
            val statusWord: String
            when {
                m.sets < mev -> { statusColor = ForgeWarning; statusWord = "BELOW MAINTENANCE" }
                m.sets > mav -> { statusColor = ForgeWarning; statusWord = "HIGH VOLUME" }
                else -> { statusColor = ForgeLastGreen; statusWord = "PRODUCTIVE" }
            }
            Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(m.muscle.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    Text("${m.sets} SETS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                }
                Spacer(Modifier.height(5.dp))
                Box(Modifier.fillMaxWidth().height(8.dp).background(outline.copy(alpha = 0.15f), RoundedCornerShape(4.dp))) {
                    val frac = (m.sets.toFloat() / mav).coerceIn(0f, 1f)
                    Box(Modifier.fillMaxWidth(frac).height(8.dp).background(statusColor, RoundedCornerShape(4.dp)))
                }
                Spacer(Modifier.height(3.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(statusWord, style = MaterialTheme.typography.labelSmall, color = statusColor, fontSize = 8.sp, letterSpacing = 0.5.sp)
                    Text("MEV $mev · MAV $mav", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/** Segmented bar of set counts across strength / hypertrophy / endurance rep ranges. */
@Composable
internal fun RepRangeCard(dist: RepRangeDist, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (dist.total == 0) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("REP RANGE MIX", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        Row(Modifier.fillMaxWidth().height(12.dp).background(outline.copy(alpha = 0.15f), RoundedCornerShape(6.dp))) {
            if (dist.strength > 0) Box(Modifier.weight(dist.strength.toFloat()).height(12.dp).background(accent))
            if (dist.hypertrophy > 0) Box(Modifier.weight(dist.hypertrophy.toFloat()).height(12.dp).background(accent.copy(alpha = 0.55f)))
            if (dist.endurance > 0) Box(Modifier.weight(dist.endurance.toFloat()).height(12.dp).background(muted.copy(alpha = 0.6f)))
        }
        Spacer(Modifier.height(10.dp))
        RepRangeLegend("Strength · 1–5", dist.strength, dist.total, accent, onBg, muted)
        RepRangeLegend("Hypertrophy · 6–12", dist.hypertrophy, dist.total, accent.copy(alpha = 0.55f), onBg, muted)
        RepRangeLegend("Endurance · 13+", dist.endurance, dist.total, muted.copy(alpha = 0.6f), onBg, muted)
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
private fun RepRangeLegend(label: String, count: Int, total: Int, dot: Color, onBg: Color, muted: Color) {
    val pct = if (total > 0) (count * 100 / total) else 0
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Box(Modifier.height(10.dp).width(10.dp).background(dot, RoundedCornerShape(2.dp)))
        Text(label, style = MaterialTheme.typography.bodySmall, color = onBg, modifier = Modifier.weight(1f))
        Text("$count · $pct%", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
    }
}
