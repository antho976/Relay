package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import com.forge.app.program.MuscleGroup
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Muscles whose highlight reads best from the back-view figure; everything else uses the front. */
private val BACK_VIEW_MUSCLES = setOf(
    MuscleGroup.BACK, MuscleGroup.REAR_DELTS, MuscleGroup.GLUTES, MuscleGroup.HAMSTRINGS
)

/** True when [this] muscle lights up on the back figure (lats, rear delts, glutes, hamstrings). */
internal fun MuscleGroup.usesBackView(): Boolean = this in BACK_VIEW_MUSCLES

/** Leg groups frame the lower body; everything else frames head-to-hips (a torso portrait). */
private val LOWER_BODY_MUSCLES = setOf(
    MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.CALVES
)

/** A viewBox sub-rectangle (fractions of width/height) to frame the figure to. */
private class CropWindow(val left: Float, val top: Float, val right: Float, val bottom: Float)

/**
 * The viewBox slice framed for [this] muscle. Cropping to the muscle's band makes the highlight read
 * large instead of a spindly whole body. Legs ALSO crop horizontally to the central body: the arms
 * hang at the sides down to the hips, so a legs-only vertical crop would leave them flying off the
 * sides — the horizontal crop clips the arms while keeping the (central) legs and glutes.
 */
private fun MuscleGroup.cropWindow(): CropWindow =
    if (this in LOWER_BODY_MUSCLES) CropWindow(left = 0.28f, top = 0.46f, right = 0.72f, bottom = 0.94f)
    else CropWindow(left = 0f, top = 0.06f, right = 1f, bottom = 0.42f)

private fun parsePath(d: String): Path = PathParser().parsePathString(d).toPath()

/** Every muscle region (tagged with its group) + the body contour, pre-compiled off the main thread. */
private class FigureGeometry(val regions: List<Pair<MuscleGroup?, Path>>, val outline: Path)

/**
 * Compiled geometry is IDENTICAL for every figure of the same orientation (all front figures share
 * ANATOMY_FRONT, all back figures share ANATOMY_BACK), so it's parsed at most once per orientation for
 * the whole process — a grid of tiles no longer re-parses the multi-KB path set per instance. Keyed by
 * `back`; the "All" full-body figure shares the front entry.
 */
private val geometryCache = java.util.concurrent.ConcurrentHashMap<Boolean, FigureGeometry>()

private fun buildGeometry(back: Boolean): FigureGeometry {
    val parts = if (back) ANATOMY_BACK else ANATOMY_FRONT
    val outlineStr = if (back) ANATOMY_BACK_OUTLINE else ANATOMY_FRONT_OUTLINE
    return FigureGeometry(
        regions = parts.flatMap { part -> part.paths.map { part.group to parsePath(it) } },
        outline = parsePath(outlineStr)
    )
}

/**
 * A single anatomy figure (front or back) drawn small: the whole musculature rendered as faint
 * [detail] relief over the [body] silhouette, with the target [muscle] lit in [lit] on top. Drawing
 * every region — not just the target — is what makes a back view read as a back (you see lats, traps
 * and rear delts, not a flat blob identical to the front). Reuses the Stats [AnatomyPart] geometry
 * (see [BodyHeatmap]); no contour stroke (an outline reads as a wireframe at thumbnail size).
 * Compiled off the composition thread.
 */
@Composable
internal fun MuscleFigure(
    muscle: MuscleGroup,
    lit: Color,
    body: Color,
    detail: Color,
    modifier: Modifier = Modifier
) {
    val back = muscle.usesBackView()
    val originX = if (back) ANATOMY_BACK_ORIGIN_X else 0f

    // Same off-thread compile guard as BodyHeatmap — the outline d-string is kilobytes; parsing it on
    // the main thread janked the first frame. A process cache keyed by orientation means the parse runs
    // once per front/back, not once per tile — a warm cache paints on the first frame with no dispatch.
    val geometry by produceState<FigureGeometry?>(initialValue = geometryCache[back], back) {
        geometryCache[back]?.let { value = it; return@produceState }
        value = withContext(Dispatchers.Default) { geometryCache.getOrPut(back) { buildGeometry(back) } }
    }

    val cw = muscle.cropWindow()
    // clipToBounds so the cropped-away body can't spill onto neighbours.
    Canvas(modifier.clipToBounds()) {
        val g = geometry ?: return@Canvas
        // Fit the muscle's viewBox sub-rectangle to the canvas.
        val x0 = cw.left * ANATOMY_VB_W
        val y0 = cw.top * ANATOMY_VB_H
        val winW = (cw.right - cw.left) * ANATOMY_VB_W
        val winH = (cw.bottom - cw.top) * ANATOMY_VB_H
        val s = minOf(size.width / winW, size.height / winH)
        val dx = (size.width - winW * s) / 2f - x0 * s
        val dy = (size.height - winH * s) / 2f - y0 * s
        withTransform({
            translate(dx, dy)
            scale(s, s, pivot = Offset.Zero)
            translate(-originX, 0f) // back-view coords live in x∈[724,1448]; shift into view
        }) {
            drawPath(g.outline, body)                       // dim whole-body silhouette base
            g.regions.forEach { (group, path) ->            // muscle relief; the target lights up
                drawPath(path, if (group == muscle) lit else detail)
            }
        }
    }
}

/**
 * The whole front figure with every muscle drawn in [detail] and none singled out — the "All" filter's
 * glyph (the anatomy equivalent of "everything"). Uncropped, so it reads as a full person next to the
 * cropped body-part figures.
 */
@Composable
internal fun FullBodyFigure(
    body: Color,
    detail: Color,
    modifier: Modifier = Modifier
) {
    // Shares the front-orientation entry with every front MuscleFigure (identical geometry).
    val geometry by produceState<FigureGeometry?>(initialValue = geometryCache[false]) {
        if (value == null) value = withContext(Dispatchers.Default) { geometryCache.getOrPut(false) { buildGeometry(false) } }
    }
    Canvas(modifier.clipToBounds()) {
        val g = geometry ?: return@Canvas
        val s = minOf(size.width / ANATOMY_VB_W, size.height / ANATOMY_VB_H)
        val dx = (size.width - ANATOMY_VB_W * s) / 2f
        val dy = (size.height - ANATOMY_VB_H * s) / 2f
        withTransform({
            translate(dx, dy)
            scale(s, s, pivot = Offset.Zero)
        }) {
            drawPath(g.outline, body)
            g.regions.forEach { (_, path) -> drawPath(path, detail) }
        }
    }
}
