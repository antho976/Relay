package com.forge.app.ui.gym.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
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
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.theme.LocalForgeSettings

/** Volume/weight/reps value as a short label for the chosen metric (weight & volume honor the kg setting). */
internal fun formatMetricValue(value: Double, metric: SessionMetric, useKg: Boolean): String = when (metric) {
    SessionMetric.WEIGHT -> formatWeight(value, useKg)
    SessionMetric.VOLUME -> formatVolume(value, useKg)
    SessionMetric.REPS -> "${value.toInt()}"
    SessionMetric.RPE -> "RPE ${rpeLabel(value)}"
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
    // Stacked onto two lines: 4 metric pills (incl. RPE) + 2 style pills won't fit one row on a
    // narrow phone. Metric chooses what every chart plots; Style chooses bars vs line.
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp)
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

// ─── Session overview: chosen metric per exercise (bars or line) ────────────────

/**
 * Per-exercise comparison for the chosen metric. The page [SessionChartStyle] toggle drives it just
 * like the per-exercise charts: BARS = labelled horizontal bars (good for reading each value), LINE
 * = a left-to-right sparkline of the same values with a peak callout (the session's "shape").
 */
@Composable
internal fun MetricByExerciseChart(
    exercises: List<ExerciseDetail>,
    metric: SessionMetric,
    style: SessionChartStyle,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    // RPE is only meaningful for exercises that actually logged one — a 0 would read as "trained at
    // RPE 0" rather than "no data", so drop them (matches PerExerciseSetChart's per-set handling).
    val shown = if (metric == SessionMetric.RPE) exercises.filter { it.avgRpe > 0.0 } else exercises
    val values = shown.map { it.metricValue(metric) }   // computed once; reused for max + each bar
    val rawMax = values.maxOrNull() ?: 0.0
    // The Weight metric is meaningless for a bodyweight-only session — say so instead of empty bars.
    if (rawMax <= 0.0) {
        Text(
            "No ${metric.label.lowercase()} logged for this session.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
        )
        return
    }
    // A line needs ≥2 points; a one-exercise session always reads as bars.
    if (style == SessionChartStyle.LINE && values.size >= 2) {
        MetricByExerciseLine(shown, values, metric, useKg, accent, muted, onBg)
    } else {
        MetricByExerciseBars(shown, values, rawMax, metric, useKg, onBg, muted, accent, outline)
    }
}

@Composable
private fun MetricByExerciseBars(
    exercises: List<ExerciseDetail>,
    values: List<Double>,
    rawMax: Double,
    metric: SessionMetric,
    useKg: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val progress = rememberDrawProgress(metric)
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        exercises.forEachIndexed { i, ex ->
            val value = values[i]
            val frac = (value / rawMax).toFloat() * staggeredProgress(progress, i, exercises.size)
            Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(ex.name, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    Text(
                        formatMetricValue(value, metric, useKg),
                        style = MaterialTheme.typography.labelMedium,
                        color = muted
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

@Composable
private fun MetricByExerciseLine(
    exercises: List<ExerciseDetail>,
    values: List<Double>,
    metric: SessionMetric,
    useKg: Boolean,
    accent: Color,
    muted: Color,
    onBg: Color
) {
    val lo = values.min()
    val hi = values.max()
    val pad = if (hi - lo < 1e-6) 1.0 else 0.0
    val peakIdx = values.indexOf(hi)
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Sparkline(
            values = values,
            lineColor = accent,
            minValue = lo - pad,
            maxValue = hi + pad,
            modifier = Modifier.fillMaxWidth().height(72.dp),
            progress = rememberDrawProgress(metric)
        )
        Text(
            "Peak · ${exercises[peakIdx].name} (${formatMetricValue(hi, metric, useKg)}) — left→right is exercise order",
            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
        )
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
    // RPE is only meaningful on the sets that logged one — drop the rest so unrated sets don't read
    // as a 0 trough. Every other metric maps one value per set.
    val values = if (metric == SessionMetric.RPE) ex.sets.mapNotNull { it.rpe }
    else ex.sets.map { it.metricValue(metric) }
    // Nothing to plot (e.g. a bodyweight exercise under the Weight metric) — show why, don't vanish.
    if (values.isEmpty() || values.none { it > 0.0 }) {
        Text(
            "No ${metric.label.lowercase()} logged for this exercise.",
            style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f),
            fontStyle = FontStyle.Italic, fontSize = 10.sp
        )
        return
    }

    when {
        style != SessionChartStyle.LINE -> SetBars(values, metric, accent)
        values.size >= 2 -> {
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
        // A single-set exercise can't draw a line — show one endpoint dot so Line mode stays
        // visually uniform instead of silently flipping back to a bar.
        else -> SinglePointMark(accent)
    }
}

/** One centred dot — the Line-mode stand-in for an exercise with a single logged set. */
@Composable
private fun SinglePointMark(accent: Color) {
    Box(modifier = Modifier.fillMaxWidth().height(56.dp), contentAlignment = Alignment.Center) {
        Box(modifier = Modifier.size(8.dp).clip(RoundedCornerShape(50)).background(accent))
    }
}

/** Vertical per-set bars, growing from the baseline with a staggered reveal; last (latest) set bold. */
@Composable
private fun SetBars(values: List<Double>, metric: SessionMetric, accent: Color) {
    val max = (values.maxOrNull() ?: 0.0).coerceAtLeast(1.0)
    // Keyed by metric so the first draw animates in. The play-once motion kit doesn't replay on a
    // later metric switch, so BARS (like the LINE branch) just snap to the new values — matching Stats.
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
