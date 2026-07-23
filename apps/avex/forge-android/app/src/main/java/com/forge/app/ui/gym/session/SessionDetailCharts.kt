package com.forge.app.ui.gym.session

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.theme.ForgeMotion

/** Volume/weight/reps value as a short label for the chosen metric (weight & volume honor the kg setting). */
internal fun formatMetricValue(value: Double, metric: SessionMetric, weightUnit: WeightUnit): String = when (metric) {
    SessionMetric.WEIGHT -> formatWeight(value, weightUnit)
    SessionMetric.VOLUME -> formatVolume(value, weightUnit)
    SessionMetric.REPS -> "${value.toInt()}"
    SessionMetric.RPE -> "RPE ${rpeLabel(value)}"
}

// ─── Page controls ────────────────────────────────────────────────────────────

/**
 * One segmented toggle row, reused for the page-level metric picker (Weight/Volume/Reps/RPE) and the
 * per-card bars/line style toggle. The bars/line toggle now lives on each metric card rather than
 * being a single page-wide switch, so each "stat" carries its own style.
 */
@Composable
internal fun <T> SegmentRow(
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

// ─── Per-exercise: chosen metric per set (bars or line) ─────────────────────────

@Composable
internal fun PerExerciseSetChart(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    accent: Color,
    muted: Color,
    outline: Color,
    /** Background color of the surface the chart sits on — used for the dot ring cut-out in LINE mode. */
    pageBg: Color = MaterialTheme.colorScheme.background
) {
    // RPE is only meaningful on the sets that logged one — drop the rest so unrated sets don't read
    // as a 0 trough. Every other metric maps one value per set.
    val values = if (metric == SessionMetric.RPE) ex.sets.mapNotNull { it.rpe }
    else ex.sets.map { it.metricValue(metric) }
    // Nothing to plot (e.g. a bodyweight exercise under the Weight metric) — show why, don't vanish.
    if (values.isEmpty() || values.none { it > 0.0 }) {
        Text(
            "No ${metric.label.lowercase()} logged for this exercise.",
            style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f),
            fontStyle = FontStyle.Italic, fontSize = 10.sp
        )
        return
    }

    when {
        style != SessionChartStyle.LINE -> SetBars(values, metric, accent)
        values.size >= 2 -> PerSetLine(values, metric, accent, pageBg)
        // A single-set exercise can't draw a line — show one endpoint dot so Line mode stays
        // visually uniform instead of silently flipping back to a bar.
        else -> SinglePointMark(accent)
    }
}

/**
 * The per-set line: a smooth (Catmull-Rom) accent stroke over a soft gradient fill, with a "cut-out"
 * donut marker on every set and a faint underglow — a richer read than a hairline polyline. Every edge
 * is inset so end markers + the rounded stroke never clip ("points going outside"); the latest set's
 * marker is emphasized with a hollow centre. Reveals left→right; a flat series draws as a centred line.
 */
@Composable
private fun PerSetLine(values: List<Double>, metric: SessionMetric, accent: Color, pageBg: Color) {
    // Reveal starts immediately but rides the slow, gentle draw curve so the line glides in over
    // ~0.9 s instead of snapping — the default enter tween front-loads the motion and reads as sharp.
    val progress = rememberDrawProgress(metric, ForgeMotion.drawTween())
    // The marker ring "cuts" each point out of the page background so dots read crisply on the open
    // surface — now uses pageBg directly since there's no card fill beneath the chart.
    val haloBg = pageBg
    val lo = values.min()
    val hi = values.max()
    val pad = if (hi - lo < 1e-6) 1.0 else 0.0
    val minV = lo - pad
    val range = ((hi + pad) - minV).coerceAtLeast(1.0)
    Canvas(modifier = Modifier.fillMaxWidth().height(64.dp)) {
        // Inset all four edges so the end markers + the rounded stroke sit clear of the canvas bounds.
        val hInset = 10.dp.toPx()
        val vInset = 9.dp.toPx()
        val plotW = (size.width - hInset * 2).coerceAtLeast(1f)
        val plotH = (size.height - vInset * 2).coerceAtLeast(1f)
        val stepX = if (values.size > 1) plotW / (values.size - 1) else 0f
        fun yOf(v: Double): Float {
            val t = ((v - minV) / range).toFloat().coerceIn(0f, 1f)
            return vInset + (1f - t) * plotH
        }
        val pts = values.mapIndexed { i, v -> Offset(hInset + stepX * i, yOf(v)) }
        val baseline = size.height - vInset
        // Curve control points are clamped to the plot box so the spline can't bow past an end point.
        val line = smoothCurve(pts, minY = vInset, maxY = baseline)
        val fill = Path().apply {
            addPath(line)
            lineTo(pts.last().x, baseline)
            lineTo(pts.first().x, baseline)
            close()
        }
        val clip = (size.width * progress.coerceIn(0f, 1f)).coerceAtLeast(0.01f)
        clipRect(right = clip) {
            // §10: area fades to transparent from a ≤0.15 top stop.
            drawPath(fill, brush = Brush.verticalGradient(listOf(accent.copy(alpha = 0.15f), accent.copy(alpha = 0f))))
            drawPath(line, color = accent.copy(alpha = 0.15f), style = Stroke(width = 7.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
            drawPath(line, color = accent, style = Stroke(width = 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
            pts.forEachIndexed { i, p ->
                val last = i == pts.lastIndex
                val r = if (last) 4.dp.toPx() else 3.dp.toPx()
                drawCircle(color = haloBg, radius = r + 2.dp.toPx(), center = p)        // ring cuts the dot out
                drawCircle(color = accent, radius = r, center = p)                       // accent core
                if (last) drawCircle(color = haloBg, radius = 1.5.dp.toPx(), center = p) // hollow centre = latest set
            }
        }
    }
}

/**
 * A Catmull-Rom spline through [pts] as one cubic-bezier [Path]. Control-point Y is clamped to
 * [[minY], [maxY]] so the curve stays inside the plot box (an overshoot would clip a marker). Falls
 * back to a straight move/line for fewer than three points.
 */
private fun smoothCurve(pts: List<Offset>, minY: Float, maxY: Float): Path {
    val path = Path()
    if (pts.isEmpty()) return path
    path.moveTo(pts.first().x, pts.first().y)
    if (pts.size < 3) {
        for (i in 1 until pts.size) path.lineTo(pts[i].x, pts[i].y)
        return path
    }
    for (i in 0 until pts.size - 1) {
        val p0 = pts[if (i == 0) 0 else i - 1]
        val p1 = pts[i]
        val p2 = pts[i + 1]
        val p3 = pts[if (i + 2 <= pts.lastIndex) i + 2 else pts.lastIndex]
        val c1x = p1.x + (p2.x - p0.x) / 6f
        val c1y = (p1.y + (p2.y - p0.y) / 6f).coerceIn(minY, maxY)
        val c2x = p2.x - (p3.x - p1.x) / 6f
        val c2y = (p2.y - (p3.y - p1.y) / 6f).coerceIn(minY, maxY)
        path.cubicTo(c1x, c1y, c2x, c2y, p2.x, p2.y)
    }
    return path
}

/** One centred dot — the Line-mode stand-in for an exercise with a single logged set. */
@Composable
private fun SinglePointMark(accent: Color) {
    Box(modifier = Modifier.fillMaxWidth().height(64.dp), contentAlignment = Alignment.Center) {
        Box(modifier = Modifier.size(8.dp).clip(RoundedCornerShape(50)).background(accent))
    }
}

/** Vertical per-set bars, growing from the baseline with a staggered reveal; last (latest) set bold. */
@Composable
private fun SetBars(values: List<Double>, metric: SessionMetric, accent: Color) {
    val max = (values.maxOrNull() ?: 0.0).coerceAtLeast(1.0)
    // §5: earlier sets take the accent-0.6 rung (secondary), the latest set full accent.
    val secondary = MaterialTheme.colorScheme.secondary
    // Keyed by metric so the first draw animates in. The play-once motion kit doesn't replay on a
    // later metric switch, so BARS (like the LINE branch) just snap to the new values — matching Stats.
    // The slow draw curve grows the bars in gently rather than snapping them to height.
    val progress = rememberDrawProgress(metric, ForgeMotion.drawTween())
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
                    .background(if (i == values.lastIndex) accent else secondary)
            )
        }
    }
}

/**
 * The watch's heart rate over the session (W3): an open line (stroke `primary`, §10) with the
 * logged sets as on-line accent dots and exercise boundaries as vertical data hairlines. Header
 * meta carries AVG · MAX; the per-exercise readings + HRR line render beneath as §4.9 rows.
 */
@Composable
internal fun SessionHrSection(
    hr: com.forge.app.domain.health.SessionHrView,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    androidx.compose.foundation.layout.Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            com.forge.app.ui.common.EditorialHeader(label = "Heart rate", muted = muted, accent = accent)
            Text(
                "AVG ${hr.avgBpm} · MAX ${hr.maxBpm} BPM",
                style = MaterialTheme.typography.labelSmall,
                color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp
            )
        }
        androidx.compose.foundation.layout.Spacer(Modifier.height(10.dp))
        val progress = rememberDrawProgress()
        Canvas(Modifier.fillMaxWidth().height(110.dp)) {
            val pts = hr.points
            if (pts.size < 2) return@Canvas
            val t0 = pts.first().timeMs
            val t1 = pts.last().timeMs
            val span = (t1 - t0).coerceAtLeast(1L).toFloat()
            val minBpm = pts.minOf { it.bpm }.toFloat()
            val maxBpm = pts.maxOf { it.bpm }.toFloat()
            val range = (maxBpm - minBpm).coerceAtLeast(1f)
            fun x(ms: Long) = ((ms - t0) / span) * size.width
            fun y(bpm: Int) = size.height - ((bpm - minBpm) / range) * (size.height * 0.9f) - size.height * 0.05f

            // Exercise boundaries — lines as data (§1).
            hr.exerciseBoundariesMs.forEach { ms ->
                if (ms in t0..t1) drawLine(
                    color = outline.copy(alpha = 0.25f),
                    start = Offset(x(ms), 0f), end = Offset(x(ms), size.height),
                    strokeWidth = 1.dp.toPx()
                )
            }
            clipRect(right = size.width * progress) {
                val path = Path()
                pts.forEachIndexed { i, p ->
                    if (i == 0) path.moveTo(x(p.timeMs), y(p.bpm)) else path.lineTo(x(p.timeMs), y(p.bpm))
                }
                drawPath(path, color = accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
                // Set markers ride the line: the moment each set was logged.
                hr.setMarkersMs.forEach { ms ->
                    if (ms in t0..t1) {
                        val nearest = pts.minByOrNull { kotlin.math.abs(it.timeMs - ms) } ?: return@forEach
                        drawCircle(color = accent, radius = 2.5.dp.toPx(), center = Offset(x(ms), y(nearest.bpm)))
                    }
                }
            }
        }
        androidx.compose.foundation.layout.Spacer(Modifier.height(8.dp))
        hr.perExercise.forEach { ex ->
            Row(
                Modifier.fillMaxWidth().padding(vertical = 3.dp),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(ex.name, style = MaterialTheme.typography.bodySmall, color = muted)
                Text(
                    "AVG ${ex.avgBpm}",
                    style = MaterialTheme.typography.labelSmall,
                    color = onBg, fontSize = 9.sp, letterSpacing = 0.5.sp
                )
            }
        }
        hr.avgHrr60?.let { drop ->
            androidx.compose.foundation.layout.Spacer(Modifier.height(4.dp))
            Text(
                "Recovery · you shed $drop bpm in the first minute of rest",
                style = MaterialTheme.typography.bodySmall, color = muted
            )
        }
    }
}
