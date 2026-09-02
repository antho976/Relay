package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.MuscleGroup
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private fun parse(d: String): Path = PathParser().parsePathString(d).toPath()

/** An [AnatomyPart] with its SVG strings pre-compiled to [Path]s for drawing. */
private class CompiledPart(val group: MuscleGroup?, val paths: List<Path>)

/** A whole figure (regions + body outline) compiled off the main thread. */
private class CompiledFigure(val parts: List<CompiledPart>, val outline: Path)

/**
 * One of the two anatomical figures, with its geometry compiled ONCE PER PROCESS (P-10).
 *
 * It used to be compiled inside each [FigureColumn]'s own `produceState`, which is
 * composition-local however stable its keys are — so Stats, the session summary, the session
 * detail and the enlarged map each reparsed both figures' 155 path strings and the two large
 * outlines, allocated a fresh set of `Path`s, and drew blank until they landed. The comment above
 * that call claimed process-wide reuse; this is what actually provides it.
 *
 * `by lazy` is synchronized, so two surfaces opening at once compile a side once between them, and
 * the caller still forces it from a background dispatcher — the first compile must not land on the
 * main thread, and every one after it is a field read.
 */
private enum class AnatomySide(
    val title: String,
    private val parts: List<AnatomyPart>,
    private val outline: String,
    /** The back view's coords live in x∈[724,1448]; the draw shifts them into view. */
    val originX: Float
) {
    FRONT("FRONT", ANATOMY_FRONT, ANATOMY_FRONT_OUTLINE, 0f),
    BACK("BACK", ANATOMY_BACK, ANATOMY_BACK_OUTLINE, ANATOMY_BACK_ORIGIN_X);

    val compiled: CompiledFigure by lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        CompiledFigure(parts.map { CompiledPart(it.group, it.paths.map(::parse)) }, parse(outline))
    }
}

/**
 * Two anatomical figures (front + back), each muscle region tinted by its set count — faint =
 * neglected, bold accent = most-trained. The geometry is real anatomical vector art (adapted from
 * react-native-body-highlighter, MIT — see [AnatomyPart]); the whole body is drawn faint and the
 * trained muscles light up. Intensity is relative to the busiest muscle in the data set.
 */
@Composable
internal fun BodyHeatmap(
    setsByMuscle: Map<MuscleGroup, Int>,
    accent: Color,
    faint: Color,
    silhouette: Color,
    labelColor: Color,
    modifier: Modifier = Modifier,
    figureHeight: Dp = 300.dp,
    showLegend: Boolean = true,
    showTitles: Boolean = true
) {
    val maxSets = (setsByMuscle.values.maxOrNull() ?: 0).coerceAtLeast(1)
    Column(modifier) {
        Row(Modifier.fillMaxWidth()) {
            FigureColumn(AnatomySide.FRONT, setsByMuscle, maxSets, accent, faint, silhouette, labelColor, figureHeight, showTitles, Modifier.weight(1f))
            FigureColumn(AnatomySide.BACK, setsByMuscle, maxSets, accent, faint, silhouette, labelColor, figureHeight, showTitles, Modifier.weight(1f))
        }
        if (showLegend) {
            Spacer(Modifier.height(10.dp))
            HeatLegend(accent = accent, faint = faint, labelColor = labelColor)
        }
    }
}

@Composable
private fun FigureColumn(
    side: AnatomySide,
    setsByMuscle: Map<MuscleGroup, Int>,
    maxSets: Int,
    accent: Color,
    faint: Color,
    silhouette: Color,
    labelColor: Color,
    figureHeight: Dp,
    showTitle: Boolean,
    modifier: Modifier = Modifier
) {
    // Force the compile OFF the composition thread — the outline strings are kilobytes and parsing
    // them on the main thread janked the first Stats/summary frame. The geometry itself is cached
    // on [AnatomySide], so this is a real compile only the first time either figure is shown in the
    // process and a field read on every surface after that; until it lands the canvas draws nothing
    // for a frame rather than blocking.
    val compiled by produceState<CompiledFigure?>(initialValue = null, side) {
        value = withContext(Dispatchers.Default) { side.compiled }
    }
    val edge = silhouette.copy(alpha = (silhouette.alpha * 2.5f).coerceAtMost(0.6f))

    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        if (showTitle) {
            Text(side.title, style = MaterialTheme.typography.labelSmall, color = labelColor)
            Spacer(Modifier.height(6.dp))
        }
        Canvas(Modifier.fillMaxWidth().height(figureHeight)) {
            val figure = compiled ?: return@Canvas
            // Uniform scale (preserve proportions) + centre the viewBox in the canvas.
            val s = minOf(size.width / ANATOMY_VB_W, size.height / ANATOMY_VB_H)
            val dx = (size.width - ANATOMY_VB_W * s) / 2f
            val dy = (size.height - ANATOMY_VB_H * s) / 2f
            withTransform({
                translate(dx, dy)
                scale(s, s, pivot = Offset.Zero)
                translate(-side.originX, 0f) // back-view coords live in x∈[724,1448]; shift into view
            }) {
                // Solid faint body behind everything (guarantees no gaps between regions).
                drawPath(figure.outline, silhouette)
                // Every region faint; trained muscles light up toward the accent.
                figure.parts.forEach { p ->
                    val sets = if (p.group != null) setsByMuscle[p.group] ?: 0 else 0
                    val color = if (p.group != null && sets > 0)
                        lerp(faint, accent, (sets.toFloat() / maxSets).coerceIn(0f, 1f)) else faint
                    p.paths.forEach { drawPath(it, color) }
                }
                // Crisp body contour on top.
                drawPath(figure.outline, edge, style = Stroke(width = 2.5f))
            }
        }
    }
}

@Composable
private fun HeatLegend(accent: Color, faint: Color, labelColor: Color) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text("less", style = MaterialTheme.typography.labelSmall, color = labelColor)
        Canvas(
            Modifier
                .padding(horizontal = 8.dp)
                .weight(1f)
                .height(8.dp)
                .clip(androidx.compose.foundation.shape.RoundedCornerShape(4.dp))
        ) {
            val steps = 24
            val stepW = size.width / steps
            for (i in 0 until steps) {
                drawRect(
                    color = lerp(faint, accent, i / (steps - 1f)),
                    topLeft = Offset(i * stepW, 0f),
                    size = Size(stepW + 1f, size.height)
                )
            }
        }
        Text("more sets", style = MaterialTheme.typography.labelSmall, color = labelColor, textAlign = TextAlign.End)
    }
}
