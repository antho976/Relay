package com.forge.app.ui.gym.session

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatHoldLabel
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.session.state.SetDetail
import com.forge.app.ui.theme.HistoryColors
import com.forge.app.ui.theme.HistoryType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

// ─── The section head over the lifts ───────────────────────────────────────────

/** "TOP WEIGHT PER EXERCISE" and the bars/line switch on the right. */
@Composable
internal fun MetricHead(metric: SessionMetric, style: SessionChartStyle, onStyle: (SessionChartStyle) -> Unit) {
    val unit = LocalForgeSettings.current.weightUnit
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(metricHeading(metric, unit), Modifier.weight(1f))
        Spacer(Modifier.width(12.dp))
        StyleSwitch(style, onStyle)
    }
}

internal fun metricHeading(metric: SessionMetric, unit: WeightUnit): String = when (metric) {
    SessionMetric.WEIGHT -> "Top weight per exercise"
    SessionMetric.VOLUME -> "Volume per exercise · ${unitLabel(unit)}"
    SessionMetric.REPS -> "Reps per exercise"
    SessionMetric.RPE -> "Effort per exercise · avg RPE"
}

/** "BARS · LINE": the chosen one in ink. Each word is its own 48dp target. */
@Composable
private fun StyleSwitch(style: SessionChartStyle, onStyle: (SessionChartStyle) -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(verticalAlignment = Alignment.CenterVertically) {
        SessionChartStyle.entries.forEachIndexed { i, s ->
            if (i > 0) OpenLabel(" · ")
            OpenLabel(
                s.label,
                Modifier
                    .selectable(selected = s == style, role = Role.RadioButton) { onStyle(s) }
                    .padding(vertical = 12.dp, horizontal = 2.dp),
                color = if (s == style) ink else muted
            )
        }
    }
}

// ─── One lift ─────────────────────────────────────────────────────────────────

/**
 * One lift under the metric switch: its name, the metric's value and a chevron, then a 4dp bar
 * against the session's largest. Tapping opens it: the readings, its note, the per-set chart in the
 * page's metric and the sets themselves. The first lift opens by default.
 */
@Composable
internal fun LiftRow(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    maxValue: Double,
    defaultOpen: Boolean,
    onOpenLiftTrend: ((String) -> Unit)?
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val unit = LocalForgeSettings.current.weightUnit
    var open by rememberSaveable(ex.name) { mutableStateOf(defaultOpen) }
    val value = ex.metricValue(metric)
    val share = if (maxValue > 0) (value / maxValue).toFloat().coerceIn(0f, 1f) else 0f

    Column(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
        Column(
            Modifier
                .fillMaxWidth()
                .bounceCombinedClick(
                    onClickLabel = if (open) "Close ${ex.name}" else "Open ${ex.name}",
                    role = Role.Button
                ) { open = !open }
                .padding(vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    ex.name,
                    style = OpenType.rowTitle.copy(fontWeight = if (open) FontWeight.SemiBold else FontWeight.Normal),
                    color = if (open) ink else HistoryColors.NameQuiet,
                    modifier = Modifier.weight(1f)
                )
                Text(liftValue(ex, metric, unit), style = HistoryType.liftFigure, color = if (open) ink else muted)
                Icon(
                    if (open) OpenIcons.ChevronUp else OpenIcons.ChevronDown,
                    contentDescription = null,
                    tint = muted,
                    modifier = Modifier.size(16.dp)
                )
            }
            Box(Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(OpenColors.Unreached)) {
                if (share > 0f) Box(
                    Modifier.fillMaxWidth(share).height(4.dp).clip(RoundedCornerShape(2.dp))
                        .background(if (open) accent else OpenColors.Bar)
                )
            }
        }
        if (open) {
            Spacer(Modifier.height(8.dp))
            LiftBody(ex, metric, style, onOpenLiftTrend)
            Spacer(Modifier.height(14.dp))
        }
    }
}

/** The row's value for [metric]: "195 lb", "4,220", "22", "8.0". A bodyweight lift reads BW. */
internal fun liftValue(ex: ExerciseDetail, metric: SessionMetric, unit: WeightUnit): String = when (metric) {
    SessionMetric.WEIGHT -> ex.topWeightLb?.takeIf { it > 0 }?.let { formatWeight(it, unit) }
        ?: if (ex.sets.isNotEmpty() && ex.sets.all { it.durationSeconds != null }) formatHoldLabel(ex.sets.maxOf { it.durationSeconds ?: 0 })
        else "BW"
    SessionMetric.VOLUME -> String.format(java.util.Locale.getDefault(), "%,d", toDisplayWeight(ex.volumeLb, unit).toLong())
    SessionMetric.REPS -> "${ex.totalReps}"
    SessionMetric.RPE -> if (ex.sets.any { it.rpe != null }) String.format(java.util.Locale.getDefault(), "%.1f", ex.avgRpe) else "0"
}

/**
 * An opened lift: its readings (e1RM, PR, top set against last time, effort) as bare mono words,
 * the note, the per-set chart in the page's metric, the sets, and the way to its own page.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LiftBody(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    onOpenLiftTrend: ((String) -> Unit)?
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val unit = LocalForgeSettings.current.weightUnit
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            ex.e1rmLb?.let { e ->
                OpenLabel("e1RM ${weightInputValue(e, unit)}" + if (ex.e1rmIsBest) " · best" else "", style = OpenType.monoXS, color = ink)
            }
            if (ex.isPr) OpenLabel("↑ PR", style = OpenType.monoXS, color = accent)
            topSetChange(ex, unit)?.let { (text, moved) -> OpenLabel(text, style = OpenType.monoXS, color = if (moved) ink else muted) }
            if (ex.sets.any { it.rpe != null }) {
                OpenLabel("Effort ${rpeLabel(ex.avgRpe)}", style = OpenType.monoXS, color = if (metric == SessionMetric.RPE) ink else muted)
            }
            ex.effort?.let { OpenLabel("Felt ${it.displayName.lowercase()}", style = OpenType.monoXS) }
        }
        if (!ex.note.isNullOrBlank()) Text("“${ex.note}”", style = HistoryType.journal, color = muted)
        SetChart(ex, metric, style)
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            ex.sets.forEach { SetLine(it, unit) }
        }
        val liftId = ex.exerciseId
        if (onOpenLiftTrend != null && liftId != null) {
            Spacer(Modifier.height(2.dp))
            Row(
                Modifier
                    .touchSlop(vertical = 12.dp)
                    .clickableLabeled("Open ${ex.name}'s own page") { onOpenLiftTrend(liftId) }
                    .padding(vertical = 12.dp)
                    .heightIn(min = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OpenLabel("All-time trend")
                Icon(OpenIcons.ChevronRight, contentDescription = null, tint = muted, modifier = Modifier.size(14.dp))
            }
        }
    }
}

/** "Top set +5 lb" against the last time this lift was done; "Top set same" when it matched. */
internal fun topSetChange(ex: ExerciseDetail, unit: WeightUnit): Pair<String, Boolean>? {
    val now = ex.topWeightLb?.takeIf { it > 0 } ?: return null
    val prev = ex.prevTopWeightLb?.takeIf { it > 0 } ?: return null
    val diff = now - prev
    if (kotlin.math.abs(diff) < 0.01) return "Top set same" to false
    val sign = if (diff > 0) "+" else "−"
    return "Top set $sign${weightInputValue(kotlin.math.abs(diff), unit)} ${unitLabel(unit)}" to true
}

/** The per-set chart in the page's metric: columns by default, or the line the switch picks. */
@Composable
private fun SetChart(ex: ExerciseDetail, metric: SessionMetric, style: SessionChartStyle) {
    val unit = LocalForgeSettings.current.weightUnit
    val accent = MaterialTheme.colorScheme.primary
    val rows: List<Pair<SetDetail, Double>> = when (metric) {
        SessionMetric.RPE -> ex.sets.mapNotNull { s -> s.rpe?.let { s to it } }
        SessionMetric.WEIGHT, SessionMetric.VOLUME -> ex.sets.map { it to toDisplayWeight(it.metricValue(metric), unit) }
        SessionMetric.REPS -> ex.sets.map { it to it.reps.toDouble() }
    }
    // One set is a row, not a chart: the set line below already says it.
    if (rows.size < 2 || rows.none { it.second > 0.0 }) return
    val description = "${metricTabLabel(metric)} per set: " + rows.joinToString(", ") { "set ${it.first.number} ${rpeLabel(it.second)}" }
    if (style == SessionChartStyle.LINE) {
        PerSetLine(rows.map { it.second }, metric, description)
        return
    }
    val top = rows.maxOf { it.second }
    OpenColumnChart(
        values = rows.map { it.second.toFloat() },
        // Weight lights the heaviest sets; the other metrics are all the point.
        colors = rows.map { (_, v) -> if (metric != SessionMetric.WEIGHT || v >= top - 1e-6) accent else OpenColors.Bar },
        height = 96.dp + 18.dp,
        columnLabels = rows.map { "Set ${it.first.number}" },
        valueLabels = true,
        gapFraction = 0.45f,
        modifier = Modifier.padding(horizontal = 12.dp),
        description = description
    )
}

/** One set: its number, "195 × 5" in serif, then its marks and RPE on the right. */
@Composable
private fun SetLine(s: SetDetail, unit: WeightUnit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
        OpenLabel("${s.number}", Modifier.widthIn(min = 10.dp), style = OpenType.monoS)
        Text(
            if (s.durationSeconds != null) buildAnnotatedString { append(formatHoldLabel(s.durationSeconds)) }
            else buildAnnotatedString {
                append(setWeight(s, unit))
                withStyle(HistoryType.liftFigureTimes.toSpanStyle().copy(color = muted)) { append(" × ") }
                append("${s.reps}")
            },
            style = HistoryType.liftFigure,
            color = ink,
            modifier = Modifier.weight(1f)
        )
        val marks = buildList {
            if (s.setType == "warmup") add("Warm")
            if (s.isAmrap) add("AMRAP")
            if (s.toFailure) add("Fail")
            if (s.isAssisted) add("Assist")
            if (s.setType == "drop" || !s.dropAnnotation.isNullOrBlank()) add("Drop")
            s.rpe?.let { add("RPE ${rpeLabel(it)}") }
        }
        if (marks.isNotEmpty()) OpenLabel(marks.joinToString(" · "), style = OpenType.monoXS)
    }
}

/** A set's weight in the user's unit without the unit word ("195"); bodyweight reads BW. */
private fun setWeight(s: SetDetail, unit: WeightUnit): String =
    if (s.weightLb != null && s.weightLb > 0) weightInputValue(s.weightLb, unit)
    else s.weightText.ifBlank { "BW" }

