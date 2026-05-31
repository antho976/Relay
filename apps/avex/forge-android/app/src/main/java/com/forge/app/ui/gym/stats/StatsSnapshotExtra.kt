package com.forge.app.ui.gym.stats

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.formatVolume
import com.forge.app.ui.gym.stats.state.PeriodStats
import com.forge.app.ui.theme.ForgeLastGreen

/** "MOMENTUM · VS LAST WEEK" — a 2×2 grid of this-week values with deltas vs last week. */
@Composable
internal fun MomentumGrid(current: PeriodStats?, previous: PeriodStats?, onBg: Color, muted: Color, outline: Color) {
    val cur = current ?: PeriodStats(0, 0.0, 0, 0)
    val prev = previous ?: PeriodStats(0, 0.0, 0, 0)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("MOMENTUM · VS LAST WEEK", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        Column(
            Modifier.fillMaxWidth().border(0.5.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp)).padding(16.dp)
        ) {
            Row(Modifier.fillMaxWidth()) {
                MomentumCell("SESSIONS", "${cur.sessions}", cur.sessions.toDouble(), prev.sessions.toDouble(), "was ${prev.sessions}", onBg, muted, Modifier.weight(1f))
                MomentumCell("VOLUME LB", formatVolume(cur.volumeLb), cur.volumeLb, prev.volumeLb, "was ${formatVolume(prev.volumeLb)}", onBg, muted, Modifier.weight(1f))
            }
            Spacer(Modifier.height(16.dp))
            Row(Modifier.fillMaxWidth()) {
                MomentumCell("PRS", "${cur.prs}", cur.prs.toDouble(), prev.prs.toDouble(), "was ${prev.prs}", onBg, muted, Modifier.weight(1f))
                MomentumCell("SETS", "${cur.sets}", cur.sets.toDouble(), prev.sets.toDouble(), "was ${prev.sets}", onBg, muted, Modifier.weight(1f))
            }
        }
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
private fun MomentumCell(
    label: String, valueText: String, cur: Double, prev: Double, wasText: String,
    onBg: Color, muted: Color, modifier: Modifier = Modifier
) {
    val error = MaterialTheme.colorScheme.error
    val deltaText: String?
    val deltaColor: Color
    when {
        prev <= 0.0 && cur > 0.0 -> { deltaText = "↑ +new"; deltaColor = ForgeLastGreen }
        prev > 0.0 -> {
            val pct = ((cur - prev) / prev * 100).toInt()
            when {
                pct > 0 -> { deltaText = "↑ +$pct%"; deltaColor = ForgeLastGreen }
                pct < 0 -> { deltaText = "↓ $pct%"; deltaColor = error }
                else -> { deltaText = null; deltaColor = muted }
            }
        }
        else -> { deltaText = null; deltaColor = muted }
    }
    Column(modifier) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp)
        Spacer(Modifier.height(2.dp))
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(valueText, style = MaterialTheme.typography.headlineMedium, color = onBg)
            deltaText?.let {
                Text(it, style = MaterialTheme.typography.labelSmall, color = deltaColor, fontSize = 10.sp, modifier = Modifier.padding(bottom = 4.dp))
            }
        }
        Text(wasText, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp)
    }
}

/** Consistency-streak + progressive-overload highlight cards, side by side. */
@Composable
internal fun HighlightCards(streakWeeks: Int, overloadPct: Double?, onBg: Color, muted: Color, outline: Color) {
    val error = MaterialTheme.colorScheme.error
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(
                Modifier.weight(1f).border(0.5.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp)).padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("$streakWeeks", style = MaterialTheme.typography.headlineMedium, color = ForgeLastGreen)
                    Text("WK", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.padding(bottom = 4.dp))
                }
                Text("CONSISTENCY STREAK", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, letterSpacing = 0.5.sp)
                Text("$streakWeeks weeks ≥ target", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
            }
            Column(
                Modifier.weight(1f).border(0.5.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp)).padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (overloadPct != null) {
                        val s = (if (overloadPct >= 0) "+" else "") + "%.1f".format(overloadPct)
                        Text(s, style = MaterialTheme.typography.headlineMedium, color = if (overloadPct >= 0) onBg else error)
                        Text("%/MO", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.padding(bottom = 4.dp))
                    } else {
                        Text("—", style = MaterialTheme.typography.headlineMedium, color = muted)
                    }
                }
                Text("PROGRESSIVE OVERLOAD", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, letterSpacing = 0.5.sp)
                Text("avg load across lifts", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
            }
        }
        Spacer(Modifier.height(20.dp))
    }
}
