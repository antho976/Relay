package com.forge.app.domain.cardio

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.DirectionsRun
import androidx.compose.material.icons.filled.NordicWalking
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.PathNode
import androidx.compose.ui.graphics.vector.VectorGroup
import androidx.compose.ui.graphics.vector.VectorPath
import androidx.compose.ui.graphics.vector.group
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp
import kotlin.math.sqrt

/**
 * Custom cardio glyphs for the activity types where Material's stock icons read
 * poorly or don't exist:
 *  - [RowingMachine] — Material's `Rowing` person has oars too thin to read, so
 *    we draw the gym erg (fan + monorail + seat). No human figure to clash with
 *    the Material glyphs the rest of [CardioType] uses.
 *  - [Walk] — Material's `DirectionsWalk` reads as a crouched sneak; `NordicWalking`
 *    has the right upright stride but draws two walking poles. We reuse its figure
 *    and strip just the pole sub-paths, leaving a clean walker.
 *  - [Treadmill] — no Material glyph exists. Rather than hand-draw a runner
 *    (which never matches Material's figure style), we embed Material's real
 *    `DirectionsRun` figure scaled into the top and hand-draw only the belt +
 *    console beneath it.
 *
 * Both are 24dp filled silhouettes at the same visual weight as the
 * `Icons.Filled.*` glyphs. The source fill is black; callers render them through
 * `Icon(..., tint = ...)`, which recolors the whole vector, exactly like the
 * Material icons.
 */
object CardioIcons {

    val RowingMachine: ImageVector by lazy {
        cardioIcon("CardioRowingMachine") {
            // Flywheel cage (ring) up front-left.
            fillPath(PathFillType.EvenOdd) {
                circle(6.5f, 8f, 4.2f)
                circle(6.5f, 8f, 1.7f)
            }
            fillPath {
                circle(6.5f, 8f, 0.8f)              // hub
                // Front A-frame leg down to the floor.
                capsule(6.5f, 11.6f, 3.7f, 18.8f, 0.95f)
                foot(3.7f, 18.9f)
                // Monorail sloping down toward the back-right.
                capsule(7.6f, 14.2f, 21f, 17.6f, 0.95f)
                // Seat riding on the rail.
                capsule(11.6f, 13f, 14.6f, 13f, 1.1f)     // seat pad
                capsule(13.1f, 13.8f, 13.1f, 15.4f, 0.5f) // seat post onto rail
                // Rear leg under the rail end.
                capsule(21f, 17.4f, 21f, 19.2f, 0.95f)
                foot(21f, 19.3f)
            }
        }
    }

    val Walk: ImageVector by lazy {
        cardioIcon("CardioWalk") {
            // NordicWalking's single path is 4 sub-paths: 0 = right pole, 1 = left
            // pole, 2 = head, 3 = body+limbs. Keep the head and body, drop the poles.
            addFilteredPaths(Icons.Filled.NordicWalking, keepSubPaths = setOf(2, 3))
        }
    }

    val Treadmill: ImageVector by lazy {
        cardioIcon("CardioTreadmill") {
            // Material's running figure, shrunk into the upper area so a belt fits beneath.
            addPathsFrom(
                Icons.AutoMirrored.Filled.DirectionsRun,
                scaleX = 0.72f,
                scaleY = 0.72f,
                translationX = 0.4f,
                translationY = 2.4f,
            )
            // ---- Treadmill base under the runner ----
            fillPath {
                roundRect(3f, 18.7f, 16.4f, 20.5f, 0.9f) // belt deck
                // Upright console + handlebar at the front.
                capsule(16.4f, 10.6f, 16.4f, 19.6f, 0.85f) // console post
                capsule(13.8f, 10.8f, 17.4f, 10.8f, 0.85f) // handlebar
            }
        }
    }
}

// --- builders --------------------------------------------------------------

private fun cardioIcon(name: String, block: ImageVector.Builder.() -> Unit): ImageVector =
    ImageVector.Builder(
        name = name,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply(block).build()

private fun ImageVector.Builder.fillPath(
    fillType: PathFillType = PathFillType.NonZero,
    block: PathBuilder.() -> Unit,
) {
    path(fill = SolidColor(Color.Black), pathFillType = fillType, pathBuilder = block)
}

/**
 * Copies every path of [source] into this builder, wrapped in a group that
 * scales/translates it. Used to embed a real Material glyph (so its figure style
 * matches the other cardio icons) alongside hand-drawn geometry.
 */
private fun ImageVector.Builder.addPathsFrom(
    source: ImageVector,
    scaleX: Float = 1f,
    scaleY: Float = 1f,
    translationX: Float = 0f,
    translationY: Float = 0f,
) {
    group(
        scaleX = scaleX,
        scaleY = scaleY,
        translationX = translationX,
        translationY = translationY,
    ) {
        source.root.collectPaths().forEach { vp ->
            addPath(
                pathData = vp.pathData,
                pathFillType = vp.pathFillType,
                name = vp.name,
                fill = SolidColor(Color.Black),
                fillAlpha = vp.fillAlpha,
                stroke = null,
                strokeAlpha = vp.strokeAlpha,
                strokeLineWidth = vp.strokeLineWidth,
                strokeLineCap = vp.strokeLineCap,
                strokeLineJoin = vp.strokeLineJoin,
                strokeLineMiter = vp.strokeLineMiter,
            )
        }
    }
}

/**
 * Copies [source]'s paths into this builder, keeping only the sub-paths whose
 * index (counted by `moveTo` boundaries within each path) is in [keepSubPaths].
 * Lets us reuse a Material figure while dropping an unwanted element (e.g. the
 * walking poles on `NordicWalking`).
 */
private fun ImageVector.Builder.addFilteredPaths(
    source: ImageVector,
    keepSubPaths: Set<Int>,
) {
    source.root.collectPaths().forEach { vp ->
        val kept = mutableListOf<PathNode>()
        var subPath = -1
        vp.pathData.forEach { node ->
            if (node is PathNode.MoveTo || node is PathNode.RelativeMoveTo) subPath++
            if (subPath in keepSubPaths) kept.add(node)
        }
        if (kept.isEmpty()) return@forEach
        addPath(
            pathData = kept,
            pathFillType = vp.pathFillType,
            name = vp.name,
            fill = SolidColor(Color.Black),
            fillAlpha = vp.fillAlpha,
            stroke = null,
            strokeAlpha = vp.strokeAlpha,
            strokeLineWidth = vp.strokeLineWidth,
            strokeLineCap = vp.strokeLineCap,
            strokeLineJoin = vp.strokeLineJoin,
            strokeLineMiter = vp.strokeLineMiter,
        )
    }
}

private fun VectorGroup.collectPaths(): List<VectorPath> = buildList {
    this@collectPaths.forEach { node ->
        when (node) {
            is VectorPath -> add(node)
            is VectorGroup -> addAll(node.collectPaths())
        }
    }
}

/** Full circle centered at ([cx], [cy]) with radius [r]. */
private fun PathBuilder.circle(cx: Float, cy: Float, r: Float) {
    moveTo(cx - r, cy)
    arcToRelative(r, r, 0f, true, true, 2 * r, 0f)
    arcToRelative(r, r, 0f, true, true, -2 * r, 0f)
    close()
}

/** A thick line segment with rounded ends — a limb or bar. */
private fun PathBuilder.capsule(x1: Float, y1: Float, x2: Float, y2: Float, r: Float) {
    val dx = x2 - x1
    val dy = y2 - y1
    val len = sqrt(dx * dx + dy * dy).coerceAtLeast(0.0001f)
    val px = -dy / len * r
    val py = dx / len * r
    moveTo(x1 + px, y1 + py)
    lineTo(x2 + px, y2 + py)
    lineTo(x2 - px, y2 - py)
    lineTo(x1 - px, y1 - py)
    close()
    circle(x1, y1, r)
    circle(x2, y2, r)
}

/** A small grounded foot pad centered at ([cx], [cy]). */
private fun PathBuilder.foot(cx: Float, cy: Float) {
    capsule(cx - 0.9f, cy, cx + 0.9f, cy, 0.6f)
}

/** Axis-aligned rounded rectangle from ([l], [t]) to ([r], [b]). */
private fun PathBuilder.roundRect(l: Float, t: Float, r: Float, b: Float, radius: Float) {
    val cy = (t + b) / 2f
    capsule(l + radius, cy, r - radius, cy, (b - t) / 2f)
}
