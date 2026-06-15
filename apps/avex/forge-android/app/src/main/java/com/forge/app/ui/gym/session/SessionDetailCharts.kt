package com.forge.app.ui.gym.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.overview.formatVolumeLb
import com.forge.app.ui.theme.LocalForgeSettings

/** Volume/weight/reps value as a short label for the chosen metric (weight honors the kg setting). */
internal fun formatMetricValue(value: Double, metric: SessionMetric, useKg: Boolean): String = when (metric) {
    SessionMetric.WEIGHT -> formatWeight(value, useKg)
    SessionMetric.VOLUME -> formatVolumeLb(value)
    SessionMetric.REPS -> "${value.toInt()}"
}

// ─── Page controls ────────────────────────────────────────────────────────────

/** The two segmented toggles (Metric · Style) that restyle every chart on the page at once. */
@Composable
internal fun MetricStyleControls(
    metric: SessionMetric,
    style: SessionChartStyle,
    onMetric: (SessionMetric) -> Unit,
    onStyle: (SessionChartStyle) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        SegmentRow(
            items = SessionMetric.entries,
            isSelected = { it == metric },
            label = { it.label },
            onSelect = onMetric,
            onBg = onBg, muted = muted, accent = accent, outline = outline
        )
        SegmentRow(
            items = SessionChartStyle.entries,
            isSelected = { it == style },
            label = { it.label },
            onSelect = onStyle,
            onBg = onBg, muted = muted, accent = accent, outline = outline
        )
    }
}

@Composable
private fun <T> SegmentRow(
    items: List<T>,
    isSelected: (T) -> Boolean,
    label: (T) -> String,
    onSelect: (T) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        items.forEach { item ->
            SegmentPill(
                text = label(item),
                selected = isSelected(item),
                onClick = { onSelect(item) },
                accent = accent, onBg = onBg, muted = muted, outline = outline
            )
        }
    }
}

// ─── Session overview: chosen metric per exercise (horizontal bars) ─────────────

@Composable
internal fun MetricByExerciseChart(
    exercises: List<ExerciseDetail>,
    metric: SessionMetric,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    val values = exercises.map { it.metricValue(metric) }   // computed once; reused for max + each bar
    val rawMax = values.maxOrNull() ?: 0.0
    // The Weight metric is meaningless for a bodyweight-only session — say so instead of empty bars.
    if (rawMax <= 0.0) {
        Text(
            "No ${metric.label.lowercase()} logged for this session.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
        )
        return
    }
    val progress = rememberDrawProgress(metric)
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        exercises.forEachIndexed { i, ex ->
            val value = values[i]
            val frac = (value / rawMax).toFloat() * staggeredProgress(progress, i, exercises.size)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(ex.name, style = MaterialTheme.typography.bodySmall, color = onBg, fontSize = 12.sp)
                    Text(
                        formatMetricValue(value, metric, useKg),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 10.sp
                    )
                }
                Box(
                    modifier = Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(50))
                        .background(outline.copy(alpha = 0.18f))
                ) {
                    Box(
                        modifier = Modifier.fillMaxWidth(frac.coerceIn(0f, 1f)).fillMaxHeight()
                            .clip(RoundedCornerShape(50)).background(accent)
                    )
                }
            }
        }
    }
}

// ─── Per-exercise: chosen metric per set (bars or line) ─────────────────────────

@Composable
internal fun PerExerciseSetChart(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    accent: Color,
    muted: Color,
    outline: Color
) {
    val values = ex.sets.map { it.metricValue(metric) }
    // Nothing to plot (e.g. a bodyweight exercise under the Weight metric) — show why, don't vanish.
    if (values.none { it > 0.0 }) {
        Text(
            "No ${metric.label.lowercase()} logged for this exercise.",
            style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f),
            fontStyle = FontStyle.Italic, fontSize = 10.sp
        )
        return
    }

    when {
        style == SessionChartStyle.LINE && values.size >= 2 -> {
            val lo = values.min()
            val hi = values.max()
            // A flat series (every set at the same weight) would otherwise glue the line to the
            // bottom edge — pad the range so it draws as a centred flat line.
            val pad = if (hi - lo < 1e-6) 1.0 else 0.0
            Sparkline(
                values = values,
                lineColor = accent,
                minValue = lo - pad,
                maxValue = hi + pad,
                modifier = Modifier.fillMaxWidth().height(56.dp),
                progress = rememberDrawProgress(metric)
            )
        }
        else -> SetBars(values, metric, accent)
    }
}

/** Vertical per-set bars, growing from the baseline with a staggered reveal; last (latest) set bold. */
@Composable
private fun SetBars(values: List<Double>, metric: SessionMetric, accent: Color) {
    val max = (values.maxOrNull() ?: 0.0).coerceAtLeast(1.0)
    // Re-reveal when the metric changes — matches the LINE branch's key so BARS/LINE animate alike.
    val progress = rememberDrawProgress(metric)
    Row(
        modifier = Modifier.fillMaxWidth().height(56.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        values.forEachIndexed { i, v ->
            val frac = ((v / max).toFloat() * staggeredProgress(progress, i, values.size)).coerceIn(0.03f, 1f)
            Box(
                modifier = Modifier.weight(1f).fillMaxHeight(frac)
                    .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                    .background(if (i == values.lastIndex) accent else accent.copy(alpha = 0.5f))
            )
        }
    }
}
