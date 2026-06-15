package com.forge.app.ui.gym.session

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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionDetailData
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.session.state.SetDetail
import com.forge.app.ui.overview.SummaryStat
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

// ─── Header + summary ───────────────────────────────────────────────────────--

@Composable
internal fun SessionHeader(data: SessionDetailData, onBg: Color, muted: Color, outline: Color) {
    val zone = ZoneId.systemDefault()
    val date = Instant.ofEpochMilli(data.dateMs).atZone(zone).toLocalDate()
    val dateStr = date.format(DateTimeFormatter.ofPattern("EEEE, d MMMM", Locale.getDefault()))
    Column(Modifier.fillMaxWidth()) {
        val sub = buildList {
            // Deload marker wins; otherwise the stored session-type's pill (TEST/TECHNIQUE/… once a
            // picker writes them) — same resolution as the Overview status pill so the two can't drift.
            (if (data.deload) SessionType.DELOAD.pillLabel else SessionType.fromKey(data.sessionType)?.pillLabel)
                ?.let { add(it) }
            if (data.intensity.isNotBlank() && data.intensity != "normal") add(data.intensity.uppercase())
            // Quick tags are stored comma-separated (#107); add each as its own token so the sub-line
            // never shows a raw "FOCUS,HEAVY" (the Overview pill splits them the same way).
            data.tag.split(",").forEach { t -> t.trim().takeIf { it.isNotEmpty() }?.let { add(it.uppercase()) } }
        }.joinToString(" · ")
        if (sub.isNotEmpty()) {
            Text(sub, style = MaterialTheme.typography.labelSmall, letterSpacing = 1.5.sp, color = muted, fontSize = 9.sp)
            Spacer(Modifier.height(2.dp))
        }
        Text(data.title, style = MaterialTheme.typography.headlineSmall, color = onBg, fontWeight = FontWeight.Normal)
        Text(dateStr, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp, fontStyle = FontStyle.Italic)
        if (data.journal.isNotBlank()) {
            Spacer(Modifier.height(10.dp))
            Text("“${data.journal}”", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        }
    }
}

/** The top stats strip — reuses the overview's [SummaryStat] so the language matches the rest of history. */
@Composable
internal fun SummaryStrip(data: SessionDetailData, onBg: Color, muted: Color) {
    val useKg = LocalForgeSettings.current.useKg
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
        data.volumeLb?.takeIf { it > 0 }?.let { SummaryStat(formatVolume(it, useKg), "VOLUME", muted, onBg) }
        data.durationMin?.takeIf { it > 0 }?.let { SummaryStat("$it min", "DURATION", muted, onBg) }
        if (data.setCount > 0) SummaryStat("${data.setCount}", "SETS", muted, onBg)
        if (data.prCount > 0) SummaryStat("${data.prCount}", "PRs", muted, onBg)
        data.avgRpe?.let { SummaryStat(rpeLabel(it), "AVG RPE", muted, onBg) }
    }
}

// ─── Exercise card ───────────────────────────────────────────────────────────-

@Composable
internal fun ExerciseCard(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Column(
        modifier = Modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(ex.name, style = MaterialTheme.typography.bodyLarge, color = onBg, modifier = Modifier.weight(1f))
            if (ex.isPr) Chip("PR", accent, accent.copy(alpha = 0.15f))
            ex.effort?.let { Chip(it.displayName, effortColor(it), effortColor(it).copy(alpha = 0.15f)) }
        }
        if (!ex.note.isNullOrBlank()) {
            Text("“${ex.note}”", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
        }

        SetTable(ex.sets, onBg, muted, accent, outline)

        // The per-exercise graph (bars or line) in the page's chosen metric.
        PerExerciseSetChart(ex, metric, style, accent, muted, outline)
    }
}

@Composable
private fun SetTable(sets: List<SetDetail>, onBg: Color, muted: Color, accent: Color, outline: Color) {
    val useKg = LocalForgeSettings.current.useKg
    Column {
        sets.forEachIndexed { i, s ->
            if (i > 0) HorizontalDivider(color = outline.copy(alpha = 0.1f))
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text("${s.number}", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f), fontSize = 10.sp, modifier = Modifier.width(14.dp))
                    Text(
                        "${weightLabel(s, useKg)} × ${s.reps}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (s.isTopSet) accent else onBg,
                        fontWeight = if (s.isTopSet) FontWeight.SemiBold else FontWeight.Normal
                    )
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (s.isAmrap) SetTag("AMRAP", muted, outline)
                    if (s.toFailure) SetTag("FAIL", muted, outline)
                    if (s.isAssisted) SetTag("ASSIST", muted, outline)
                    if (!s.dropAnnotation.isNullOrBlank()) SetTag("DROP", muted, outline)
                    s.rpe?.let {
                        Text("RPE ${rpeLabel(it)}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    }
                }
            }
        }
    }
}

// ─── Small bits ────────────────────────────────────────────────────────────--

@Composable
private fun Chip(text: String, fg: Color, bg: Color) {
    Box(
        modifier = Modifier.clip(RoundedCornerShape(50)).background(bg).padding(horizontal = 8.dp, vertical = 2.dp)
    ) {
        Text(text, style = MaterialTheme.typography.labelSmall, color = fg, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp)
    }
}

@Composable
private fun SetTag(text: String, muted: Color, outline: Color) {
    Box(
        modifier = Modifier.border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(3.dp)).padding(horizontal = 4.dp, vertical = 1.dp)
    ) {
        Text(text, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 7.sp, letterSpacing = 0.5.sp)
    }
}

private fun weightLabel(s: SetDetail, useKg: Boolean): String =
    if (s.weightLb != null && s.weightLb > 0) formatWeight(s.weightLb, useKg) else s.weightText.ifBlank { "BW" }

private fun effortColor(effort: EffortRating): Color = when (effort) {
    EffortRating.EASY -> Color(0xFF4CAF50)
    EffortRating.JUST_RIGHT -> Color(0xFF2196F3)
    EffortRating.HARD -> Color(0xFFFF9800)
    EffortRating.BRUTAL -> Color(0xFFE53935)
}
