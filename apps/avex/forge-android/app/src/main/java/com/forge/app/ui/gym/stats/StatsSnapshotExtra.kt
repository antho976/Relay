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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.theme.emphasized
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.CountUpText
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.formatVolume
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.state.OverloadSummary
import com.forge.app.ui.gym.stats.state.PeriodStats
import com.forge.app.ui.gym.stats.state.PulseBand
import com.forge.app.ui.gym.stats.state.ReadinessPulse
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeWarning
import com.forge.app.ui.theme.LocalForgeSettings

/** "MOMENTUM · VS LAST WEEK" — a 2×2 grid of this-week values with deltas vs last week. */
@Composable
internal fun MomentumGrid(current: PeriodStats?, previous: PeriodStats?, onBg: Color, muted: Color, outline: Color) {
    val cur = current ?: PeriodStats(0, 0.0, 0, 0)
    val prev = previous ?: PeriodStats(0, 0.0, 0, 0)
    val useKg = LocalForgeSettings.current.useKg
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("MOMENTUM · VS LAST WEEK", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        Column(
            Modifier.fillMaxWidth().border(0.5.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp)).padding(16.dp)
        ) {
            Row(Modifier.fillMaxWidth()) {
                MomentumCell("SESSIONS", "${cur.sessions}", cur.sessions.toDouble(), prev.sessions.toDouble(), "was ${prev.sessions}", onBg, muted, Modifier.weight(1f))
                MomentumCell("VOLUME ${unitLabel(useKg).uppercase()}", formatVolume(cur.volumeLb, useKg), cur.volumeLb, prev.volumeLb, "was ${formatVolume(prev.volumeLb, useKg)}", onBg, muted, Modifier.weight(1f))
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
            Text(valueText, style = MaterialTheme.typography.headlineMedium, color = emphasized(onBg))
            deltaText?.let {
                Text(it, style = MaterialTheme.typography.labelSmall, color = deltaColor, fontSize = 10.sp, modifier = Modifier.padding(bottom = 4.dp))
            }
        }
        Text(wasText, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp)
    }
}

/**
 * "Progressive overload, concretely" — the weekly average e1RM across tracked lifts as a
 * big serif number counting up from last week's value, with the weekly series drawing in.
 */
@Composable
internal fun OverloadCard(overload: OverloadSummary?, monthlyPct: Double?, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (overload == null) return
    val error = MaterialTheme.colorScheme.error
    val useKg = LocalForgeSettings.current.useKg
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("PROGRESSIVE OVERLOAD", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CountUpText(
                value = toDisplayWeight(overload.current, useKg),
                fromValue = overload.prevWeek?.let { toDisplayWeight(it, useKg) },
                style = MaterialTheme.typography.displaySmall,
                color = emphasized(onBg)
            )
            Text("AVG E1RM · ${unitLabel(useKg).uppercase()}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.padding(bottom = 8.dp))
            overload.deltaVsPrevWeek?.let { d ->
                Text(
                    "${if (d >= 0) "↑ +" else "↓ "}${String.format(java.util.Locale.US, "%.1f", toDisplayWeight(kotlin.math.abs(d), useKg))} vs last week · was ${String.format(java.util.Locale.US, "%.1f", toDisplayWeight(overload.prevWeek!!, useKg))}",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (d >= 0) ForgeLastGreen else error,
                    fontSize = 9.sp,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
            }
        }
        if (overload.weekly.size >= 2) {
            Spacer(Modifier.height(10.dp))
            Sparkline(
                values = overload.weekly, lineColor = accent,
                minValue = overload.weekly.min(), maxValue = overload.weekly.max(),
                modifier = Modifier.fillMaxWidth().height(48.dp),
                progress = rememberDrawProgress(overload.weekly.size)
            )
        }
        monthlyPct?.let {
            Spacer(Modifier.height(8.dp))
            Text(
                "Averaging ${if (it >= 0) "+" else ""}${"%.1f".format(it)}% per month across lifts.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
            )
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/**
 * "READINESS PULSE" — DeloadAdvisor's fatigue score banded Fresh / Building / Deload soon,
 * with the engine's drivers verbatim as the why-line. Null pulse = the data gates haven't
 * opened yet; that still renders as a deliberate (cold-start) state, never a gap.
 */
@Composable
internal fun PulseCard(pulse: ReadinessPulse?, onBg: Color, muted: Color, outline: Color) {
    // Null pulse = data gates not open yet; render a neutral cold-start state, NOT a green "Fresh"
    // (which would falsely signal a healthy reading when there's simply no data). (F16.)
    val color = when (pulse?.band) {
        null -> muted
        PulseBand.FRESH -> ForgeLastGreen
        PulseBand.BUILDING -> ForgeWarning
        PulseBand.DELOAD_SOON -> MaterialTheme.colorScheme.error
    }
    val label = pulse?.band?.label ?: "No read yet"
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("READINESS PULSE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Box(Modifier.size(8.dp).background(color, CircleShape))
            Text(label, style = MaterialTheme.typography.headlineMedium, color = onBg)
            if (pulse != null && pulse.score > 0) {
                Text("FATIGUE ${pulse.score}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            when {
                pulse == null -> "Not enough history to read fatigue yet. The signals accrue as you train."
                pulse.drivers.isEmpty() -> "No fatigue signals firing. Push."
                else -> pulse.drivers.joinToString(" · ")
            },
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/** "WEEK VS PLAN" — working sets logged this week against the program's planned total. */
@Composable
internal fun WeekVsPlanCard(actualSets: Int, plannedSets: Int, onBg: Color, muted: Color, outline: Color) {
    if (plannedSets <= 0) return
    val pct = actualSets * 100 / plannedSets
    val onPlan = pct in 80..110
    val color = if (onPlan) ForgeLastGreen else ForgeWarning
    val word = when {
        pct < 80 -> if (actualSets == 0) "NOT STARTED" else "BEHIND PLAN"
        pct > 110 -> "AHEAD OF PLAN"
        else -> "ON PLAN"
    }
    val progress = rememberDrawProgress(actualSets)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("WEEK VS PLAN", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Text(word, style = MaterialTheme.typography.labelSmall, color = color, fontSize = 9.sp, letterSpacing = 1.sp)
        }
        Spacer(Modifier.height(10.dp))
        Box(Modifier.fillMaxWidth().height(6.dp).background(outline.copy(alpha = 0.15f), RoundedCornerShape(3.dp))) {
            val frac = (actualSets.toFloat() / plannedSets).coerceIn(0f, 1f) * progress
            if (frac > 0f) {
                Box(Modifier.fillMaxWidth(frac).height(6.dp).background(color, RoundedCornerShape(3.dp)))
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            if (actualSets == 0) "$plannedSets working sets planned. Nothing lifted against them yet."
            else "$actualSets of $plannedSets planned working sets · $pct%",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
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
                        Text(s, style = MaterialTheme.typography.headlineMedium, color = if (overloadPct >= 0) emphasized(onBg) else error)
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
