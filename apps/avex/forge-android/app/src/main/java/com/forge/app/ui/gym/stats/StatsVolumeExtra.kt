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
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.components.formatVolume
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.gym.stats.state.BalanceRatioUi
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.RepRangeDist
import com.forge.app.ui.gym.stats.state.WeeklyTonnage
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeWarning

/**
 * "Where the work goes" — weekly working sets per muscle with the program's planned
 * target overlaid as a tick. Merges the old lb-bars + MEV/MAV landmark cards into one
 * actual-vs-plan view; renders the full plan with empty bars at zero data, so the cold
 * start still looks intentional.
 */
@Composable
internal fun MuscleTargetSection(
    actual: List<MuscleSetCount>,
    planned: Map<MuscleGroup, Int>,
    volumeLbByMuscle: Map<MuscleGroup, Double>,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val actualByMuscle = actual.associate { it.muscle to it.sets }
    val muscles = (planned.keys + actualByMuscle.keys).distinct().sortedWith(
        compareByDescending<MuscleGroup> { planned[it] ?: 0 }.thenByDescending { actualByMuscle[it] ?: 0 }
    )
    if (muscles.isEmpty()) return
    val anyLogged = actualByMuscle.values.any { it > 0 }
    val progress = rememberDrawProgress(actualByMuscle.values.sum())

    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Where the work goes", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text(
            if (anyLogged) "Working sets this week, against the plan." else "Targets set. Nothing lifted against them yet.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(14.dp))
        muscles.forEachIndexed { i, muscle ->
            val sets = actualByMuscle[muscle] ?: 0
            val target = planned[muscle] ?: 0
            val scaleMax = maxOf(target, sets, 1)
            val fillColor = when {
                target > 0 && sets >= target && sets <= target * 3 / 2 -> ForgeLastGreen
                target > 0 && sets > target * 3 / 2 -> ForgeWarning
                else -> onBg.copy(alpha = 0.8f)
            }
            val rowProgress = staggeredProgress(progress, i, muscles.size)
            Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(muscle.displayName.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp)
                    val lb = volumeLbByMuscle[muscle]?.takeIf { it > 0 }
                    Text(
                        buildString {
                            append(if (target > 0) "$sets / $target sets" else "$sets sets")
                            lb?.let { append(" · ${formatVolume(it)} lb") }
                        },
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                    )
                }
                Spacer(Modifier.height(5.dp))
                Box(Modifier.fillMaxWidth().height(6.dp).background(outline.copy(alpha = 0.15f), RoundedCornerShape(3.dp))) {
                    val frac = (sets.toFloat() / scaleMax).coerceIn(0f, 1f) * rowProgress
                    if (frac > 0f) {
                        Box(Modifier.fillMaxWidth(frac).height(6.dp).background(fillColor, RoundedCornerShape(3.dp)))
                    }
                    if (target > 0) {
                        // The plan tick: a hairline standing slightly proud of the track.
                        Box(
                            Modifier.fillMaxWidth((target.toFloat() / scaleMax).coerceIn(0f, 1f)),
                            contentAlignment = Alignment.CenterEnd
                        ) {
                            Box(Modifier.width(2.dp).height(10.dp).background(onBg.copy(alpha = 0.7f), RoundedCornerShape(1.dp)))
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/**
 * "Balance" — the engine's always-on structural ratios (push/pull, quad/ham) as
 * two-sided bars with the healthy verdict. Ports InsightEngine's counting; the gated
 * insight only speaks when out of band, these bars speak always.
 */
@Composable
internal fun BalanceRatiosSection(ratios: List<BalanceRatioUi>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (ratios.isEmpty()) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Balance", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("Working sets per side, last 28 days.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        ratios.forEach { r ->
            val statusWord: String
            val statusColor: Color
            when {
                r.total == 0 -> { statusWord = "NO SETS YET"; statusColor = muted }
                r.balanced == true -> { statusWord = "BALANCED"; statusColor = ForgeLastGreen }
                r.balanced == false -> {
                    val heavy = if (r.setsA.toDouble() / r.setsB > r.healthyHigh) r.labelA else r.labelB
                    statusWord = "${heavy.uppercase()}-HEAVY"; statusColor = ForgeWarning
                }
                else -> { statusWord = "ONE-SIDED"; statusColor = ForgeWarning }
            }
            Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    Text(r.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    Text(statusWord, style = MaterialTheme.typography.labelSmall, color = statusColor, fontSize = 8.sp, letterSpacing = 1.sp)
                }
                Spacer(Modifier.height(6.dp))
                if (r.total > 0) {
                    Row(Modifier.fillMaxWidth().height(6.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                        if (r.setsA > 0) Box(
                            Modifier.weight(r.setsA.toFloat()).height(6.dp)
                                .background(onBg.copy(alpha = 0.8f), RoundedCornerShape(3.dp))
                        )
                        if (r.setsB > 0) Box(
                            Modifier.weight(r.setsB.toFloat()).height(6.dp)
                                .background(muted.copy(alpha = 0.45f), RoundedCornerShape(3.dp))
                        )
                    }
                } else {
                    Box(Modifier.fillMaxWidth().height(6.dp).background(outline.copy(alpha = 0.12f), RoundedCornerShape(3.dp)))
                }
                Spacer(Modifier.height(4.dp))
                Text(
                    "${r.setsA} ${r.labelA} · ${r.setsB} ${r.labelB}",
                    style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                )
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

/** "The work, week by week" — tonnage bars growing in with a stagger; deload weeks ring hollow. */
@Composable
internal fun TonnageTrendCard(weeks: List<WeeklyTonnage>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    if (weeks.size < 2) return
    val maxV = weeks.maxOf { it.volumeLb }.coerceAtLeast(1.0)
    val progress = rememberDrawProgress(weeks.size)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("The work, week by week", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text("Total tonnage per week. Deload weeks ring hollow.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth().height(96.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom) {
            weeks.forEachIndexed { i, w ->
                val tile = staggeredProgress(progress, i, weeks.size)
                val frac = (w.volumeLb / maxV).toFloat() * tile
                val isLast = i == weeks.lastIndex
                Box(
                    Modifier
                        .weight(1f)
                        .height((4 + 88 * frac).dp)
                        .then(
                            if (w.isDeload)
                                Modifier.border(1.dp, muted.copy(alpha = 0.6f), RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                            else
                                Modifier.background(
                                    if (isLast) accent else onBg.copy(alpha = 0.75f),
                                    RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp)
                                )
                        )
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${weeks.size} WKS AGO", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
            Text("THIS WEEK · ${formatVolume(weeks.last().volumeLb)} LB", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
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
