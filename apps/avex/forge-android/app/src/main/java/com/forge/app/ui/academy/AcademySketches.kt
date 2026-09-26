package com.forge.app.ui.academy

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.asComposePath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.academy.AcademyCardio
import com.forge.app.domain.academy.AcademyCoachLessons
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.AcademyTraining
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * # The Academy's drawings (2026-09-26)
 *
 * Every lesson is taught by one diagram that draws itself: the double-progression staircase built
 * from real sets, rep speed slowing into reserve, the volume curve against its fatigue cost. They
 * replaced 35 photographic covers that set a mood and taught nothing. A reader should get the idea
 * of a lesson from its drawing before reading a word of it.
 *
 * ## One grammar
 *
 * Every drawing is laid out in the same 160 by 110 design box, in the vocabulary of a good printed
 * explainer graphic rather than a dashboard chart:
 *
 *  - **guide**: axes, ticks, dotted gridlines and leader lines, thin and muted;
 *  - **ink**: the thing being described, in the page's foreground, with a soft wash beneath it;
 *  - **accent**: the ONE element that carries the answer (where to stop, the sweet spot, today),
 *    finished with a halo that pulses once when it lands;
 *  - **one serif figure** per drawing where the lesson has a number worth remembering (1.6, 2, 14),
 *    counting up as it appears; everything else is the mono caption voice.
 *
 * Label sizes are set in design units, not sp, so a diagram scales as one picture: at a large font
 * setting its words would otherwise outgrow the geometry they annotate. The lesson prose around it
 * carries the same facts at the reader's own text size, and the drawing's `contentDescription`
 * states them for TalkBack.
 *
 * ## How it draws
 *
 * Each drawing is a choreography ([Choreo]): every mark has its own window on one clock, so axes
 * and grid arrive first, then the data line is inked by a travelling pen tip while its wash sweeps
 * in behind it, then the accent lands and pulses, and the words fade up last. About three seconds
 * end to end. Thumbnails and reduced motion show the finished frame.
 */
class Sketch(
    val marks: List<Timed>,
    /** What the drawing SAYS, for TalkBack (§14). Never a description of its shape. */
    val description: String
) {
    private val centers = HashMap<Boolean, Offset>()

    /**
     * The middle of what this drawing actually puts on the page, in design units.
     *
     * Drawings are laid out by hand in the design box, and a long label on one side or an axis
     * starting at x 40 left several of them sitting visibly off-centre in the frame (Antho, on the
     * rotating block). Centring the measured content rather than the box fixes every drawing at
     * once, and keeps fixing the next one somebody draws.
     */
    internal fun center(withLabels: Boolean): Offset = centers.getOrPut(withLabels) {
        contentBounds(marks.map { it.mark }, withLabels).center
    }
}

/** What a set of marks covers, labels estimated from their length in the mono caption voice. */
private fun contentBounds(marks: List<SketchMark>, withLabels: Boolean): Rect {
    var l = Float.MAX_VALUE
    var t = Float.MAX_VALUE
    var r = -Float.MAX_VALUE
    var b = -Float.MAX_VALUE
    fun add(rect: Rect) {
        l = minOf(l, rect.left); t = minOf(t, rect.top); r = maxOf(r, rect.right); b = maxOf(b, rect.bottom)
    }
    marks.forEach { m ->
        when (m) {
            is SketchMark.Line -> add(m.path.getBounds())
            is SketchMark.Area -> add(m.path.getBounds())
            is SketchMark.Band -> add(Rect(m.l, m.t, m.r, m.b))
            is SketchMark.Column -> add(Rect(m.x - m.width / 2f, m.top, m.x + m.width / 2f, m.base))
            is SketchMark.Dot -> {
                val reach = if (m.halo) m.r * 2.6f else m.r
                add(Rect(m.x - reach, m.y - reach, m.x + reach, m.y + reach))
            }
            is SketchMark.Label -> if (withLabels) {
                val units = if (m.figure) FIGURE_UNITS else LABEL_UNITS
                val w = m.text.length * units * (if (m.figure) 0.55f else 0.7f)
                val left = when (m.anchor) {
                    Anchor.START -> m.x
                    Anchor.CENTER -> m.x - w / 2f
                    Anchor.END -> m.x - w
                }
                add(Rect(left, m.y - units / 2f, left + w, m.y + units / 2f))
            }
        }
    }
    return if (l > r) Rect(0f, 0f, BOX_W, BOX_H) else Rect(l, t, r, b)
}

/** A mark and its window on the drawing's clock, both ends in 0..1. */
class Timed(val mark: SketchMark, val t0: Float, val t1: Float)

enum class Ink { GUIDE, INK, ACCENT }

enum class Anchor { START, CENTER, END }

enum class Stroked { SOLID, DASHED, DOTTED }

sealed interface SketchMark {
    /** A stroked path, trimmed along its length as it draws, with a pen tip at the live end. */
    data class Line(
        val path: Path,
        val ink: Ink,
        val weight: Float = 1f,
        val style: Stroked = Stroked.SOLID
    ) : SketchMark

    /** A closed region washed with a vertical gradient, swept in from the left. */
    data class Area(val path: Path, val ink: Ink) : SketchMark

    /** A rectangle of tone: a zone, a band, a phase. */
    data class Band(
        val l: Float,
        val t: Float,
        val r: Float,
        val b: Float,
        val ink: Ink,
        val radius: Float = 0f
    ) : SketchMark

    /** A column growing up from its base. A ghost column is a dashed outline: what was left. */
    data class Column(
        val x: Float,
        val top: Float,
        val base: Float,
        val width: Float,
        val ink: Ink,
        val ghost: Boolean = false
    ) : SketchMark

    data class Dot(
        val x: Float,
        val y: Float,
        val r: Float,
        val ink: Ink,
        val filled: Boolean = true,
        /** A standing halo, and one pulse when the dot lands. Reserved for the answer. */
        val halo: Boolean = false
    ) : SketchMark

    data class Label(
        val text: String,
        val x: Float,
        val y: Float,
        val anchor: Anchor = Anchor.START,
        val ink: Ink = Ink.GUIDE,
        /** The drawing's one serif figure. */
        val figure: Boolean = false,
        /** When set, a figure counts up to this value as it appears, printed with [decimals]. */
        val countTo: Float? = null,
        val decimals: Int = 0
    ) : SketchMark
}

/** The design box every drawing is laid out in. */
private const val BOX_W = 160f
private const val BOX_H = 110f

/** The aspect every drawing is shown at, hero and thumbnail alike. */
const val SKETCH_ASPECT = BOX_W / BOX_H

/**
 * How long a drawing takes to assemble: three and a half chart draw-ins. A drawing lays down a
 * grid, a line, its wash, an accent and five or six words; at one draw-in all of that arrived at
 * once and read as a flash rather than as something being explained.
 */
private const val DRAW_MS = ForgeMotion.DurationDraw * 7 / 2

/** Caption and figure sizes, in design units. At hero width one unit is about 2.2dp. */
private const val LABEL_UNITS = 4.9f
private const val FIGURE_UNITS = 12f

/** How long a halo's pulse runs after its dot lands, as a share of the whole draw. */
private const val PULSE = 0.09f

/**
 * One lesson's drawing.
 *
 * [labels] false is the thumbnail: no words, no guides, the shape alone. [animate] false lands on
 * the finished frame; so does reduced motion. [replayKey] restarts the draw, which the opening
 * rotator uses so each lesson assembles as it turns up.
 */
@Composable
fun LessonSketch(
    lessonId: String,
    modifier: Modifier = Modifier,
    labels: Boolean = true,
    animate: Boolean = true,
    replayKey: Any? = lessonId
) {
    // A thumbnail is its own composition (`AcademyThumbs`), not the hero with its words stripped
    // off: a frame cut from a drawing that only makes sense once it has finished read as a piece
    // missing its labels.
    val sketch = remember(lessonId, labels) {
        if (labels) AcademySketches.forLesson(lessonId)
        else AcademyThumbs.forLesson(lessonId) ?: AcademySketches.forLesson(lessonId)
    }
    SketchCanvas(
        sketch = sketch,
        modifier = modifier,
        labels = labels,
        animate = animate,
        replayKey = replayKey
    )
}

/**
 * Any [Sketch] on the page: a lesson's drawing, its thumbnail, or a figure inside the lesson.
 *
 * [play] false holds the drawing blank until it is set, which is how a figure waits for the reader
 * to scroll to it before it starts to draw.
 */
@Composable
internal fun SketchCanvas(
    sketch: Sketch,
    modifier: Modifier = Modifier,
    labels: Boolean = true,
    animate: Boolean = true,
    play: Boolean = true,
    replayKey: Any? = sketch,
    /** Centre on what is drawn. Off for drawings whose content moves under a finger. */
    fitContent: Boolean = true
) {
    val measurer = rememberTextMeasurer()
    val scheme = MaterialTheme.colorScheme
    val colors = SketchColors(
        guide = scheme.onSurfaceVariant.copy(alpha = 0.65f),
        grid = scheme.outline,
        ink = scheme.onBackground,
        accent = scheme.primary,
        label = scheme.onSurfaceVariant,
        labelStrong = scheme.onBackground,
        ground = scheme.background
    )
    val captionStyle = MaterialTheme.typography.labelSmall
    val figureStyle = MaterialTheme.typography.headlineSmall
    val drawn = animate && !ForgeMotion.animationsOff
    val progress = remember(replayKey) { Animatable(if (drawn) 0f else 1f) }
    LaunchedEffect(replayKey, drawn, play) {
        if (drawn && !play) {
            progress.snapTo(0f)
        } else if (drawn) {
            progress.snapTo(0f)
            // Linear on the master clock: each mark eases inside its own window, so an eased master
            // would bunch the whole choreography into its first second.
            progress.animateTo(1f, tween(ForgeMotion.scaledDuration(DRAW_MS), easing = LinearEasing))
        } else {
            progress.snapTo(1f)
        }
    }

    Canvas(
        modifier
            .fillMaxWidth()
            .aspectRatio(SKETCH_ASPECT)
            .semantics { contentDescription = sketch.description }
    ) {
        drawSketch(
            sketch,
            progress.value,
            colors,
            if (labels) measurer else null,
            captionStyle,
            figureStyle,
            // A dedicated thumbnail is drawn whole; only a hero standing in for one is stripped.
            stripForThumb = !labels && sketch.marks.any { it.mark is SketchMark.Label },
            fitContent = fitContent
        )
    }
}

private class SketchColors(
    val guide: Color,
    val grid: Color,
    val ink: Color,
    val accent: Color,
    val label: Color,
    val labelStrong: Color,
    val ground: Color
)

private fun SketchColors.of(role: Ink): Color = when (role) {
    Ink.GUIDE -> guide
    Ink.INK -> ink
    Ink.ACCENT -> accent
}

/** Deceleration inside a mark's own window. */
private fun ease(t: Float): Float = 1f - (1f - t).pow(3)

private fun DrawScope.drawSketch(
    sketch: Sketch,
    progress: Float,
    colors: SketchColors,
    /** Null on thumbnails, which draw no words and no guides. */
    measurer: TextMeasurer?,
    captionStyle: TextStyle,
    figureStyle: TextStyle,
    stripForThumb: Boolean,
    fitContent: Boolean
) {
    val full = measurer != null
    val scale = minOf(size.width / BOX_W, size.height / BOX_H)
    val middle = if (fitContent) sketch.center(withLabels = full) else Offset(BOX_W / 2f, BOX_H / 2f)
    val dx = size.width / 2f - middle.x * scale
    val dy = size.height / 2f - middle.y * scale
    fun px(x: Float) = dx + x * scale
    fun py(y: Float) = dy + y * scale
    // Line widths are set in dp and stay hairlines at any size; they are divided back out of the
    // box's scale because the paths are drawn inside it.
    val hair = (if (full) 1.8.dp else 1.5.dp).toPx()

    sketch.marks.forEach { timed ->
        val mark = timed.mark
        if (stripForThumb && !mark.keepsInThumbnail()) return@forEach
        val span = (timed.t1 - timed.t0).coerceAtLeast(0.0001f)
        val raw = ((progress - timed.t0) / span).coerceIn(0f, 1f)
        if (raw <= 0f) return@forEach
        val local = ease(raw)

        when (mark) {
            is SketchMark.Line -> {
                val width = when (mark.ink) {
                    Ink.GUIDE -> hair * 0.5f
                    Ink.INK -> hair
                    Ink.ACCENT -> hair * 1.25f
                } * mark.weight
                val (path, tip) = if (raw >= 1f) mark.path to null else mark.path.trimmed(local)
                val dotted = mark.ink == Ink.GUIDE && mark.style == Stroked.DOTTED
                withTransform({
                    translate(dx, dy)
                    scale(scale, scale, pivot = Offset.Zero)
                }) {
                    drawPath(
                        path,
                        color = colors.of(mark.ink),
                        style = Stroke(
                            width = (if (dotted) width * 1.6f else width) / scale,
                            cap = if (mark.style == Stroked.DASHED) StrokeCap.Butt else StrokeCap.Round,
                            join = StrokeJoin.Round,
                            pathEffect = when (mark.style) {
                                Stroked.SOLID -> null
                                Stroked.DASHED -> PathEffect.dashPathEffect(floatArrayOf(3f, 2.4f))
                                Stroked.DOTTED -> PathEffect.dashPathEffect(floatArrayOf(0.01f, 2.8f))
                            }
                        )
                    )
                }
                // The pen: a bead at the live end of a data line while it is being drawn.
                if (tip != null && mark.ink != Ink.GUIDE) {
                    drawCircle(colors.of(mark.ink), width * 1.5f, Offset(px(tip.x), py(tip.y)))
                }
            }

            is SketchMark.Area -> {
                val b = mark.path.getBounds()
                val top = if (mark.ink == Ink.ACCENT) colors.accent else colors.ink
                val brush = Brush.verticalGradient(
                    0f to top.copy(alpha = 0.15f),
                    1f to top.copy(alpha = 0f),
                    startY = b.top,
                    endY = b.bottom
                )
                withTransform({
                    translate(dx, dy)
                    scale(scale, scale, pivot = Offset.Zero)
                }) {
                    clipRect(b.left, b.top - 1f, b.left + b.width * local, b.bottom + 1f) {
                        drawPath(mark.path, brush)
                    }
                }
            }

            is SketchMark.Band -> {
                val rect = Rect(px(mark.l), py(mark.t), px(mark.r), py(mark.b))
                val (from, to) = when (mark.ink) {
                    Ink.ACCENT -> colors.accent.copy(alpha = 0.25f) to colors.accent.copy(alpha = 0f)
                    else -> colors.grid to colors.grid.copy(alpha = 0.35f)
                }
                drawRoundRect(
                    Brush.verticalGradient(0f to from, 1f to to, startY = rect.top, endY = rect.bottom),
                    topLeft = rect.topLeft,
                    size = rect.size,
                    cornerRadius = CornerRadius(mark.radius * scale),
                    alpha = local
                )
            }

            is SketchMark.Column -> {
                val w = mark.width * scale
                val left = px(mark.x) - w / 2f
                val base = py(mark.base)
                val fullTop = py(mark.top)
                val top = base - (base - fullTop) * local
                if (mark.ghost) {
                    drawRoundRect(
                        colors.guide,
                        topLeft = Offset(left, top),
                        size = Size(w, base - top),
                        cornerRadius = CornerRadius(minOf(w / 2f, 2.2f * scale)),
                        style = Stroke(
                            width = hair * 0.6f,
                            pathEffect = PathEffect.dashPathEffect(floatArrayOf(hair * 1.6f, hair * 1.4f))
                        )
                    )
                } else {
                    val c = colors.of(mark.ink)
                    drawRoundRect(
                        Brush.verticalGradient(0f to c, 1f to c.copy(alpha = 0.35f), startY = fullTop, endY = base),
                        topLeft = Offset(left, top),
                        size = Size(w, base - top),
                        cornerRadius = CornerRadius(minOf(w / 2f, 2.2f * scale))
                    )
                }
            }

            is SketchMark.Dot -> {
                val c = Offset(px(mark.x), py(mark.y))
                val r = mark.r * scale * local
                val color = colors.of(mark.ink)
                if (mark.halo) {
                    drawCircle(color.copy(alpha = 0.15f), r * 2.6f, c)
                    val pulse = ((progress - timed.t1) / PULSE).coerceIn(0f, 1f)
                    if (pulse > 0f && pulse < 1f) {
                        drawCircle(
                            color,
                            radius = r * (1.2f + 3.2f * ease(pulse)),
                            center = c,
                            alpha = (1f - pulse) * 0.6f,
                            style = Stroke(width = hair * 0.8f)
                        )
                    }
                }
                if (mark.filled) {
                    drawCircle(color, r, c)
                } else {
                    // A ring knocks the ground out behind it, so lines pass under rather than through.
                    drawCircle(colors.ground, r, c)
                    drawCircle(color, r, c, style = Stroke(width = hair * 0.9f))
                }
            }

            is SketchMark.Label -> if (measurer != null) {
                val units = if (mark.figure) FIGURE_UNITS else LABEL_UNITS
                val sizeSp = (units * scale / density / fontScale).sp
                val base = if (mark.figure) figureStyle else captionStyle
                val style = base.copy(
                    fontSize = sizeSp,
                    lineHeight = sizeSp,
                    letterSpacing = if (mark.figure) base.letterSpacing else (units * 0.1f * scale / density / fontScale).sp,
                    color = if (mark.ink == Ink.GUIDE) colors.label else colors.labelStrong
                )
                val shown = if (mark.figure) mark.text else mark.text.uppercase()
                val text = mark.countTo?.let { target ->
                    val v = target * local
                    if (mark.decimals == 0) v.roundToInt().toString() else "%.${mark.decimals}f".format(v)
                } ?: shown
                val layout = measurer.measure(text, style)
                // Placed by the finished text, so a counting figure does not slide as its digits change.
                val finalWidth = if (mark.countTo != null) measurer.measure(shown, style).size.width else layout.size.width
                val left = when (mark.anchor) {
                    Anchor.START -> px(mark.x)
                    Anchor.CENTER -> px(mark.x) - finalWidth / 2f
                    Anchor.END -> px(mark.x) - finalWidth
                }
                val rise = (1f - local) * 2.5f * scale
                val top = py(mark.y) - layout.size.height / 2f + rise
                drawText(layout, topLeft = Offset(left, top), alpha = local)
            }
        }
    }
}

/** A thumbnail keeps the shape: ink, accent and their washes, without axes, grid or words. */
private fun SketchMark.keepsInThumbnail(): Boolean = when (this) {
    is SketchMark.Label -> false
    is SketchMark.Line -> ink != Ink.GUIDE
    is SketchMark.Area -> true
    is SketchMark.Band -> ink == Ink.ACCENT
    is SketchMark.Column -> !ghost
    is SketchMark.Dot -> ink != Ink.GUIDE
}

/**
 * The first [fraction] of a path's length, and where the pen is.
 *
 * The platform measure, not Compose's: Compose's walks only the first contour, and a set of
 * separate strokes is several. Contours are trimmed in the order they were drawn.
 */
private fun Path.trimmed(fraction: Float): Pair<Path, Offset?> {
    val src = asAndroidPath()
    val measure = android.graphics.PathMeasure(src, false)
    var total = 0f
    do {
        total += measure.length
    } while (measure.nextContour())
    var remaining = total * fraction
    measure.setPath(src, false)
    val out = android.graphics.Path()
    var tip: Offset? = null
    val pos = FloatArray(2)
    do {
        if (remaining <= 0f) break
        val len = measure.length
        val take = minOf(len, remaining)
        measure.getSegment(0f, take, out, true)
        if (measure.getPosTan(take, pos, null)) tip = Offset(pos[0], pos[1])
        remaining -= len
    } while (measure.nextContour())
    return out.asComposePath() to tip
}

/**
 * Lays a drawing out on a clock measured in beats, then normalises it to 0..1.
 *
 * Beats keep the choreography readable where it is written ("the curve inks from beat 2 to beat
 * 6"), and the normalisation means a drawing with more to say simply has more beats rather than
 * everything being re-timed by hand. A tail is left so the last halo finishes its pulse.
 */
internal class Choreo {
    private val items = mutableListOf<Triple<SketchMark, Float, Float>>()

    fun at(start: Float, beats: Float, vararg marks: SketchMark) {
        marks.forEach { items += Triple(it, start, start + beats) }
    }

    /** Several marks one after another, each [beats] long, starting [gap] apart. */
    fun stagger(start: Float, beats: Float, gap: Float, marks: List<SketchMark>) {
        marks.forEachIndexed { i, m -> items += Triple(m, start + i * gap, start + i * gap + beats) }
    }

    fun build(description: String): Sketch {
        val end = items.maxOf { it.third }
        val total = end / (1f - PULSE)
        return Sketch(items.map { (m, a, b) -> Timed(m, a / total, b / total) }, description)
    }
}

internal fun sketch(description: String, body: Choreo.() -> Unit): Sketch =
    Choreo().apply(body).build(description)

/**
 * The twelve drawings, one per lesson, built once.
 *
 * Each is an exact diagram of the lesson's single idea, with the accent on the answer. The numbers
 * drawn are the lesson's own (10 sets, 1.6 g per kg, 48 to 72 hours); the series they sit on are
 * illustrative shapes, never a claim about the reader.
 */
object AcademySketches {

    private val byId: Map<String, Sketch> by lazy {
        mapOf(
            AcademyTraining.gettingStronger.id to gettingStronger(),
            AcademyTraining.effort.id to effort(),
            AcademyTraining.volume.id to volume(),
            AcademyTraining.form.id to warmupRamp(),
            AcademyTraining.recovery.id to recovery(),
            AcademyTraining.soreness.id to soreness(),
            AcademyTraining.protein.id to protein(),
            AcademyCoachLessons.howItDecides.id to coachLoop(),
            AcademyCoachLessons.readiness.id to readiness(),
            AcademyCoachLessons.blocks.id to block(),
            AcademyCardio.zone2.id to zones(),
            AcademyCardio.intervals.id to intervals()
        )
    }

    /** Every shipped lesson has a drawing; `AcademySketchesTest` holds that true. */
    fun has(lessonId: String): Boolean = AcademyRegistry.canonical(lessonId) in byId

    fun forLesson(lessonId: String): Sketch =
        byId[AcademyRegistry.canonical(lessonId)] ?: Sketch(emptyList(), "")

    // ── Geometry ─────────────────────────────────────────────────────────────

    internal fun line(vararg xy: Float): Path = Path().apply {
        moveTo(xy[0], xy[1])
        var i = 2
        while (i < xy.size) {
            lineTo(xy[i], xy[i + 1]); i += 2
        }
    }

    /** Separate strokes in one path: each group of four floats is one segment. */
    internal fun segments(vararg xy: Float): Path = Path().apply {
        var i = 0
        while (i < xy.size) {
            moveTo(xy[i], xy[i + 1]); lineTo(xy[i + 2], xy[i + 3]); i += 4
        }
    }

    internal fun curve(x0: Float, x1: Float, steps: Int = 64, y: (Float) -> Float): Path = Path().apply {
        moveTo(x0, y(x0))
        for (s in 1..steps) {
            val x = x0 + (x1 - x0) * s / steps
            lineTo(x, y(x))
        }
    }

    /** Catmull-Rom through hand-placed points, so a series reads as one continuous line. */
    internal fun smooth(vararg xy: Float): Path = Path().apply {
        val pts = xy.toList().chunked(2).map { Offset(it[0], it[1]) }
        moveTo(pts[0].x, pts[0].y)
        for (i in 0 until pts.size - 1) {
            val p0 = pts[maxOf(i - 1, 0)]
            val p1 = pts[i]
            val p2 = pts[i + 1]
            val p3 = pts[minOf(i + 2, pts.size - 1)]
            cubicTo(
                p1.x + (p2.x - p0.x) / 6f, p1.y + (p2.y - p0.y) / 6f,
                p2.x - (p3.x - p1.x) / 6f, p2.y - (p3.y - p1.y) / 6f,
                p2.x, p2.y
            )
        }
    }

    /** The region between a line and a floor, for a wash. */
    internal fun under(path: Path, x0: Float, x1: Float, floor: Float): Path = Path().apply {
        addPath(path)
        lineTo(x1, floor)
        lineTo(x0, floor)
        close()
    }

    /** A horizontal axis with ticks hanging below it. */
    internal fun axisX(y: Float, x0: Float, x1: Float, ticks: List<Float>, tick: Float = 2.2f): Path =
        Path().apply {
            moveTo(x0, y); lineTo(x1, y)
            ticks.forEach { moveTo(it, y); lineTo(it, y + tick) }
        }

    internal fun axisY(x: Float, y0: Float, y1: Float, ticks: List<Float>): Path = Path().apply {
        moveTo(x, y0); lineTo(x, y1)
        ticks.forEach { moveTo(x - 2.2f, it); lineTo(x, it) }
    }

    internal fun gridH(ys: List<Float>, x0: Float, x1: Float): Path = Path().apply {
        ys.forEach { moveTo(x0, it); lineTo(x1, it) }
    }

    /** A bracket: a span with its ends turned down, or up with a negative [lip]. */
    internal fun bracket(x0: Float, x1: Float, y: Float, lip: Float = 2.4f): Path =
        line(x0, y + lip, x0, y, x1, y, x1, y + lip)

    internal fun arrowHead(tipX: Float, tipY: Float, dirX: Float, dirY: Float, size: Float = 2.6f): Path {
        val len = sqrt(dirX * dirX + dirY * dirY)
        val ux = dirX / len
        val uy = dirY / len
        val bx = tipX - ux * size
        val by = tipY - uy * size
        return line(
            bx - uy * size * 0.6f, by + ux * size * 0.6f,
            tipX, tipY,
            bx + uy * size * 0.6f, by - ux * size * 0.6f
        )
    }

    internal fun label(text: String, x: Float, y: Float, anchor: Anchor = Anchor.START, strong: Boolean = false) =
        SketchMark.Label(text, x, y, anchor, if (strong) Ink.INK else Ink.GUIDE)

    internal fun figure(value: Float, decimals: Int, x: Float, y: Float, anchor: Anchor = Anchor.START) =
        SketchMark.Label(
            text = if (decimals == 0) value.roundToInt().toString() else "%.${decimals}f".format(value),
            x = x,
            y = y,
            anchor = anchor,
            ink = Ink.INK,
            figure = true,
            countTo = value,
            decimals = decimals
        )

    internal fun dotted(path: Path) = SketchMark.Line(path, Ink.GUIDE, style = Stroked.DOTTED)

    // ── Training ─────────────────────────────────────────────────────────────

    /**
     * Double progression, from real sets: eight sessions of three sets each. Reps climb inside the
     * range, and the session where every set reaches 10 is the one that moves the bar up.
     */
    private fun gettingStronger(): Sketch = sketch(
        "Eight sessions of three sets. Reps climb from 8 toward 10, and each time every set reaches 10 the weight goes up 2.5 kg."
    ) {
        val xs = (0 until 8).map { 28f + it * 16.4f }
        val reps = listOf(
            listOf(8, 8, 8), listOf(9, 9, 8), listOf(10, 10, 9), listOf(10, 10, 10),
            listOf(8, 8, 8), listOf(9, 9, 8), listOf(10, 9, 9), listOf(10, 10, 10)
        )
        fun ry(r: Int) = 90f - (r - 8) * 12f
        val jump1 = xs[3] + 7.6f
        val jump2 = xs[7] + 7.6f
        val weight = line(18f, 40f, jump1, 40f, jump1, 29f, jump2, 29f, jump2, 18f, 150f, 18f)

        at(0f, 2f, dotted(gridH(listOf(ry(8), ry(9), ry(10)), 22f, 150f)))
        at(0.3f, 1.6f, label("10", 18f, ry(10), Anchor.END), label("9", 18f, ry(9), Anchor.END), label("8", 18f, ry(8), Anchor.END))
        at(0.6f, 1.6f, label("reps per set", 22f, 58f), label("weight", 18f, 10f))
        at(1.2f, 4.2f, SketchMark.Line(weight, Ink.INK, weight = 1.1f))
        at(2f, 4.4f, SketchMark.Area(under(weight, 18f, 150f, 48f), Ink.INK))

        xs.forEachIndexed { i, x ->
            val topped = reps[i].all { it == 10 }
            val dots = reps[i].mapIndexed { k, r ->
                SketchMark.Dot(x + (k - 1) * 3.4f, ry(r), 1.45f, if (topped) Ink.ACCENT else Ink.INK)
            }
            at(2.4f + i * 0.55f, 0.9f, *dots.toTypedArray())
        }
        at(3.2f, 2.4f, label("session 1", 20f, 101f), label("4", xs[3], 101f, Anchor.CENTER), label("8", xs[7], 101f, Anchor.CENTER))

        at(4.6f, 1.2f, dotted(line(xs[3], ry(10) - 4f, xs[3], 45f, jump1 - 1f, 42f)))
        at(4.9f, 1f, SketchMark.Dot(xs[3], ry(10), 1.6f, Ink.ACCENT, halo = true))
        at(6.9f, 1.2f, dotted(line(xs[7], ry(10) - 4f, xs[7], 34f, jump2 - 1f, 31f)))
        at(7.2f, 1f, SketchMark.Dot(xs[7], ry(10), 1.6f, Ink.ACCENT, halo = true))

        at(5.4f, 1.6f, label("60 kg", 20f, 34f), label("62.5 kg", jump1 + 2f, 23f), label("65 kg", jump2 - 2f, 12f, Anchor.END))
        at(6f, 1.8f, label("every set at 10", xs[3] + 3f, 52f, strong = true))
        at(6.4f, 1.6f, label("+2.5 kg", jump1 - 2f, 35f, Anchor.END, strong = true))
    }

    /**
     * Reps in reserve, read off bar speed: each rep is slower than the last, the trend runs to the
     * rep that would not move, and the set stops two short of it.
     */
    private fun effort(): Sketch = sketch(
        "Each rep moves slower than the last. Stop after rep 8 with two reps still in reserve, rather than grinding to failure at rep 11."
    ) {
        val base = 86f
        fun x(rep: Int) = 20f + (rep - 1) * 11.6f
        fun speed(rep: Float) = 58f * (1f - ((rep - 1f) / 10f).pow(1.55f))
        val failX = x(11)
        val stopX = (x(8) + x(9)) / 2f

        at(0f, 1.6f, SketchMark.Line(axisX(base, 14f, 150f, emptyList()), Ink.GUIDE))
        at(0.3f, 1.6f, dotted(gridH(listOf(base - 20f, base - 60f), 14f, 150f)))
        at(0.6f, 1.4f, label("bar speed", 16f, 12f))

        stagger(1.2f, 1f, 0.42f, (1..8).map { SketchMark.Column(x(it), base - speed(it.toFloat()), base, 5.2f, Ink.INK) })
        at(4.6f, 1.6f, SketchMark.Line(curve(x(1), failX) { base - speed(1f + (it - x(1)) / 11.6f) }, Ink.GUIDE, style = Stroked.DASHED))
        stagger(5.2f, 1f, 0.4f, (9..10).map { SketchMark.Column(x(it), base - speed(it.toFloat()), base, 5.2f, Ink.GUIDE, ghost = true) })
        at(6.2f, 0.9f, SketchMark.Line(segments(failX - 2.4f, base - 2.4f, failX + 2.4f, base + 2.4f, failX - 2.4f, base + 2.4f, failX + 2.4f, base - 2.4f), Ink.GUIDE, weight = 1.6f))
        at(6.6f, 1.4f, label("failure", failX, 95f, Anchor.CENTER))

        at(6.8f, 1.6f, SketchMark.Line(line(stopX, 16f, stopX, 92f), Ink.ACCENT))
        at(7.8f, 1.2f, SketchMark.Dot(stopX, 16f, 1.8f, Ink.ACCENT, halo = true))
        at(7.4f, 1.4f, label("stop here", stopX - 5f, 16f, Anchor.END, strong = true))
        at(8f, 1.2f, SketchMark.Line(bracket(x(9) - 3f, x(10) + 3f, 56f), Ink.GUIDE))
        at(8.4f, 1.8f, figure(2f, 0, (x(9) + x(10)) / 2f, 37f, Anchor.CENTER))
        at(8.8f, 1.6f, label("in reserve", stopX + 3f, 48f))
        at(8.6f, 1.6f, label("rep 1", x(1), 95f, Anchor.CENTER), label("rep 8", x(8), 95f, Anchor.CENTER))
    }

    /** Growth against weekly sets, and the fatigue each set costs: the gap is the sweet spot. */
    private fun volume(): Sketch = sketch(
        "Growth rises fast up to about 10 hard sets per muscle a week and slowly to 20, while fatigue keeps climbing. Between 10 and 20 is the sweet spot."
    ) {
        val base = 90f
        fun x(sets: Float) = 18f + sets * 4.4f
        fun growth(px: Float) = base - 66f * (1f - exp(-((px - 18f) / 4.4f) / 8.3f))
        fun fatigue(px: Float) = base - 50f * ((px - 18f) / 132f).pow(2.2f)
        val g = curve(18f, 150f) { growth(it) }
        val p10 = Offset(x(10f), growth(x(10f)))

        at(0f, 1.6f, SketchMark.Line(axisX(base, 18f, 150f, listOf(x(0f), x(10f), x(20f), x(30f))), Ink.GUIDE))
        at(0.2f, 1.6f, SketchMark.Line(axisY(18f, 14f, base, emptyList()), Ink.GUIDE))
        at(0.4f, 1.6f, dotted(gridH(listOf(base - 22f, base - 44f, base - 66f), 18f, 150f)))
        at(0.8f, 1.4f, label("0", x(0f), 98f, Anchor.CENTER), label("10", x(10f), 98f, Anchor.CENTER), label("20", x(20f), 98f, Anchor.CENTER))
        at(1f, 1.4f, label("hard sets a week", 150f, 105f, Anchor.END), label("growth", 21f, 10f))

        at(1.6f, 1.8f, SketchMark.Band(x(10f), 14f, x(20f), base, Ink.ACCENT))
        at(2f, 4.2f, SketchMark.Line(g, Ink.INK, weight = 1.1f))
        at(2.6f, 4.2f, SketchMark.Area(under(g, 18f, 150f, base), Ink.INK))
        at(4.4f, 3f, SketchMark.Line(curve(18f, 150f) { fatigue(it) }, Ink.GUIDE, weight = 1.6f, style = Stroked.DASHED))
        at(6.4f, 1.6f, label("fatigue", 150f, 70f, Anchor.END))

        at(6f, 1.1f, SketchMark.Dot(p10.x, p10.y, 2.1f, Ink.ACCENT, halo = true))
        at(6.4f, 1.1f, SketchMark.Dot(x(20f), growth(x(20f)), 1.9f, Ink.INK, filled = false))
        at(6.8f, 1.6f, label("sweet spot", (x(10f) + x(20f)) / 2f, 19f, Anchor.CENTER, strong = true))
        at(7f, 1.2f, dotted(line(p10.x - 3f, p10.y - 2f, 53f, 36f)))
        at(7.3f, 1.6f, label("most gain", 22f, 34f, strong = true))
    }

    /** The warm-up ramp: each set heavier and shorter, arriving at the working weight fresh. */
    private fun warmupRamp(): Sketch = sketch(
        "Warm up by ramping: the empty bar for 10, about half for 5, seventy percent for 3, eighty-five percent for 1 or 2, then your working sets."
    ) {
        val base = 88f
        val xs = listOf(30f, 56f, 82f, 108f, 136f)
        val pct = listOf(20f, 50f, 70f, 85f, 100f)
        fun top(p: Float) = base - p * 0.64f
        val repsText = listOf("10 reps", "5 reps", "3 reps", "1 rep")

        at(0f, 1.6f, SketchMark.Line(axisX(base, 14f, 150f, emptyList()), Ink.GUIDE))
        at(0.4f, 1.8f, dotted(gridH(listOf(top(100f)), 14f, 150f)))
        at(0.8f, 1.6f, label("working weight", 16f, top(100f) - 6f))

        stagger(1.4f, 1.3f, 0.7f, xs.indices.map { i ->
            SketchMark.Column(xs[i], top(pct[i]), base, 11f, if (i == 4) Ink.ACCENT else Ink.INK)
        })
        at(4.6f, 2.2f, dotted(line(*xs.indices.flatMap { listOf(xs[it], top(pct[it]) - 3f) }.toFloatArray())))
        at(6.2f, 1.1f, SketchMark.Dot(xs[4], top(100f), 1.8f, Ink.ACCENT, halo = true))

        stagger(2f, 1.4f, 0.7f, xs.indices.map { i ->
            SketchMark.Label(listOf("bar", "50%", "70%", "85%", "work")[i], xs[i], 96f, Anchor.CENTER, if (i == 4) Ink.INK else Ink.GUIDE)
        })
        at(6.6f, 1.4f, label("% of your working weight", 82f, 105f, Anchor.CENTER))
        stagger(2.4f, 1.4f, 0.7f, (0 until 4).map { i ->
            SketchMark.Label(repsText[i], xs[i], top(pct[i]) - 7f, Anchor.CENTER, Ink.INK)
        })
    }

    /**
     * Supercompensation over three sessions: each one digs a hole, you rebuild past where you
     * started, and training again near that peak stacks the gains.
     */
    private fun recovery(): Sketch = sketch(
        "Each session leaves you below where you started, then you rebuild above it about 48 to 72 hours later. Train again near that peak and the gains stack. Wait too long and they fade."
    ) {
        val s1 = Offset(22f, 72f)
        val s2 = Offset(56f, 58f)
        val s3 = Offset(90f, 45f)
        val peak = Offset(124f, 33f)
        val response = smooth(
            14f, 72f, s1.x, s1.y, 33f, 86f, 46f, 66f, s2.x, s2.y, 67f, 73f, 80f, 53f, s3.x, s3.y,
            101f, 60f, 114f, 39f, peak.x, peak.y, 137f, 38f, 150f, 46f
        )

        at(0f, 1.6f, SketchMark.Line(axisX(94f, 14f, 150f, emptyList()), Ink.GUIDE))
        at(0.4f, 2f, dotted(line(14f, 72f, 150f, 72f)))
        at(0.9f, 1.4f, label("where you started", 150f, 78f, Anchor.END))
        at(1.4f, 5f, SketchMark.Line(response, Ink.INK, weight = 1.1f))
        at(2f, 5f, SketchMark.Area(under(response, 14f, 150f, 94f), Ink.INK))
        stagger(1.6f, 0.9f, 1.3f, listOf(s1, s2, s3).map { SketchMark.Dot(it.x, it.y, 1.9f, Ink.INK) })
        at(5.6f, 2.2f, SketchMark.Line(line(s1.x, s1.y - 5f, s2.x, s2.y - 5f, s3.x, s3.y - 5f, peak.x, peak.y - 5f), Ink.GUIDE, style = Stroked.DASHED))

        at(6.6f, 1.1f, SketchMark.Dot(peak.x, peak.y, 2.2f, Ink.ACCENT, halo = true))
        at(7f, 1.6f, label("train again", peak.x, peak.y - 10f, Anchor.CENTER, strong = true))
        at(6.8f, 0.9f, SketchMark.Dot(15f, 10f, 1.4f, Ink.INK))
        at(6.8f, 1.4f, label("session", 19f, 10f))
        at(7.4f, 1.2f, SketchMark.Line(bracket(s1.x, s2.x, 100f, -2.4f), Ink.GUIDE))
        at(7.8f, 1.6f, label("48 to 72 h", (s1.x + s2.x) / 2f, 106f, Anchor.CENTER, strong = true))
        at(8.1f, 1.6f, label("if you wait", 150f, 56f, Anchor.END))
        at(8.3f, 1.6f, label("gains stack", 64f, 38f, Anchor.CENTER))
    }

    /** Two small panels on one scale: soreness peaks and fades, injury is sharp and flares under load. */
    private fun soreness(): Sketch = sketch(
        "Soreness peaks a day or two after training and fades within the week. Injury pain is sharp from the start, stays, and flares every time the area is loaded."
    ) {
        val base = 86f
        fun ax(d: Float) = 16f + d * 8.4f
        fun bx(d: Float) = 92f + d * 8.4f
        val sore = smooth(
            ax(0f), 86f, ax(0.6f), 70f, ax(1.3f), 42f, ax(1.9f), 36f, ax(2.7f), 50f,
            ax(3.8f), 70f, ax(5f), 80f, ax(7f), 84f
        )
        val injury = line(
            bx(0f), 86f, bx(0.18f), 34f, bx(1.6f), 36f, bx(2.9f), 35f, bx(3.2f), 20f, bx(3.6f), 34f,
            bx(5.2f), 35f, bx(5.5f), 20f, bx(5.9f), 34f, bx(7f), 33f
        )

        at(0f, 1.6f, SketchMark.Line(axisX(base, ax(0f), ax(7f), listOf(ax(0f), ax(3f), ax(6f))), Ink.GUIDE))
        at(0.2f, 1.6f, SketchMark.Line(axisX(base, bx(0f), bx(7f), listOf(bx(0f), bx(3f), bx(6f))), Ink.GUIDE))
        at(0.6f, 1.4f, label("soreness", ax(0f), 12f, strong = true), label("injury", bx(0f), 12f, strong = true))
        at(
            0.9f, 1.4f,
            label("0", ax(0f), 94f, Anchor.CENTER), label("3", ax(3f), 94f, Anchor.CENTER), label("6", ax(6f), 94f, Anchor.CENTER),
            label("0", bx(0f), 94f, Anchor.CENTER), label("3", bx(3f), 94f, Anchor.CENTER), label("6", bx(6f), 94f, Anchor.CENTER)
        )
        at(1.1f, 1.4f, label("days", ax(7f), 103f, Anchor.END))

        at(1.6f, 3.6f, SketchMark.Line(sore, Ink.INK, weight = 1.1f))
        at(2.1f, 3.6f, SketchMark.Area(under(sore, ax(0f), ax(7f), base), Ink.INK))
        at(4.4f, 1.1f, SketchMark.Dot(ax(1.9f), 36f, 1.8f, Ink.INK))
        at(4.8f, 1.5f, label("peaks", ax(1.9f) + 4f, 30f), label("fades", ax(4.2f), 64f))

        at(4.6f, 3.4f, SketchMark.Line(injury, Ink.ACCENT))
        at(5f, 3.4f, SketchMark.Area(under(injury, bx(0f), bx(7f), base), Ink.ACCENT))
        at(7.2f, 1.1f, SketchMark.Dot(bx(5.5f), 20f, 1.9f, Ink.ACCENT, halo = true))
        at(6.6f, 1.2f, SketchMark.Line(segments(bx(3.2f), base, bx(3.2f), base - 4f, bx(5.5f), base, bx(5.5f), base - 4f), Ink.GUIDE, weight = 1.4f))
        at(7.4f, 1.6f, label("load", bx(3.2f), 102f, Anchor.CENTER, strong = true), label("load", bx(5.5f), 102f, Anchor.CENTER, strong = true))
        at(7.8f, 1.5f, label("sharp, stays", bx(0.6f), 52f))
    }

    /** Protein dose-response: the benefit flattens at about 1.6 g per kg a day. */
    private fun protein(): Sketch = sketch(
        "The benefit of protein rises until about 1.6 grams per kilogram of bodyweight a day, then flattens. Up to 2.2 is a sensible margin. For an 80 kg lifter that is about 130 grams."
    ) {
        val base = 90f
        fun x(g: Float) = 18f + g * 44f
        fun benefit(px: Float) = base - 50f * (1f - exp(-((px - 18f) / 44f) / 0.55f))
        val c = curve(18f, 150f) { benefit(it) }
        val p = Offset(x(1.6f), benefit(x(1.6f)))

        at(0f, 1.6f, SketchMark.Line(axisX(base, 18f, 150f, listOf(0f, 0.5f, 1f, 1.5f, 2f, 2.5f, 3f).map { x(it) }), Ink.GUIDE))
        at(0.2f, 1.6f, SketchMark.Line(axisY(18f, 30f, base, emptyList()), Ink.GUIDE))
        at(0.4f, 1.6f, dotted(gridH(listOf(base - 25f, base - 50f), 18f, 150f)))
        at(0.8f, 1.4f, label("0", x(0f), 98f, Anchor.CENTER), label("1", x(1f), 98f, Anchor.CENTER), label("2", x(2f), 98f, Anchor.CENTER), label("3", x(3f), 98f, Anchor.CENTER))
        at(1f, 1.4f, label("g per kg of bodyweight a day", 150f, 106f, Anchor.END))
        at(1.2f, 1.4f, label("muscle gained", 21f, 30f))

        at(1.6f, 1.8f, SketchMark.Band(x(1.6f), 36f, x(2.2f), base, Ink.ACCENT))
        at(2f, 4f, SketchMark.Line(c, Ink.INK, weight = 1.1f))
        at(2.6f, 4f, SketchMark.Area(under(c, 18f, 150f, base), Ink.INK))
        at(5.2f, 1.4f, SketchMark.Line(line(p.x, base, p.x, 36f), Ink.ACCENT))
        at(6.2f, 1.1f, SketchMark.Dot(p.x, p.y, 2.1f, Ink.ACCENT, halo = true))
        at(5.8f, 2f, figure(1.6f, 1, p.x, 17f, Anchor.CENTER))
        at(6.6f, 1.5f, label("g per kg", p.x, 29f, Anchor.CENTER, strong = true))
        at(7f, 1.5f, label("up to 2.2", x(2.2f) + 2f, 52f))
        at(7.4f, 1.6f, label("80 kg person", 148f, 72f, Anchor.END), label("130 g a day", 148f, 80f, Anchor.END, strong = true))
    }

    // ── Your coach ───────────────────────────────────────────────────────────

    /** The weekly loop: read, propose, you decide, then two weeks of watching before the verdict. */
    private fun coachLoop(): Sketch = sketch(
        "Each week the coach reads your log and proposes changes. You decide on each one. It then watches every applied change for 14 days before judging whether it worked."
    ) {
        val cx = 80f
        val cy = 57f
        val r = 33f
        fun on(deg: Float, radius: Float = r) =
            Offset(cx + radius * cos(deg * PI.toFloat() / 180f), cy + radius * sin(deg * PI.toFloat() / 180f))
        fun arc(from: Float, to: Float): Path = Path().apply {
            val p0 = on(from)
            moveTo(p0.x, p0.y)
            var a = from
            while (a < to) {
                a = minOf(a + 3f, to)
                val p = on(a)
                lineTo(p.x, p.y)
            }
        }
        fun head(deg: Float): Path {
            val tip = on(deg)
            val back = on(deg - 6f)
            return arrowHead(tip.x, tip.y, tip.x - back.x, tip.y - back.y)
        }
        val top = on(-90f)
        val right = on(0f)
        val bottom = on(90f)
        val left = on(180f)
        val gap = 13f

        at(0f, 1.4f, SketchMark.Dot(top.x, top.y, 3.2f, Ink.INK, filled = false))
        at(0.3f, 1.4f, label("reads your log", top.x, top.y - 10f, Anchor.CENTER))
        at(0.8f, 2f, SketchMark.Line(arc(-90f + gap, -gap), Ink.INK), SketchMark.Line(head(-gap), Ink.INK))
        at(2.4f, 1.2f, SketchMark.Dot(right.x, right.y, 3.2f, Ink.INK, filled = false))
        at(2.7f, 1.4f, label("proposes", right.x + 7f, right.y))
        at(3.2f, 2f, SketchMark.Line(arc(gap, 90f - gap), Ink.INK), SketchMark.Line(head(90f - gap), Ink.INK))
        at(4.8f, 1.2f, SketchMark.Dot(bottom.x, bottom.y, 3.6f, Ink.ACCENT, halo = true))
        at(5.2f, 1.5f, label("you decide", bottom.x, bottom.y + 11f, Anchor.CENTER, strong = true))

        // The watch: fourteen ticks, one a day, round the arc back to the start.
        val watch = Path()
        val a0 = 90f + gap
        val a1 = 180f - gap
        for (d in 0 until 14) {
            val a = a0 + (a1 - a0) * d / 13f
            val o = on(a, r - 2.2f)
            val i = on(a, r + 2.2f)
            watch.moveTo(o.x, o.y); watch.lineTo(i.x, i.y)
        }
        at(5.6f, 2.4f, SketchMark.Line(watch, Ink.INK))
        at(7.4f, 1.2f, SketchMark.Dot(left.x, left.y, 3.2f, Ink.INK, filled = false))
        at(7.6f, 1.6f, figure(14f, 0, left.x - 7f, left.y - 4f, Anchor.END))
        at(8f, 1.4f, label("days watched", left.x - 7f, left.y + 8f, Anchor.END))
        at(8f, 2f, SketchMark.Line(arc(180f + gap, 270f - gap), Ink.INK), SketchMark.Line(head(270f - gap), Ink.INK))
        at(8.8f, 1.6f, label("every monday", cx, cy, Anchor.CENTER))
    }

    /** Readiness: two weeks inside your own normal, today below it, and the small nudge it causes. */
    private fun readiness(): Sketch = sketch(
        "Your readings over two weeks sit inside your own normal range. Today's is below it, so today's targets ease down by a few percent. It never cancels the session."
    ) {
        val ys = listOf(46f, 40f, 49f, 43f, 38f, 47f, 51f, 44f, 40f, 48f, 45f, 52f, 55f, 74f)
        val xs = ys.indices.map { 18f + it * 7.2f }
        val trace = smooth(*xs.indices.flatMap { listOf(xs[it], ys[it]) }.toFloatArray())
        val today = Offset(xs.last(), ys.last())
        val gx = 131f

        at(0f, 1.6f, SketchMark.Line(axisX(92f, 14f, 114f, listOf(xs[0], xs[7], xs[13])), Ink.GUIDE))
        at(0.3f, 1.8f, SketchMark.Band(14f, 32f, 114f, 60f, Ink.GUIDE, radius = 2f))
        at(0.8f, 1.4f, label("your normal", 16f, 24f))
        at(1f, 1.4f, label("14 days", 16f, 100f), label("today", today.x, 100f, Anchor.CENTER, strong = true))
        at(1.4f, 4.2f, SketchMark.Line(trace, Ink.INK))
        stagger(1.6f, 0.7f, 0.3f, xs.indices.toList().dropLast(1).map { SketchMark.Dot(xs[it], ys[it], 1.1f, Ink.INK) })
        at(5.8f, 1.1f, SketchMark.Dot(today.x, today.y, 2.2f, Ink.ACCENT, halo = true))

        // The nudge: a small scale of today's targets, and where today lands on it.
        at(5.2f, 1.6f, SketchMark.Line(axisY(gx, 24f, 88f, listOf(24f, 40f, 56f, 72f, 88f)), Ink.GUIDE))
        at(5.4f, 1.4f, label("targets", gx, 14f, Anchor.CENTER), label("+", gx + 4f, 25f), label("0", gx + 4f, 56f), label("-", gx + 4f, 87f))
        at(6.4f, 1.4f, dotted(line(today.x + 4f, today.y - 1f, gx - 5f, 64f)))
        at(7f, 1.2f, SketchMark.Line(line(gx - 4f, 64f, gx + 1f, 64f), Ink.ACCENT, weight = 1.4f))
        at(7.2f, 1f, SketchMark.Dot(gx, 64f, 1.9f, Ink.ACCENT))
        at(7.6f, 1.6f, label("lighter", gx + 4f, 64f, strong = true))
    }

    /**
     * A block on the fitness-fatigue model: fatigue climbs faster than fitness while you build, so
     * what you can show stays flat, and the easy week clears the fatigue so it finally shows.
     */
    private fun block(): Sketch = sketch(
        "Across a block, fatigue climbs faster than fitness, so what you can show stays flat. The easy week clears the fatigue, and what you built shows."
    ) {
        fun x(w: Float) = 16f + w * 22.3f
        val fitness = smooth(x(0f), 70f, x(2f), 62f, x(4f), 54f, x(5f), 51f, x(6f), 49f)
        val fatigue = smooth(x(0f), 74f, x(2f), 54f, x(4f), 40f, x(5f), 36f, x(6f), 72f)
        val shows = smooth(x(0f), 88f, x(2f), 88f, x(4f), 89f, x(5f), 86f, x(5.6f), 70f, x(6f), 58f)
        val phases = listOf(Triple(0f, 2f, "build"), Triple(2f, 4f, "heavy"), Triple(4f, 5f, "peak"), Triple(5f, 6f, "easy"))

        stagger(0f, 1f, 0.35f, phases.map { (a, b, _) ->
            SketchMark.Band(x(a) + 0.8f, 14f, x(b) - 0.8f, 18f, if (b == 6f) Ink.ACCENT else Ink.GUIDE, radius = 2f)
        })
        stagger(0.4f, 1.2f, 0.35f, phases.map { (a, b, t) ->
            SketchMark.Label(t, (x(a) + x(b)) / 2f, 25f, Anchor.CENTER, if (b == 6f) Ink.INK else Ink.GUIDE)
        })
        at(1f, 1.6f, SketchMark.Line(axisX(94f, 16f, 150f, (0..6).map { x(it.toFloat()) }), Ink.GUIDE))
        at(1.2f, 1.6f, SketchMark.Band(x(5f), 32f, x(6f), 94f, Ink.ACCENT))
        at(1.4f, 1.4f, label("week 1", x(0.5f), 102f, Anchor.CENTER), label("week 6", x(5.5f), 102f, Anchor.CENTER))

        at(1.8f, 4f, SketchMark.Line(fatigue, Ink.GUIDE, weight = 1.6f, style = Stroked.DASHED))
        at(2.2f, 4f, SketchMark.Line(fitness, Ink.INK))
        at(4.8f, 1.5f, label("fatigue", x(3.4f), 36f, Anchor.CENTER))
        at(5f, 1.5f, label("fitness", x(2.6f), 67f, Anchor.CENTER, strong = true))
        at(5.4f, 3.4f, SketchMark.Line(shows, Ink.ACCENT))
        at(6f, 3f, SketchMark.Area(under(shows, x(0f), x(6f), 94f), Ink.ACCENT))
        at(8.4f, 1.1f, SketchMark.Dot(x(6f), 58f, 2.2f, Ink.ACCENT, halo = true))
        at(8.4f, 1.6f, label("what shows", x(4.8f), 80f, Anchor.END, strong = true))
    }

    // ── Cardio ───────────────────────────────────────────────────────────────

    /** Five heart-rate zones, and a 45-minute easy session holding in zone 2 with a little drift. */
    private fun zones(): Sketch = sketch(
        "An easy 45-minute session warms up, then holds steady in zone 2, the pace where you can still talk in full sentences. Late in the session heart rate drifts up slightly at the same pace."
    ) {
        val bands = listOf(18f, 30f, 42f, 54f, 66f, 78f)
        fun t(min: Float) = 40f + min / 45f * 110f
        val hr = smooth(
            t(0f), 76f, t(3f), 72f, t(7f), 63.5f, t(10f), 60.5f, t(16f), 60f, t(22f), 61f,
            t(28f), 59.8f, t(34f), 59.6f, t(39f), 58.4f, t(45f), 56.2f
        )

        at(0f, 2f, SketchMark.Line(gridH(bands, 40f, 150f), Ink.GUIDE))
        at(0.6f, 1.8f, SketchMark.Band(40f, 54f, 150f, 66f, Ink.ACCENT))
        at(
            0.8f, 1.4f,
            label("5", 34f, 24f, Anchor.END), label("4", 34f, 36f, Anchor.END), label("3", 34f, 48f, Anchor.END),
            label("zone 2", 34f, 60f, Anchor.END, strong = true), label("1", 34f, 72f, Anchor.END)
        )
        at(1f, 1.6f, SketchMark.Line(axisX(84f, 40f, 150f, listOf(t(0f), t(15f), t(30f), t(45f))), Ink.GUIDE))
        at(1.2f, 1.4f, label("0", t(0f), 92f, Anchor.CENTER), label("15", t(15f), 92f, Anchor.CENTER), label("30", t(30f), 92f, Anchor.CENTER), label("45 min", t(45f), 92f, Anchor.END))

        at(1.8f, 5f, SketchMark.Line(hr, Ink.INK, weight = 1.1f))
        at(2.4f, 5f, SketchMark.Area(under(hr, t(0f), t(45f), 78f), Ink.INK))
        at(6.4f, 1.1f, SketchMark.Dot(t(22f), 61f, 2f, Ink.ACCENT, halo = true))
        at(6.8f, 1.2f, dotted(line(t(22f), 57f, t(22f), 38f)))
        at(7.2f, 1.6f, label("you can still talk", t(22f) + 3f, 36f, strong = true))
        at(7.8f, 1.5f, label("drift", t(45f), 48f, Anchor.END))
        at(8f, 1.4f, label("heart rate", 16f, 10f))
    }

    /** One interval session, and where it goes in the week: a full day clear of heavy legs. */
    private fun intervals(): Sketch = sketch(
        "One interval session a week: a warm-up, five hard efforts with easy recoveries between, a cool-down. Put it on a day at least a full day away from your heavy leg sessions."
    ) {
        val hi = 15f
        val lo = 36f
        val floor = 46f
        val profile = Path().apply {
            moveTo(16f, floor)
            lineTo(32f, lo + 2f)
            var x = 34f
            repeat(5) {
                lineTo(x, lo); lineTo(x, hi); lineTo(x + 9f, hi); lineTo(x + 9f, lo)
                x += 19f
                lineTo(x, lo)
            }
            lineTo(150f, floor)
        }
        val bouts = Path().apply {
            var x = 34f
            repeat(5) {
                moveTo(x, hi); lineTo(x + 9f, hi); lineTo(x + 9f, floor); lineTo(x, floor); close()
                x += 19f
            }
        }

        at(0f, 1.4f, SketchMark.Line(line(16f, floor, 150f, floor), Ink.GUIDE))
        at(0.4f, 1.4f, label("one hard session", 150f, 7f, Anchor.END))
        at(0.8f, 5f, SketchMark.Line(profile, Ink.ACCENT))
        at(1.4f, 5f, SketchMark.Area(bouts, Ink.ACCENT))
        at(3.4f, 1.5f, label("hard", 38.5f, 8f, Anchor.CENTER, strong = true))

        // The week: seven cells, two heavy leg days, the hard session a day clear of both.
        fun day(i: Int) = 24f + i * 18.8f
        val letters = listOf("M", "T", "W", "T", "F", "S", "S")
        at(4.4f, 1.8f, SketchMark.Line(Path().apply {
            (0 until 7).forEach { i ->
                val c = day(i)
                addRoundRect(RoundRect(c - 8f, 66f, c + 8f, 88f, CornerRadius(2.5f)))
            }
        }, Ink.GUIDE))
        stagger(4.6f, 1f, 0.15f, letters.mapIndexed { i, s ->
            SketchMark.Label(s, day(i), 96f, Anchor.CENTER, if (i == 2) Ink.INK else Ink.GUIDE)
        })
        at(5.6f, 1.1f, SketchMark.Column(day(0), 70f, 85f, 5f, Ink.INK), SketchMark.Column(day(4), 70f, 85f, 5f, Ink.INK))
        at(6f, 1.4f, label("legs", day(0), 104f, Anchor.CENTER), label("legs", day(4), 104f, Anchor.CENTER))
        at(6.2f, 1.2f, SketchMark.Line(Path().apply {
            val c = day(2)
            moveTo(c - 5f, 84f); lineTo(c - 5f, 72f); lineTo(c - 2f, 72f); lineTo(c - 2f, 84f)
            lineTo(c + 1f, 84f); lineTo(c + 1f, 72f); lineTo(c + 4f, 72f); lineTo(c + 4f, 84f)
        }, Ink.ACCENT))
        at(6.6f, 1f, SketchMark.Dot(day(5), 77f, 3f, Ink.INK, filled = false))
        at(7f, 1.4f, SketchMark.Line(Path().apply {
            moveTo(day(0) + 3f, 64f); quadraticTo((day(0) + day(2)) / 2f, 56f, day(2) - 3f, 64f)
        }, Ink.GUIDE, style = Stroked.DOTTED))
        at(7.6f, 1.6f, label("a day clear", (day(0) + day(2)) / 2f, 54f, Anchor.CENTER, strong = true))
        at(8f, 1.4f, label("easy", day(5), 104f, Anchor.CENTER))
    }
}
