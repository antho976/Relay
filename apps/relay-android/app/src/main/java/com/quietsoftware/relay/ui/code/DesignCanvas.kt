package com.quietsoftware.relay.ui.code

import android.graphics.BlurMaskFilter
import android.graphics.Matrix
import android.graphics.Path.Op as PathOp
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.SweepGradient
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.Shader
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toAndroidRectF
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.BaselineShift
import androidx.compose.ui.text.style.LineBreak
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.design.CANVAS
import com.quietsoftware.relay.core.design.DesignGot
import com.quietsoftware.relay.core.design.DesignGradient
import com.quietsoftware.relay.core.design.DesignIcons
import com.quietsoftware.relay.core.design.DesignNode
import com.quietsoftware.relay.core.design.DesignScene
import com.quietsoftware.relay.core.design.DesignShadow
import com.quietsoftware.relay.core.design.DesignTextRun
import com.quietsoftware.relay.core.design.DesignValues
import com.quietsoftware.relay.core.design.MAX_SHADOW_BLUR
import com.quietsoftware.relay.core.design.Measure
import com.quietsoftware.relay.core.design.Omissions
import com.quietsoftware.relay.core.design.PLACED_LIMIT
import com.quietsoftware.relay.core.design.PathSegs
import com.quietsoftware.relay.core.design.Placed
import com.quietsoftware.relay.core.design.Pt
import com.quietsoftware.relay.core.design.Rect
import com.quietsoftware.relay.core.design.STICKY_PAD
import com.quietsoftware.relay.core.design.STROKE
import com.quietsoftware.relay.core.design.Seg
import com.quietsoftware.relay.core.design.TextSize
import com.quietsoftware.relay.core.design.alphaOf
import com.quietsoftware.relay.core.design.arcEnds
import com.quietsoftware.relay.core.design.arcOf
import com.quietsoftware.relay.core.design.arrowheads
import com.quietsoftware.relay.core.design.closedShape
import com.quietsoftware.relay.core.design.cornerRadii
import com.quietsoftware.relay.core.design.dashIntervals
import com.quietsoftware.relay.core.design.decodeDesignGot
import com.quietsoftware.relay.core.design.effectiveShadows
import com.quietsoftware.relay.core.design.fillOf
import com.quietsoftware.relay.core.design.fillableLine
import com.quietsoftware.relay.core.design.fitRect
import com.quietsoftware.relay.core.design.fontFamily
import com.quietsoftware.relay.core.design.fontSize
import com.quietsoftware.relay.core.design.fontWeight
import com.quietsoftware.relay.core.design.groundOf
import com.quietsoftware.relay.core.design.labelBox
import com.quietsoftware.relay.core.design.lineHeight
import com.quietsoftware.relay.core.design.linePoints
import com.quietsoftware.relay.core.design.linearEnds
import com.quietsoftware.relay.core.design.nodePolygon
import com.quietsoftware.relay.core.design.omissions
import com.quietsoftware.relay.core.design.paintedBounds
import com.quietsoftware.relay.core.design.parseColor
import com.quietsoftware.relay.core.design.pathSegments
import com.quietsoftware.relay.core.design.place
import com.quietsoftware.relay.core.design.rotation
import com.quietsoftware.relay.core.design.routed
import com.quietsoftware.relay.core.design.strokeCap
import com.quietsoftware.relay.core.design.strokeJoin
import com.quietsoftware.relay.core.design.textAlign
import com.quietsoftware.relay.core.design.textCaseIndexed
import com.quietsoftware.relay.core.design.textColor
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.conflate
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.serialization.json.JsonObject
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min

/*
 * A design page drawn on the phone as the PC draws it (apps/relay-native/src/design/paint.rs),
 * from the nodes `design.get` gives: laid out by the port of the PC's layout (core/design), its
 * words set by Compose in the fonts the app carries, then painted by the PC's rules in page units
 * under one fit transform. Each answer is decoded, laid out and set once, off the main thread
 * ([designPlans]); drawing only reads the plan.
 *
 * What the phone leaves out, each counted in the page's caption (`Omissions`): a layer's blur,
 * the shadow a text, line, path or icon casts (a glow), SVG pictures, kinds and layouts from a
 * newer Relay, layers nested past MAX_DEPTH, and motion (the phone shows a page at rest).
 */

/** One page as the phone draws it: laid out, its words set, the boxes its layers paint in. */
class PagePlan internal constructor(
    val id: String,
    /** The page's name, or "Page N". */
    val name: String,
    val scene: DesignScene,
    val placed: Placed,
    /** Everything on the page as painted, the box the canvas fits; null for nothing. */
    val bounds: Rect?,
    /** The colour the page shows behind what is on it. */
    val ground: Color,
    internal val words: Map<String, Words>,
    /** Where a see-through or blending layer paints, drawn on its own: in its parent's frame, turned as it sits there. */
    internal val layers: Map<String, Rect>,
    /** Where what a node holds paints, its ground with it, when one of its children blends: in the node's own frame, before its turn. */
    internal val held: Map<String, Rect>,
    internal val icons: Map<String, Path?>,
    /** The page's top-level frames, shown, at the size they are drawn. */
    val frames: List<FrameRow>,
    val omissions: Omissions,
    /** Laying it out failed here: the page is not drawn. */
    val failed: Boolean = false,
)

/** A top-level frame of a page, as its frames list shows it: its name and its size as drawn (B56). */
data class FrameRow(val id: String, val name: String, val w: Double, val h: Double) {
    val size: String get() = "${w.roundPx()} × ${h.roundPx()}"
}

private fun Double.roundPx(): Long = if (isFinite()) Math.round(this) else 0

/** A node's words set to draw: the layout, where its top-left goes and its colour. */
internal class Words(val layout: TextLayoutResult, val x: Double, val y: Double, val color: Color)

/** A `design.get` answer as the phone shows it: the reply, the design it reads as, and each page planned. */
data class DesignPlan(val live: Live, val got: DesignGot?, val pages: List<PagePlan>) {
    /** An answer came, but not as a design this phone can read. */
    val unreadable: Boolean get() = live.result != null && got == null
}

/** Plans for `design.get` with [payload] (a whole design, or one page), each answer planned once off the main thread; null until the first. */
@Composable
fun rememberDesignPlan(nav: Nav, payload: JsonObject?): DesignPlan? {
    val resolver = LocalFontFamilyResolver.current
    val flow = remember(payload, resolver) { payload?.let { designPlans(nav.relay.live("design.get", it), resolver) } ?: emptyFlow() }
    return flow.collectAsStateWithLifecycle(null).value
}

/**
 * [source]'s answers planned on [Dispatchers.Default]: decoded, laid out and their words set, one
 * at a time, the latest waiting while one is planned (a run of edits costs two plans, not one each);
 * an answer that only changes how fresh the reply is keeps the plan it has.
 */
fun designPlans(source: Flow<Live>, resolver: FontFamily.Resolver): Flow<DesignPlan> = flow {
    var last: DesignPlan? = null
    source.conflate().collect { live ->
        val plan = last?.takeIf { it.live.result == live.result }?.copy(live = live) ?: planDesign(live, resolver)
        last = plan
        emit(plan)
    }
}.flowOn(Dispatchers.Default)

/** [live]'s answer decoded and every page planned with a fresh measure (call it off the main thread). */
fun planDesign(live: Live, resolver: FontFamily.Resolver): DesignPlan {
    val got = decodeDesignGot(live.result) ?: return DesignPlan(live, null, emptyList())
    return DesignPlan(live, got, planPages(got, ComposeMeasure(resolver)))
}

/** Each page of [got] planned, in order ([only]: that page alone). */
fun planPages(got: DesignGot, measure: ComposeMeasure, only: String? = null): List<PagePlan> {
    val scene = got.scene()
    return scene.pages.filter { only == null || it == only }.mapIndexed { i, page ->
        val name = scene.node(page)?.name?.ifBlank { null } ?: "Page ${i + 1}"
        try {
            planPage(scene, page, name, measure)
        } catch (e: CancellationException) {
            throw e
        } catch (_: RuntimeException) {
            // A design this phone's port trips on is said, not a crash: the PC still draws it.
            PagePlan(page, name, scene, place(DesignScene(emptyList()), page, measure), null, colorOf(CANVAS)!!, emptyMap(), emptyMap(), emptyMap(), emptyMap(), emptyList(), Omissions(), failed = true)
        }
    }
}

/** [page] laid out, its words set at the widths layout gave them and its layers bounded. */
internal fun planPage(scene: DesignScene, page: String, name: String, measure: ComposeMeasure): PagePlan {
    val placed = place(scene, page, measure)
    val words = HashMap<String, Words>()
    val layers = HashMap<String, Rect>()
    val held = HashMap<String, Rect>()
    val icons = HashMap<String, Path?>()
    val todo = ArrayDeque(scene.children(page))
    while (todo.isNotEmpty()) {
        val id = todo.removeLast()
        val n = scene.node(id) ?: continue
        val r = placed.rect(id) ?: continue
        if (n.isHidden || (n.opacity ?: 1.0) <= 0.0) continue
        if ((n.opacity ?: 1.0) < 1.0 || blendOf(n) != null) paintedBounds(scene, placed, measure, id)?.let { layers[id] = it }
        if (scene.children(id).any { blends(scene, it) }) held[id] = heldBounds(scene, placed, measure, n, r)
        when (n.kind) {
            "text" -> if (!n.text.isNullOrEmpty()) {
                val wrap = placed.wrapWidth(n, r)
                words[id] = Words(measure.layout(n, wrap, textAlign(n)), r.x, r.y, textColorOf(scene, n))
            }
            "sticky" -> if (!n.text.isNullOrEmpty()) {
                val wrap = placed.wrapWidth(n, r)
                words[id] = Words(measure.layout(n, wrap, textAlign(n)), r.x + STICKY_PAD, r.y + STICKY_PAD, textColorOf(scene, n))
            }
            "rect", "ellipse", "polygon" -> if (!n.text.isNullOrEmpty()) {
                // A label, centred both ways inside the shape (shapes.rs `label_box`).
                val box = labelBox(n, r) { measure.measure(n, it).h }
                words[id] = Words(measure.layout(n, box.w, textAlign(n)), box.x, box.y, textColorOf(scene, n))
            }
            "icon" -> iconData(n)?.let { d -> if (d !in icons) icons[d] = iconPath(d) }
        }
        todo.addAll(scene.children(id))
    }
    val bounds = paintedBounds(scene, placed, measure, page)?.let { b ->
        // A page of one level line or one point still has room to show it.
        if (b.w > 0.0 && b.h > 0.0) b else b.inflate(1.0)
    }
    if (bounds != null && scene.children(page).any { blends(scene, it) }) held[page] = bounds
    val frames = scene.children(page).mapNotNull { id ->
        val n = scene.node(id)?.takeIf { it.kind == "frame" && !it.isHidden } ?: return@mapNotNull null
        val r = placed.rect(id) ?: return@mapNotNull null
        FrameRow(id, n.name?.ifBlank { null } ?: "Frame", r.w, r.h)
    }
    val ground = scene.node(page)?.let(::groundOf)?.let(::colorOf)?.takeIf { it.alpha > 0f } ?: colorOf(CANVAS)!!
    return PagePlan(page, name, scene, placed, bounds, ground, words, layers, held, icons, frames, omissions(scene, page))
}

/**
 * Where what [n] (placed at [r]) holds paints, its ground with it, in its own frame before its
 * turn: the layer its children blend in is opened inside the turn, so it is bounded there, not
 * by [paintedBounds], which is the box as it sits turned on the page and would cut what it holds
 * to a strip. A clipping frame's box holds it all; otherwise its children as they paint, and a
 * frame's ground in its box.
 */
private fun heldBounds(scene: DesignScene, placed: Placed, measure: Measure, n: DesignNode, r: Rect): Rect {
    if (n.kind == "frame" && n.clip != false) return r
    val kids = scene.children(n.id)
        .filter { c -> scene.node(c)?.let { k -> !k.isHidden && (k.opacity ?: 1.0) > 0.0 } == true }
        .mapNotNull { paintedBounds(scene, placed, measure, it) }
        .reduceOrNull { a, b -> a.union(b) }
    return if (n.kind == "frame") kids?.union(r) ?: r else kids ?: r
}

/** Whether [id] is a shown node that blends with what is under it. */
private fun blends(scene: DesignScene, id: String): Boolean = scene.node(id)?.let { !it.isHidden && blendOf(it) != null } == true

private fun textColorOf(scene: DesignScene, n: DesignNode): Color = colorOf(textColor(scene, n)) ?: Color.Black

/** An icon's path data as a path on its 24-unit grid. */
internal fun iconPath(data: String): Path? = runCatching { PathParser().parsePathString(data).toPath() }.getOrNull()

/** What an icon node draws: its own outline (a saved icon's), else the set's icon it names (paint.rs `icon`). */
internal fun iconData(n: DesignNode): String? = n.iconPath ?: DesignIcons.path(n.icon)

/** `#RGB`, `#RRGGBB` or `#RRGGBBAA` as a colour; null for anything else. */
internal fun colorOf(hex: String?): Color? = parseColor(hex)?.let { Color(it) }

/** The colour the PC draws a written colour in: black when it does not parse (paint.rs `rgba`). */
private fun colorOrBlack(hex: String?): Color = colorOf(hex) ?: Color.Black

/** A node's `blend` as a drawing mode; null for normal or a mode this phone does not know (drawn normally). */
internal fun blendOf(n: DesignNode): BlendMode? = when (n.blend?.takeIf { it in DesignValues.blends }) {
    "multiply" -> BlendMode.Multiply
    "screen" -> BlendMode.Screen
    "overlay" -> BlendMode.Overlay
    "darken" -> BlendMode.Darken
    "lighten" -> BlendMode.Lighten
    "color_dodge" -> BlendMode.ColorDodge
    "color_burn" -> BlendMode.ColorBurn
    "hard_light" -> BlendMode.Hardlight
    "soft_light" -> BlendMode.Softlight
    "difference" -> BlendMode.Difference
    "exclusion" -> BlendMode.Exclusion
    "hue" -> BlendMode.Hue
    "saturation" -> BlendMode.Saturation
    "color" -> BlendMode.Color
    "luminosity" -> BlendMode.Luminosity
    else -> null
}

// ---- Words ----

/**
 * Words measured and set as the PC's Pango does them (paint.rs `Painter::new_layout` and its
 * `Measure`), with Compose's text in one px a unit (one sp is one design px): the node's family,
 * size, weight, slant, underline, strike, letter spacing and line height (a multiple of its size,
 * every line that tall), its case, its runs each over its stretch, wrapped greedily at the width
 * layout gives, at most `max_lines` lines ending in "…". A family the app does not carry is the
 * phone's sans serif, and the phone's fonts measure a little otherwise than the PC's, so a line
 * may break a word earlier or later than there.
 *
 * Not thread-safe: one plan's thread uses one, and drawing reads the layouts the plan keeps.
 */
class ComposeMeasure(resolver: FontFamily.Resolver) : Measure {
    private val measurer = TextMeasurer(resolver, Density(1f, 1f), LayoutDirection.Ltr, cacheSize = 0)
    private val layouts = HashMap<Key, TextLayoutResult>()

    /** The node itself, not its id: a probe with other words or a cut must not read another's layout. */
    private data class Key(val node: DesignNode, val wrap: Long?, val align: String)

    /** [n]'s words wrapped at [wrap] px (unwrapped for null), lines aligned by [align]; a text with no words is set as one space. */
    fun layout(n: DesignNode, wrap: Double?, align: String): TextLayoutResult = layouts.getOrPut(Key(n, wrap?.toRawBits(), align)) {
        val cut = n.maxLines?.takeIf { wrap != null }
        measurer.measure(
            text = if (n.text.isNullOrEmpty()) AnnotatedString(" ") else wordsOf(n),
            style = styleOf(n, align),
            overflow = if (cut != null) TextOverflow.Ellipsis else TextOverflow.Clip,
            softWrap = wrap != null,
            maxLines = cut?.coerceAtLeast(1) ?: Int.MAX_VALUE,
            constraints = wrap?.let { val w = floor(it).coerceIn(1.0, MAX_WRAP).toInt(); Constraints.fitPrioritizingWidth(w, w, 0, Constraints.Infinity) } ?: Constraints(),
        )
    }

    /** The PC's measure (paint.rs `Measure for Painter`): the widest line plus a px, as tall as its lines; no words are 2 px wide and a line tall. */
    override fun measure(n: DesignNode, wrap: Double?): TextSize {
        val t = layout(n, wrap, textAlign(n))
        val w = if (n.text.isNullOrEmpty()) 2.0 else max(1.0, ceil(widest(t))) + 1.0
        return TextSize(w, max(1, t.size.height).toDouble())
    }

    private fun widest(t: TextLayoutResult): Double {
        var most = 0f
        for (i in 0 until t.lineCount) most = max(most, t.getLineRight(i) - t.getLineLeft(i))
        return most.toDouble()
    }

    private companion object {
        /** The widest wrap Compose's constraints hold; words wrapped wider are as good as unwrapped. */
        const val MAX_WRAP = 200_000.0
    }
}

/** The family the phone sets [name] in: the PC's bundled ones it carries too, else its own sans serif. */
internal fun familyOf(name: String?): FontFamily = when (name) {
    "Geist" -> Fonts.Geist
    "Geist Mono" -> Fonts.GeistMono
    "Sora" -> Fonts.Sora
    "Fira Mono" -> Fonts.FiraMono
    "Fira Sans" -> Fonts.FiraSans
    "Fira Sans Condensed" -> Fonts.FiraCondensed
    else -> FontFamily.SansSerif
}

/** A weight as Pango draws it, in nine steps (paint.rs `weight`): 450 is medium, 1000 heavy. */
internal fun pangoWeight(w: Int): Int = when {
    w < 150 -> 100
    w < 250 -> 200
    w < 350 -> 300
    w < 450 -> 400
    w < 550 -> 500
    w < 650 -> 600
    w < 750 -> 700
    w < 850 -> 800
    else -> 900
}

private fun decoration(underline: Boolean, strike: Boolean): TextDecoration = when {
    underline && strike -> TextDecoration.combine(listOf(TextDecoration.Underline, TextDecoration.LineThrough))
    underline -> TextDecoration.Underline
    strike -> TextDecoration.LineThrough
    else -> TextDecoration.None
}

/**
 * The node's look for its words. Every line is `line_height` times the size tall, as Pango's
 * absolute line height; where a run is larger, every line takes its height, so its line never
 * overlaps the next (Pango raises that line alone).
 */
private fun styleOf(n: DesignNode, align: String): TextStyle {
    val largest = n.runs.orEmpty().mapNotNull { it.fontSize }.fold(fontSize(n), ::max)
    return TextStyle(
        fontFamily = familyOf(fontFamily(n)),
        fontSize = fontSize(n).sp,
        fontWeight = FontWeight(pangoWeight(fontWeight(n))),
        fontStyle = if (n.italic == true) FontStyle.Italic else FontStyle.Normal,
        textDecoration = decoration(n.underline == true, n.strike == true),
        letterSpacing = (n.letterSpacing ?: 0.0).sp,
        lineHeight = (lineHeight(n) * largest).sp,
        lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
        textAlign = when (align) {
            "center" -> TextAlign.Center
            "right" -> TextAlign.Right
            else -> TextAlign.Left
        },
        lineBreak = LineBreak.Simple,
        platformStyle = PlatformTextStyle(includeFontPadding = false),
    )
}

/**
 * [n]'s words as drawn (paint.rs `shown_text`): in its case, less any control character but tabs
 * and line breaks (B43), each run's look over its stretch. A run counts characters of the stored
 * words (Unicode scalar values), found again in the cased ones; one past the words is cut short.
 */
internal fun wordsOf(n: DesignNode): AnnotatedString {
    val (cased, starts) = textCaseIndexed(n.textCase, n.text.orEmpty())
    val (shown, moved) = withoutControls(cased)
    val runs = n.runs.orEmpty()
    if (runs.isEmpty()) return AnnotatedString(shown)
    val last = starts.size - 1
    val b = AnnotatedString.Builder(shown)
    for (run in runs) {
        val s = moved(starts[run.start.coerceIn(0, last)])
        val e = moved(starts[run.end.coerceIn(0, last)])
        if (s < e) b.addStyle(spanOf(n, run), s, e)
    }
    return b.toAnnotatedString()
}

/** [text] without control characters but `\t`, `\n` and `\r` (design.rs `strip_controls`), and where each index of [text] lands in it. */
internal fun withoutControls(text: String): Pair<String, (Int) -> Int> {
    fun bad(c: Char) = (c < ' ' && c != '\t' && c != '\n' && c != '\r') || c == '\u007f'
    if (text.none(::bad)) return text to { i -> i }
    val out = StringBuilder(text.length)
    val moved = IntArray(text.length + 1)
    for ((i, c) in text.withIndex()) {
        moved[i] = out.length
        if (!bad(c)) out.append(c)
    }
    moved[text.length] = out.length
    return out.toString() to { i -> moved[i.coerceIn(0, text.length)] }
}

/**
 * What a run sets of its own (paint.rs `run_attrs`): colour, family, size, weight, slant,
 * underline and strike (each over the node's), rise, in px up (Compose shifts by a share of
 * the font's ascent, taken here as nine tenths of its size), and letter spacing, as the node's is.
 */
private fun spanOf(n: DesignNode, run: DesignTextRun): SpanStyle {
    val size = run.fontSize ?: fontSize(n)
    return SpanStyle(
        color = run.color?.let(::colorOf) ?: Color.Unspecified,
        fontSize = run.fontSize?.sp ?: TextUnit.Unspecified,
        fontWeight = run.fontWeight?.let { FontWeight(pangoWeight(it)) },
        fontStyle = run.italic?.let { if (it) FontStyle.Italic else FontStyle.Normal },
        fontFamily = run.font?.let(::familyOf),
        textDecoration = if (run.underline != null || run.strike != null) decoration(run.underline ?: (n.underline == true), run.strike ?: (n.strike == true)) else null,
        baselineShift = run.rise?.takeIf { size > 0.0 }?.let { BaselineShift((it / (0.9 * size)).toFloat()) },
        letterSpacing = run.letterSpacing?.sp ?: TextUnit.Unspecified,
    )
}

// ---- Drawing ----

/** The page's canvas, fit to the column's width and at most [tallest] tall: everything on it, on its ground. */
@Composable
fun PageCanvas(plan: PagePlan, modifier: Modifier = Modifier, tallest: Dp = 360.dp, picture: (String) -> ImageBitmap? = { null }) {
    val box = plan.bounds ?: return
    val c = Relay.colors
    val density = LocalDensity.current
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val pad = with(density) { 12.dp.toPx() }
        val wide = constraints.maxWidth.toFloat()
        val high = with(density) { tallest.toPx() }
        val zoom = fitZoom(box, wide - 2 * pad, high - 2 * pad)
        val height = with(density) { (box.h.toFloat() * zoom + 2 * pad).toDp() }
        Canvas(Modifier.fillMaxWidth().height(height).clip(Radii.key).background(colorOf(CANVAS)!!).background(plan.ground).border(1.dp, c.edge, Radii.key)) {
            // Centred across; strokes at least a screen pixel at any zoom.
            val dx = (size.width - box.w.toFloat() * zoom) / 2 - box.x.toFloat() * zoom
            val dy = pad - box.y.toFloat() * zoom
            withTransform({
                translate(dx, dy)
                scale(zoom, zoom, Offset.Zero)
            }) {
                drawPage(plan, 1f / zoom, picture)
            }
        }
    }
}

/** The scale that fits [box] in [wide] by [high] screen px, never above [MOST_ZOOM] (a lone icon is not blown up past use). */
internal fun fitZoom(box: Rect, wide: Float, high: Float): Float =
    min(wide / box.w.toFloat(), high / box.h.toFloat()).coerceIn(0.0001f, MOST_ZOOM)

private const val MOST_ZOOM = 4f

/**
 * [plan]'s page in page units, by the PC's paint order (paint.rs `Painter::paint`), each stroke
 * at least [hair] page units wide (a screen pixel); [picture] gives a stored picture by its `src`
 * once the phone holds it, else its box shows a placeholder.
 */
fun DrawScope.drawPage(plan: PagePlan, hair: Float, picture: (String) -> ImageBitmap? = { null }) {
    if (plan.failed) return
    with(PagePainter(plan, hair, picture)) {
        children(plan.id) { plan.held[plan.id]?.let { drawRect(plan.ground, it.topLeft, it.size) } }
    }
}

private val Rect.topLeft: Offset get() = Offset(x.toFloat(), y.toFloat())
private val Rect.size: Size get() = Size(w.toFloat(), h.toFloat())
private fun Rect.g(): androidx.compose.ui.geometry.Rect = androidx.compose.ui.geometry.Rect(x.toFloat(), y.toFloat(), right.toFloat(), bottom.toFloat())
private fun Pt.o(): Offset = Offset(x.toFloat(), y.toFloat())

/**
 * A box with rounded corners, each an ellipse's quarter [rx] by [ry] (top-left, top-right,
 * bottom-right, bottom-left): a node's outline as GSK's rounded rect holds it, an ellipse being one
 * whose corners are half its sides (paint.rs `outline`).
 */
internal class RoundBox(val rect: Rect, val rx: DoubleArray, val ry: DoubleArray) {
    /**
     * Grown by [by] on every side (shrunk for a negative one) as GSK spreads and insets a rounded
     * box (`gsk_rounded_rect_shrink`): a rounded corner grows or shrinks with it, a square one
     * stays square, and one shrunk to nothing either way is square.
     */
    fun grown(by: Double): RoundBox {
        val gx = DoubleArray(4)
        val gy = DoubleArray(4)
        for (i in 0 until 4) {
            val x = if (rx[i] > 0.0) rx[i] + by else rx[i]
            val y = if (ry[i] > 0.0) ry[i] + by else ry[i]
            if (x > 0.0 && y > 0.0) {
                gx[i] = x
                gy[i] = y
            }
        }
        return RoundBox(Rect(rect.x - by, rect.y - by, max(rect.w + 2 * by, 0.0), max(rect.h + 2 * by, 0.0)), gx, gy)
    }

    fun moved(dx: Double, dy: Double): RoundBox = RoundBox(rect.translate(dx, dy), rx, ry)

    fun path(): Path = Path().apply {
        fun corner(i: Int) = CornerRadius(rx[i].toFloat(), ry[i].toFloat())
        addRoundRect(RoundRect(rect.x.toFloat(), rect.y.toFloat(), rect.right.toFloat(), rect.bottom.toFloat(), corner(0), corner(1), corner(2), corner(3)))
    }
}

/** [n]'s outline in its box [r]: an ellipse, or its box with its corners rounded (`radii`, else `radius`, a sticky's 6). */
internal fun outline(n: DesignNode, r: Rect): RoundBox {
    if (n.kind == "ellipse") return RoundBox(r, DoubleArray(4) { r.w / 2 }, DoubleArray(4) { r.h / 2 })
    val radii = cornerRadii(n, r)
    return RoundBox(r, radii, radii.copyOf())
}

/** A polygon's or star's outline (its corners from shapes.rs `node_polygon`, flip included). */
internal fun polygonPath(n: DesignNode, r: Rect): Path = Path().apply {
    val pts = nodePolygon(n, r)
    pts.firstOrNull()?.let { moveTo(it.x.toFloat(), it.y.toFloat()) }
    for (p in pts.drop(1)) lineTo(p.x.toFloat(), p.y.toFloat())
    close()
}

/**
 * An ellipse's arc as a closed outline (paint.rs `arc_path`): a pie slice, or with a hole a ring
 * segment whose ends are shaped by `arc.cap` (`arcEnds`), or a whole ring (two outlines, the hole
 * cut out by the even-odd rule). Angles run clockwise from the top.
 */
internal fun arcPath(n: DesignNode, r: Rect): Path {
    val arc = arcOf(n) ?: return outline(n, r).path()
    val cx = r.cx
    val cy = r.cy
    val rx = r.w / 2
    val ry = r.h / 2
    if (arc.inner > 0.0 && abs(arc.sweep) >= 360.0) {
        return Path().apply {
            fillType = PathFillType.EvenOdd
            addOval(Rect(r.x, r.y, r.w, r.h).g())
            addOval(Rect(cx - rx * arc.inner, cy - ry * arc.inner, r.w * arc.inner, r.h * arc.inner).g())
        }
    }
    fun at(deg: Double, k: Double): Offset {
        val a = Math.toRadians(deg)
        return Offset((cx + rx * k * kotlin.math.sin(a)).toFloat(), (cy - ry * k * kotlin.math.cos(a)).toFloat())
    }
    fun oval(k: Double) = Rect(cx - rx * k, cy - ry * k, 2 * rx * k, 2 * ry * k).g()
    return Path().apply {
        val start = at(arc.start, 1.0)
        moveTo(start.x, start.y)
        // Compose counts angles from three o'clock; the model from twelve.
        arcTo(oval(1.0), (arc.start - 90).toFloat(), arc.sweep.toFloat(), false)
        if (arc.inner > 0.0) {
            val ends = arcEnds(n, r)
            val end = at(arc.start + arc.sweep, arc.inner)
            if (ends != null) capTo(ends.cap, ends.end, inward = true, clockwise = arc.sweep > 0) else lineTo(end.x, end.y)
            arcTo(oval(arc.inner), (arc.start + arc.sweep - 90).toFloat(), (-arc.sweep).toFloat(), false)
            if (ends != null) capTo(ends.cap, ends.start, inward = false, clockwise = arc.sweep > 0)
        } else {
            lineTo(cx.toFloat(), cy.toFloat())
        }
        close()
    }
}

/** Across one capped end of a ring segment, from its outer edge to its inner one ([inward]) or back (paint.rs `cap_to`). */
private fun Path.capTo(cap: String, end: com.quietsoftware.relay.core.design.ArcEnd, inward: Boolean, clockwise: Boolean) {
    val from = if (inward) end.outer else end.inner
    val to = if (inward) end.inner else end.outer
    val c = end.capRadius
    when (cap) {
        "round" -> {
            val mid = end.mid
            val startAngle = Math.toDegrees(atan2(from.y - mid.y, from.x - mid.x))
            arcTo(Rect(mid.x - c, mid.y - c, 2 * c, 2 * c).g(), startAngle.toFloat(), if (clockwise) 180f else -180f, false)
        }
        "square" -> {
            val ox = end.out.x * c
            val oy = end.out.y * c
            lineTo((from.x + ox).toFloat(), (from.y + oy).toFloat())
            lineTo((to.x + ox).toFloat(), (to.y + oy).toFloat())
            lineTo(to.x.toFloat(), to.y.toFloat())
        }
        else -> lineTo(to.x.toFloat(), to.y.toFloat())
    }
}

/** A stroke's pieces (shapes.rs `path_segments`) as a path, closed for a closed shape. */
internal fun segsPath(segs: PathSegs, closed: Boolean): Path = Path().apply {
    moveTo(segs.start.x.toFloat(), segs.start.y.toFloat())
    for (s in segs.segs) {
        when (s) {
            is Seg.Line -> lineTo(s.to.x.toFloat(), s.to.y.toFloat())
            is Seg.Quad -> quadraticTo(s.c.x.toFloat(), s.c.y.toFloat(), s.to.x.toFloat(), s.to.y.toFloat())
            is Seg.Cubic -> cubicTo(s.c1.x.toFloat(), s.c1.y.toFloat(), s.c2.x.toFloat(), s.c2.y.toFloat(), s.to.x.toFloat(), s.to.y.toFloat())
        }
    }
    if (closed) close()
}

/** The pen a node strokes with at [width] (paint.rs `stroke_of`): its cap, its join (or [join]), its dashes. */
internal fun penOf(n: DesignNode, width: Float, join: StrokeJoin = joinOf(n)): Stroke = Stroke(
    width = width,
    cap = when (strokeCap(n)) {
        "butt" -> StrokeCap.Butt
        "square" -> StrokeCap.Square
        else -> StrokeCap.Round
    },
    join = join,
    pathEffect = dashIntervals(n.dash)?.takeIf { it.sum() > 0.0 }?.let { d -> PathEffect.dashPathEffect(FloatArray(d.size) { d[it].toFloat() }) },
)

private fun joinOf(n: DesignNode): StrokeJoin = when (strokeJoin(n)) {
    "miter" -> StrokeJoin.Miter
    "bevel" -> StrokeJoin.Bevel
    else -> StrokeJoin.Round
}

/**
 * A gradient laid over [r] (paint.rs `gradient`): linear along its angle as CSS reads it, radial
 * as an ellipse filling the box, conic round the centre from its angle (0 at the top); its stops
 * in order. A kind this phone does not know paints its middle stop flat.
 */
internal fun brushOf(g: DesignGradient, r: Rect): Brush? {
    val stops = g.stops.sortedBy { it.at }
    if (stops.isEmpty()) return null
    if (stops.size == 1) return SolidColor(colorOrBlack(stops[0].color))
    val colors = stops.map { colorOrBlack(it.color) }
    val at = FloatArray(stops.size) { stops[it].at.toFloat().coerceIn(0f, 1f) }
    val cx = r.cx.toFloat()
    val cy = r.cy.toFloat()
    return when (g.kind) {
        "linear" -> {
            val (a, b) = linearEnds(g.angle, r)
            if (a == b) SolidColor(colors.last()) else Brush.linearGradient(*Array(stops.size) { at[it] to colors[it] }, start = a.o(), end = b.o(), tileMode = TileMode.Clamp)
        }
        "radial" -> {
            val rx = max(r.w / 2, 0.5).toFloat()
            val ry = max(r.h / 2, 0.5).toFloat()
            shaderBrush(RadialGradient(cx, cy, rx, IntArray(colors.size) { colors[it].toArgb() }, at, android.graphics.Shader.TileMode.CLAMP).apply {
                setLocalMatrix(Matrix().apply { setScale(1f, ry / rx, cx, cy) })
            })
        }
        "conic" -> shaderBrush(SweepGradient(cx, cy, IntArray(colors.size) { colors[it].toArgb() }, at).apply {
            setLocalMatrix(Matrix().apply { setRotate((g.angle ?: 0.0).toFloat() - 90f, cx, cy) })
        })
        else -> SolidColor(colorOrBlack(stops[stops.size / 2].color))
    }
}

private fun shaderBrush(shader: Shader): Brush = object : ShaderBrush() {
    override fun createShader(size: Size): Shader = shader
}

/** A shadow's blur as painted: within 0 to 200 (paint.rs `blur_of`). */
private fun blurOf(s: DesignShadow): Double = if (s.blur.isFinite()) s.blur.coerceIn(0.0, MAX_SHADOW_BLUR) else 0.0

/**
 * The radius Android's blur takes for the PC's blur [blur] px: GSK blurs by σ = blur / 2, and
 * Skia reads a radius r as σ = 0.57735 r + 0.5. Null for no blur.
 */
internal fun androidBlur(blur: Double): Float? {
    if (blur <= 0.0) return null
    return max(0.01, (blur / 2 - 0.5) / 0.57735).toFloat()
}

/** Paints one page: a member a node's drawing needs, with the plan and the hairline at hand. */
private class PagePainter(val plan: PagePlan, val hair: Float, val picture: (String) -> ImageBitmap?) {
    val scene get() = plan.scene
    val placed get() = plan.placed

    /**
     * [parent]'s children in order on [ground] (its fill and inner shadow). A child with a `blend`
     * mixes with what is painted before it under the same parent, its ground included, so they
     * are painted in a layer of their own (paint.rs `paint_children`), inside the parent's turn.
     */
    fun DrawScope.children(parent: String, ground: (DrawScope.() -> Unit)?) {
        val kids = scene.children(parent)
        if (kids.none { blends(scene, it) }) {
            ground?.invoke(this)
            for (k in kids) paint(k)
            return
        }
        layer(plan.held[parent]) {
            ground?.invoke(this)
            for (k in kids) paint(k)
        }
    }

    /** One node and what is inside it: see-through and blending in a layer, turned about its box's centre. */
    fun DrawScope.paint(id: String) {
        val n = scene.node(id)?.takeIf { !it.isHidden } ?: return
        val r = placed.rect(id) ?: return
        val opacity = n.opacity ?: 1.0
        if (opacity <= 0.0) return
        val blend = blendOf(n)
        if (opacity < 1.0 || blend != null) {
            layer(plan.layers[id], opacity.coerceAtMost(1.0).toFloat(), blend ?: BlendMode.SrcOver) { turned(n, r) }
        } else {
            turned(n, r)
        }
    }

    fun DrawScope.turned(n: DesignNode, r: Rect) {
        val turn = rotation(n)
        if (turn != 0.0) rotate(turn.toFloat(), Offset(r.cx.toFloat(), r.cy.toFloat())) { body(n, r) } else body(n, r)
    }

    fun DrawScope.body(n: DesignNode, r: Rect) {
        when {
            n.kind == "group" -> children(n.id, null)
            n.kind == "frame" -> frame(n, r)
            n.kind == "ellipse" && arcOf(n) != null -> {
                shape(n, r, arcPath(n, r))
                words(n)
            }
            n.kind == "rect" || n.kind == "sticky" || n.kind == "ellipse" -> {
                // A sticky's own shadow, unless it names one, is among its effective shadows (B37).
                box(n, r)
                border(n, r)
                words(n)
            }
            n.kind == "polygon" -> {
                shape(n, r, polygonPath(n, r))
                words(n)
            }
            // Its glow (the shadow it casts) is not drawn here; the caption counts it.
            n.kind == "icon" -> flipped(n, r) { icon(n, r) }
            n.kind == "text" -> {
                colorOf(n.fill)?.let { drawRect(it, r.topLeft, r.size) }
                words(n)
            }
            n.kind == "line" || n.kind == "path" -> line(n, r)
            n.kind == "image" -> image(n, r)
        }
    }

    /** A frame: its shadows, then inside its rounded clip its fill, its inner shadows (over the fill, under its children, B93) and its children, then its border. */
    fun DrawScope.frame(n: DesignNode, r: Rect) {
        val box = outline(n, r)
        val shadows = effectiveShadows(n)
        outerShadows(box, shadows)
        val ground: DrawScope.() -> Unit = {
            fill(n, box)
            innerShadows(box, shadows)
        }
        if (n.clip != false) clipPath(box.path()) { children(n.id, ground) } else children(n.id, ground)
        border(n, r)
    }

    /** Shadow, fill (a colour or a gradient) and inner shadow, inside the rounded outline. */
    fun DrawScope.box(n: DesignNode, r: Rect) {
        val box = outline(n, r)
        val shadows = effectiveShadows(n)
        outerShadows(box, shadows)
        fill(n, box)
        innerShadows(box, shadows)
    }

    fun DrawScope.fill(n: DesignNode, box: RoundBox) {
        val g = n.gradient
        if (g != null) {
            brushOf(g, box.rect)?.let { drawPath(box.path(), it) }
        } else {
            colorOf(fillOf(n))?.takeIf { it.alpha > 0f }?.let { drawPath(box.path(), it) }
        }
    }

    /** The drop shadows under the outline, in order, each drawn outside it only (GSK's outset shadow). */
    fun DrawScope.outerShadows(box: RoundBox, shadows: List<DesignShadow>) {
        for (s in shadows) {
            if (s.inset) continue
            val c = colorOrBlack(s.color).takeIf { it.alpha > 0f } ?: continue
            val cast = box.grown(s.spread).moved(s.x, s.y)
            clipPath(box.path(), ClipOp.Difference) { blurred(cast.path().asAndroidPath(), c, blurOf(s)) }
        }
    }

    /** The inner shadows inside the outline, in order: everything outside the outline shrunk by the spread and moved, blurred, kept inside. */
    fun DrawScope.innerShadows(box: RoundBox, shadows: List<DesignShadow>) {
        for (s in shadows) {
            if (!s.inset) continue
            val c = colorOrBlack(s.color).takeIf { it.alpha > 0f } ?: continue
            val blur = blurOf(s)
            val around = box.rect.inflate(abs(s.x) + abs(s.y) + blur * 2 + abs(s.spread) + 1)
            val shade = Path().apply {
                fillType = PathFillType.EvenOdd
                addRect(around.g())
                addPath(box.grown(-s.spread).moved(s.x, s.y).path())
            }
            clipPath(box.path()) { blurred(shade.asAndroidPath(), c, blur) }
        }
    }

    /**
     * A shape drawn from a path (a polygon, an arc; paint.rs `shape`): its drop shadows, fill,
     * inner shadows and stroke, the stroke inside the edge as a box's border is (B128).
     */
    fun DrawScope.shape(n: DesignNode, r: Rect, path: Path) {
        val shadows = effectiveShadows(n)
        for (s in shadows) if (!s.inset) pathShadow(path, s)
        fillPath(n, path, r)
        for (s in shadows) if (s.inset) pathInsetShadow(path, s, r)
        val w = n.strokeWidth ?: 1.0
        if (w > 0.0 && (n.stroke != null || n.strokeGradient != null)) {
            // Twice as wide, clipped to the inside.
            clipPath(path) { strokeWith(n, path, penOf(n, max(2 * w.toFloat(), 2 * hair)), n.stroke?.let(::colorOrBlack), r) }
        }
    }

    fun DrawScope.fillPath(n: DesignNode, path: Path, r: Rect) {
        val g = n.gradient
        if (g != null) {
            brushOf(g, r)?.let { drawPath(path, it) }
        } else {
            colorOf(fillOf(n))?.takeIf { it.alpha > 0f }?.let { drawPath(path, it) }
        }
    }

    /**
     * A drop shadow under the shape [path] (paint.rs `path_shadow`): the shape moved by the
     * offset, grown by a positive spread or less a band along its edge for a negative one, taken
     * as one outline and blurred once, as the PC blurs the whole of it.
     */
    fun DrawScope.pathShadow(path: Path, s: DesignShadow) {
        val c = colorOrBlack(s.color).takeIf { it.alpha > 0f } ?: return
        val shape = path.asAndroidPath()
        val cast = when {
            s.spread > 0.0 -> joined(shape, band(shape, 2 * s.spread), PathOp.UNION)
            s.spread < 0.0 -> joined(shape, band(shape, -2 * s.spread), PathOp.DIFFERENCE)
            else -> shape
        }
        translate(s.x.toFloat(), s.y.toFloat()) { blurred(cast, c, blurOf(s)) }
    }

    /**
     * An inner shadow inside the shape [path] (its box [r]; paint.rs `path_inset_shadow`):
     * everything outside the shape, moved, the shape grown inwards by a positive spread (outwards
     * by a negative one), taken as one outline, blurred once and kept inside.
     */
    fun DrawScope.pathInsetShadow(path: Path, s: DesignShadow, r: Rect) {
        val c = colorOrBlack(s.color).takeIf { it.alpha > 0f } ?: return
        val blur = blurOf(s)
        val around = r.inflate(max(abs(s.x), abs(s.y)) + blur * 2 + abs(s.spread) + 1)
        val shape = path.asAndroidPath()
        val hole = if (s.spread < 0.0) joined(shape, band(shape, -2 * s.spread), PathOp.UNION) else shape
        val outside = joined(android.graphics.Path().apply { addRect(around.g().toAndroidRectF(), android.graphics.Path.Direction.CW) }, hole, PathOp.DIFFERENCE)
        val shade = if (s.spread > 0.0) joined(outside, band(shape, 2 * s.spread), PathOp.UNION) else outside
        clipPath(path) {
            translate(s.x.toFloat(), s.y.toFloat()) { blurred(shade, c, blur) }
        }
    }

    /** [path] filled in [color], blurred by the PC's [blur]. */
    fun DrawScope.blurred(path: android.graphics.Path, color: Color, blur: Double) {
        drawIntoCanvas { canvas ->
            val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                this.color = color.toArgb()
                androidBlur(blur)?.let { maskFilter = BlurMaskFilter(it, BlurMaskFilter.Blur.NORMAL) }
            }
            canvas.nativeCanvas.drawPath(path, paint)
        }
    }

    /**
     * The border inside the edge (paint.rs `border`): solid, the outline stroked twice as wide
     * inside it; dashed or in a gradient, a stroke along the outline inset by half its width
     * (square corners when not dashed). Nothing for a width of 0 (B128).
     */
    fun DrawScope.border(n: DesignNode, r: Rect) {
        if (n.stroke == null && n.strokeGradient == null) return
        n.strokeWidths?.takeIf { it.size == 4 }?.let { return borderSides(n, r, it) }
        val w = n.strokeWidth ?: 1.0
        if (w <= 0.0) return
        val color = n.stroke?.let(::colorOrBlack)
        val box = outline(n, r)
        if (n.dash != null || n.strokeGradient != null) {
            val half = minOf(w / 2, r.w / 2, r.h / 2)
            val pen = if (n.dash == null) penOf(n, max(w.toFloat(), hair), StrokeJoin.Miter) else penOf(n, max(w.toFloat(), hair))
            strokeWith(n, box.grown(-half).path(), pen, color, r)
        } else if (color != null) {
            val path = box.path()
            clipPath(path) { drawPath(path, color, style = Stroke(max(2 * w.toFloat(), 2 * hair))) }
        }
    }

    /**
     * A border of [sides] (`stroke_widths`: top, right, bottom, left) inside the edge, kept inside
     * the outline (paint.rs `border_sides`): dashed, each side with a width a stroke along its
     * middle; else the band between the box and the box inset by each side, in the stroke's
     * colour or gradient (the PC rounds the band's inner corners with the outline's).
     */
    fun DrawScope.borderSides(n: DesignNode, r: Rect, sides: List<Double>) {
        val (t, right, b, l) = sides.map { if (it.isFinite()) it.coerceIn(0.0, 1000.0) else 0.0 }
        if (t <= 0.0 && right <= 0.0 && b <= 0.0 && l <= 0.0) return
        val color = n.stroke?.let(::colorOrBlack)
        clipPath(outline(n, r).path()) {
            if (n.dash != null) {
                val lines = listOf(
                    Triple(t, Pt(r.x, r.y + t / 2), Pt(r.right, r.y + t / 2)),
                    Triple(right, Pt(r.right - right / 2, r.y), Pt(r.right - right / 2, r.bottom)),
                    Triple(b, Pt(r.right, r.bottom - b / 2), Pt(r.x, r.bottom - b / 2)),
                    Triple(l, Pt(r.x + l / 2, r.bottom), Pt(r.x + l / 2, r.y)),
                )
                for ((w, from, to) in lines) {
                    if (w <= 0.0) continue
                    val line = Path().apply { moveTo(from.x.toFloat(), from.y.toFloat()); lineTo(to.x.toFloat(), to.y.toFloat()) }
                    strokeWith(n, line, penOf(n, max(w.toFloat(), hair)), color, r)
                }
            } else {
                val band = Path().apply {
                    fillType = PathFillType.EvenOdd
                    addRect(r.g())
                    addRect(Rect(r.x + l, r.y + t, max(r.w - l - right, 0.0), max(r.h - t - b, 0.0)).g())
                }
                val g = n.strokeGradient
                if (g != null) brushOf(g, r)?.let { drawPath(band, it) } else if (color != null) drawPath(band, color)
            }
        }
    }

    /** Stroke [path] with [pen]: in the node's `stroke_gradient` laid over [area] (else the stroke's own bounds), else in [color]. */
    fun DrawScope.strokeWith(n: DesignNode, path: Path, pen: Stroke, color: Color?, area: Rect?) {
        val g = n.strokeGradient
        if (g != null) {
            brushOf(g, area ?: Rect.from(path).inflate(pen.width / 2.0))?.let { drawPath(path, it, style = pen) }
        } else if (color != null) {
            drawPath(path, color, style = pen)
        }
    }

    /** A node's words, where the plan set them. */
    fun DrawScope.words(n: DesignNode) {
        val w = plan.words[n.id] ?: return
        drawText(w.layout, color = w.color, topLeft = Offset(w.x.toFloat(), w.y.toFloat()))
    }

    /** An icon from the set, or its own outline, scaled to its box's shorter side and stroked in its colour; a grey tile for a name the phone lacks (paint.rs `icon`). */
    fun DrawScope.icon(n: DesignNode, r: Rect) {
        val path = iconData(n)?.let { plan.icons[it] }
        if (path == null) {
            drawRect(Color(0.55f, 0.55f, 0.55f, 0.3f), r.topLeft, r.size)
            return
        }
        val width = n.strokeWidth ?: 2.0
        if (width <= 0.0) return
        val color = colorOrBlack(n.stroke ?: textColor(scene, n))
        val s = max(min(r.w, r.h) / 24.0, 0.01).toFloat()
        withTransform({
            translate((r.cx - 12.0 * s).toFloat(), (r.cy - 12.0 * s).toFloat())
            scale(s, s, Offset.Zero)
        }) {
            // Weight and dashes in the icon's own grid, so they keep its proportions (B96).
            drawPath(path, color, style = penOf(n, max(width.toFloat(), hair / s)))
        }
    }

    /**
     * A line or path (paint.rs `stroke`): its fill (three or more free points), its stroke and
     * its arrowheads. A connector runs from its ends (`linePoints`, B5); a weight of 0, or a
     * stroke wholly clear, draws no stroke and no heads; a closed shape strokes inside its edge.
     */
    fun DrawScope.line(n: DesignNode, r: Rect) {
        val path = if (routed(n)) {
            val lp = linePoints(scene, placed, n)
            if (lp.points.size < 2) return
            Path().apply {
                val p = lp.points
                moveTo(p[0].x.toFloat(), p[0].y.toFloat())
                if (lp.curved) {
                    cubicTo(p[1].x.toFloat(), p[1].y.toFloat(), p[2].x.toFloat(), p[2].y.toFloat(), p[3].x.toFloat(), p[3].y.toFloat())
                } else {
                    for (q in p.drop(1)) lineTo(q.x.toFloat(), q.y.toFloat())
                }
            }
        } else {
            segsPath(pathSegments(n, Pt(r.x, r.y)) ?: return, closedShape(n))
        }
        if (fillableLine(n) && (n.fill != null || n.gradient != null)) {
            // A gradient spans the box, or the points where an agent left the box empty.
            fillPath(n, path, if (r.w > 0.0 && r.h > 0.0) r else Rect.from(path))
        }
        val width = n.strokeWidth ?: 2.0
        val color = colorOrBlack(n.stroke ?: STROKE)
        val shown = n.strokeGradient != null || n.stroke == null || (parseColor(n.stroke)?.let(::alphaOf) ?: 0) > 0
        if (width <= 0.0 || !shown) return
        if (closedShape(n)) {
            clipPath(path) { strokeWith(n, path, penOf(n, max(2 * width.toFloat(), 2 * hair)), color, null) }
            return
        }
        val pen = penOf(n, max(width.toFloat(), hair))
        val heads = arrowheads(scene, placed, n, r).map { h ->
            Path().apply {
                moveTo(h.tip.x.toFloat(), h.tip.y.toFloat())
                lineTo(h.left.x.toFloat(), h.left.y.toFloat())
                lineTo(h.right.x.toFloat(), h.right.y.toFloat())
                close()
            }
        }
        // One gradient over the line and its heads, so a head carries the colour on.
        val area = heads.fold(Rect.from(path).inflate(pen.width / 2.0)) { a, h -> a.union(Rect.from(h)) }
        strokeWith(n, path, pen, color, area)
        val g = n.strokeGradient
        for (head in heads) {
            val brush = g?.let { brushOf(it, area) }
            if (brush != null) drawPath(head, brush) else drawPath(head, color)
        }
    }

    /**
     * A picture in its rounded box (paint.rs `image`): its drop shadows, the picture (mirrored as
     * it is flipped) or a placeholder while the phone does not hold it, its inner shadows over the
     * picture (B31) and its border. Only the picture mirrors: its shadows stay put (B120).
     */
    fun DrawScope.image(n: DesignNode, r: Rect) {
        val box = outline(n, r)
        val shadows = effectiveShadows(n)
        outerShadows(box, shadows)
        clipPath(box.path()) {
            val bitmap = n.src?.let(picture)
            if (bitmap != null) {
                val target = fitRect(n.fit, r, bitmap.width.toDouble(), bitmap.height.toDouble())
                flipped(n, r) {
                    drawIntoCanvas {
                        val paint = android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG or android.graphics.Paint.ANTI_ALIAS_FLAG)
                        it.nativeCanvas.drawBitmap(bitmap.asAndroidBitmap(), null, RectF(target.x.toFloat(), target.y.toFloat(), target.right.toFloat(), target.bottom.toFloat()), paint)
                    }
                }
            } else {
                placeholder(r)
            }
        }
        innerShadows(box, shadows)
        border(n, r)
    }

    /** A picture not shown: a quiet tile with a picture glyph. */
    fun DrawScope.placeholder(r: Rect) {
        val grey = Color(0.55f, 0.55f, 0.55f, 0.25f)
        drawRect(grey, r.topLeft, r.size)
        val s = (min(r.w, r.h) * 0.25).coerceIn(6.0, 48.0).toFloat()
        val cx = r.cx.toFloat()
        val cy = r.cy.toFloat()
        val glyph = Path().apply {
            addRect(androidx.compose.ui.geometry.Rect(cx - s, cy - s * 0.75f, cx + s, cy + s * 0.75f))
            moveTo(cx - s, cy + s * 0.5f)
            lineTo(cx - s * 0.2f, cy - s * 0.2f)
            lineTo(cx + s * 0.3f, cy + s * 0.3f)
            lineTo(cx + s * 0.6f, cy)
            lineTo(cx + s, cy + s * 0.4f)
        }
        drawPath(glyph, Color(0.55f, 0.55f, 0.55f, 0.8f), style = Stroke(max(s / 8, 1f), join = StrokeJoin.Round))
    }

    /** [draw] mirrored about [r]'s centre as [n] is flipped (paint.rs `flip_about`). */
    inline fun DrawScope.flipped(n: DesignNode, r: Rect, draw: DrawScope.() -> Unit) {
        val fx = n.flipX == true
        val fy = n.flipY == true
        if (!fx && !fy) return draw()
        scale(if (fx) -1f else 1f, if (fy) -1f else 1f, Offset(r.cx.toFloat(), r.cy.toFloat())) { draw() }
    }

    /**
     * [draw] in a layer of its own over [bounds] (everything when unknown), put down at [alpha]
     * with [blend]: how a see-through node is one piece, and a blending one mixes with what is
     * under it.
     */
    inline fun DrawScope.layer(bounds: Rect?, alpha: Float = 1f, blend: BlendMode = BlendMode.SrcOver, draw: DrawScope.() -> Unit) {
        val area = bounds?.inflate(2.0 * hair + 2.0) ?: Rect(-PLACED_LIMIT, -PLACED_LIMIT, 2 * PLACED_LIMIT, 2 * PLACED_LIMIT)
        val paint = Paint().apply {
            this.alpha = alpha
            blendMode = blend
        }
        drawContext.canvas.saveLayer(area.g(), paint)
        try {
            draw()
        } finally {
            drawContext.canvas.restore()
        }
    }
}

/** The box a path's points span. */
private fun Rect.Companion.from(path: Path): Rect {
    val b = path.getBounds()
    return Rect(b.left.toDouble(), b.top.toDouble(), b.width.toDouble(), b.height.toDouble())
}

/**
 * The band [width] wide along [path]'s edge, as an outline of its own: GSK's plain stroke (mitred,
 * a miter limit of 4, butt ends), as the PC spreads a shape's shadow with.
 */
private fun band(path: android.graphics.Path, width: Double): android.graphics.Path = android.graphics.Path().also {
    android.graphics.Paint().apply {
        style = android.graphics.Paint.Style.STROKE
        strokeWidth = width.toFloat()
        strokeJoin = android.graphics.Paint.Join.MITER
        strokeMiter = 4f
    }.getFillPath(path, it)
}

/** [a] and [b] taken together by [op] as one outline; [a] alone should Skia fail to (a shadow then drawn without its spread). */
private fun joined(a: android.graphics.Path, b: android.graphics.Path, op: PathOp): android.graphics.Path =
    android.graphics.Path().takeIf { it.op(a, b, op) } ?: a
