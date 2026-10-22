package com.forge.app.ui.gym.stats

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType

/** The shape a [StatsPreview] sketches: the chart that will stand in its place. */
internal enum class PreviewShape { LINES, EFFORT, WEEKS, BARS }

// Fixed sample heights (0..1), so the sketch is the same every time and reads as a sketch, not data.
private val SAMPLE_COLUMNS = listOf(0.35f, 0.75f, 0.6f, 0.9f, 0.2f)
private val SAMPLE_WEEKS = listOf(0.5f, 0.55f, 0.48f, 0.62f, 0.66f, 0.58f, 0.72f, 0.7f, 0.78f, 0.74f, 0.82f, 0.4f)
private val SAMPLE_BARS = listOf(0.9f, 0.75f, 0.68f, 0.55f, 0.4f)
private val SAMPLE_RISE = listOf(0.25f, 0.32f, 0.3f, 0.45f, 0.5f, 0.62f, 0.6f, 0.74f)
private val SAMPLE_FALL = listOf(0.55f, 0.42f, 0.5f, 0.38f, 0.46f, 0.4f, 0.52f, 0.44f)

/**
 * A section with too little to read yet. Text alone on an empty tab read as broken (Antho,
 * 2026-10-21), so it sketches the chart that will land here in the quiet track colour, says how
 * close it is when there is a count to reach ("2 of 5 training days" over a meter), and what fills it.
 */
@Composable
internal fun StatsPreview(shape: PreviewShape, text: String, progress: Pair<Int, Int>? = null, unitWord: String = "") {
    val track = OpenColors.Track
    val quiet = OpenColors.BarQuiet
    Column {
        Canvas(Modifier.fillMaxWidth().height(if (shape == PreviewShape.BARS) 92.dp else 84.dp).clearAndSetSemantics { }) {
            val r = CornerRadius(1.dp.toPx())
            when (shape) {
                PreviewShape.EFFORT, PreviewShape.WEEKS -> {
                    val values = if (shape == PreviewShape.WEEKS) SAMPLE_WEEKS else SAMPLE_COLUMNS
                    val gap = 6.dp.toPx()
                    val w = (size.width - gap * (values.size - 1)) / values.size
                    values.forEachIndexed { i, v ->
                        val h = v * size.height
                        drawRoundRect(track, topLeft = Offset(i * (w + gap), size.height - h), size = Size(w, h), cornerRadius = r)
                    }
                }
                PreviewShape.BARS -> {
                    val gap = 8.dp.toPx()
                    val h = (size.height - gap * (SAMPLE_BARS.size - 1)) / SAMPLE_BARS.size
                    SAMPLE_BARS.forEachIndexed { i, v ->
                        drawRoundRect(track, topLeft = Offset(0f, i * (h + gap)), size = Size(v * size.width, h), cornerRadius = r)
                    }
                }
                PreviewShape.LINES -> {
                    fun line(values: List<Float>, color: androidx.compose.ui.graphics.Color) {
                        val step = size.width / (values.size - 1)
                        val path = Path()
                        values.forEachIndexed { i, v ->
                            val x = i * step
                            val y = size.height * (1f - v)
                            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
                        }
                        drawPath(path, color, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round))
                    }
                    line(SAMPLE_FALL, quiet)
                    line(SAMPLE_RISE, track)
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        if (progress != null) {
            val (have, need) = progress
            OpenMeter(have.coerceAtMost(need).toFloat() / need.coerceAtLeast(1))
            Spacer(Modifier.height(8.dp))
            OpenLabel("${have.coerceAtMost(need)} of $need $unitWord", style = OpenType.monoSTight, color = MaterialTheme.colorScheme.onBackground)
            Spacer(Modifier.height(10.dp))
        }
        Text(text, style = OpenType.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
