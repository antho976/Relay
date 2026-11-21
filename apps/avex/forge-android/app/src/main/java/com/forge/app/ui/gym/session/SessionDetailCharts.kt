package com.forge.app.ui.gym.session

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.rememberDrawProgressState
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.theme.ForgeMotion

/**
 * The opened lift's per-set chart drawn as a line (the section's "Line" pick): a smooth red stroke
 * over a fading red area with a ring-cut dot on every set, the latest hollow. Inset on every edge so
 * the end dots never clip, and revealed left to right on the slow draw curve.
 */
@Composable
internal fun PerSetLine(values: List<Double>, metric: SessionMetric, description: String) {
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background
    val progress = rememberDrawProgressState(metric, ForgeMotion.drawTween())
    val lo = values.min()
    val hi = values.max()
    val pad = if (hi - lo < 1e-6) 1.0 else 0.0
    val minV = lo - pad
    val range = ((hi + pad) - minV).coerceAtLeast(1e-6)
    Spacer(Modifier.fillMaxWidth().height(96.dp).clearAndSetSemantics { contentDescription = description }.drawWithCache {
        val hInset = 10.dp.toPx()
        val vInset = 9.dp.toPx()
        val plotW = (size.width - hInset * 2).coerceAtLeast(1f)
        val plotH = (size.height - vInset * 2).coerceAtLeast(1f)
        val stepX = if (values.size > 1) plotW / (values.size - 1) else 0f
        fun yOf(v: Double): Float = vInset + (1f - ((v - minV) / range).toFloat().coerceIn(0f, 1f)) * plotH
        val pts = values.mapIndexed { i, v -> Offset(hInset + stepX * i, yOf(v)) }
        val baseline = size.height - vInset
        val line = smoothCurve(pts, minY = vInset, maxY = baseline)
        val fill = Path().apply {
            addPath(line)
            lineTo(pts.last().x, baseline)
            lineTo(pts.first().x, baseline)
            close()
        }
        val fillBrush = Brush.verticalGradient(listOf(accent.copy(alpha = 0.15f), accent.copy(alpha = 0f)))
        val lineStroke = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
        onDrawBehind {
            val clip = (size.width * progress.value.coerceIn(0f, 1f)).coerceAtLeast(0.01f)
            clipRect(right = clip) {
                drawPath(fill, brush = fillBrush)
                drawPath(line, color = accent, style = lineStroke)
                pts.forEachIndexed { i, p ->
                    val last = i == pts.lastIndex
                    val r = if (last) 4.dp.toPx() else 3.dp.toPx()
                    drawCircle(color = ground, radius = r + 2.dp.toPx(), center = p)
                    drawCircle(color = accent, radius = r, center = p)
                    if (last) drawCircle(color = ground, radius = 1.5.dp.toPx(), center = p)
                }
            }
        }
    })
}

/**
 * A Catmull-Rom spline through [pts] as one cubic-bezier [Path], its control points clamped to
 * [[minY], [maxY]] so the curve never bows out of the plot. Straight segments under three points.
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
