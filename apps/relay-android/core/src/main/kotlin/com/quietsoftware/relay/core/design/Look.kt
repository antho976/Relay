package com.quietsoftware.relay.core.design

import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/*
 * How a node looks, by the PC's rules that need no drawing toolkit: the defaults the schema
 * documents, colours, the ground a text sits on, fonts, text case, gradients' ends, a picture's
 * fit, corners, insets, strokes, shadows, and a node's style and named colours resolved. Each is a
 * port of the function its comment names (apps/relay-native/src/design/paint.rs, the bus's
 * design.rs and design/style.rs), so the phone and the PC draw a design alike.
 */

// The defaults the schema documents (paint.rs): a white frame, a grey shape, a yellow sticky, a
// grey line, and the canvas a page shows when it names no fill.
const val FRAME_FILL = "#FFFFFF"
const val SHAPE_FILL = "#D9D9D9"
const val STICKY_FILL = "#FFE58A"
const val STROKE = "#8C8C8C"
const val CANVAS = "#1E1D1B"
const val DARK_TEXT = "#1A1A1A"
const val LIGHT_TEXT = "#EDE9E2"
const val DEFAULT_FONT = "Geist"
const val DEFAULT_FONT_SIZE = 16.0
const val DEFAULT_FONT_WEIGHT = 400
const val DEFAULT_LINE_HEIGHT = 1.3

/** The soft shadow a sticky casts when it names none (shapes.rs `effective_shadows`, B37). */
val STICKY_SHADOW = DesignShadow(y = 3.0, blur = 10.0, color = "#00000038")

/** `#RGB`, `#RRGGBB` or `#RRGGBBAA` as ARGB (`0xAARRGGBB`, Compose's `Color(Long)`); null for anything else, a `$name` included. */
fun parseColor(text: String?): Long? {
    val hex = text?.removePrefix("#")?.takeIf { it.length < text.length } ?: return null
    if (hex.isEmpty() || !hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
    val rgba = when (hex.length) {
        3 -> hex.map { "$it$it" }.joinToString("") + "ff"
        6 -> hex + "ff"
        8 -> hex
        else -> return null
    }.toLong(16)
    return (rgba and 0xFF shl 24) or (rgba ushr 8)
}

/** The alpha of an ARGB colour, 0 to 255. */
fun alphaOf(argb: Long): Int = ((argb ushr 24) and 0xFF).toInt()

/** The name in a `$name` colour, when [text] is one (style.rs `color_ref`). */
fun colorRef(text: String?): String? = text?.takeIf { it.startsWith('$') }?.substring(1)?.takeIf(::validId)

/** A node id or a colour's or style's name: 1 to 64 of letters, digits, `-`, `_`, `.` and `:` (design.rs `valid_id`). */
fun validId(id: String): Boolean =
    id.isNotEmpty() && id.length <= 64 && id.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it in "-_.:" }

/** Relative luminance of a colour, 0 (black) to 1 (white), ignoring alpha; black for what does not parse (paint.rs `luminance`). */
fun luminance(hex: String?): Double {
    val c = parseColor(hex) ?: 0xFF000000
    fun lin(v: Long): Double {
        val x = (v and 0xFF) / 255.0
        return if (x <= 0.04045) x / 12.92 else Math.pow((x + 0.055) / 1.055, 2.4)
    }
    return 0.2126 * lin(c ushr 16) + 0.7152 * lin(c ushr 8) + 0.0722 * lin(c)
}

/** The fill a node paints, its kind's default when it names none (paint.rs `fill_of`). */
fun fillOf(n: DesignNode): String? = n.fill ?: when (n.kind) {
    "frame" -> FRAME_FILL
    "rect", "ellipse", "polygon" -> SHAPE_FILL
    "sticky" -> STICKY_FILL
    "page" -> CANVAS
    else -> null
}

/** The colour a node shows behind what is on it: its fill, or the middle stop of its gradient (paint.rs `ground_of`). */
fun groundOf(n: DesignNode): String? {
    val stops = n.gradient?.stops.orEmpty()
    if (stops.isEmpty()) return fillOf(n)
    return stops.sortedBy { it.at }[stops.size / 2].color
}

/**
 * A text's colour: its own, or dark on a light ground and light on a dark one (paint.rs
 * `text_color`). A label sits on its own shape (its gradient's first stop, else its fill); a
 * text, or an icon without a stroke, on the nearest container that paints a ground at least half
 * opaque, else the canvas.
 */
fun textColor(scene: DesignScene, n: DesignNode): String {
    n.color?.let { return it }
    val ground = if (n.kind in LABELLED) {
        n.gradient?.stops?.firstOrNull()?.color ?: fillOf(n).orEmpty()
    } else {
        scene.ancestors(n.id).firstNotNullOfOrNull { id ->
            scene.node(id)?.let(::groundOf)?.takeIf { (parseColor(it)?.let(::alphaOf) ?: 0) >= 128 }
        } ?: CANVAS
    }
    return if (luminance(ground) > 0.35) DARK_TEXT else LIGHT_TEXT
}

private val LABELLED = setOf("sticky", "rect", "ellipse", "polygon")

fun fontSize(n: DesignNode): Double = n.fontSize ?: DEFAULT_FONT_SIZE

fun fontFamily(n: DesignNode): String = n.font ?: DEFAULT_FONT

fun fontWeight(n: DesignNode): Int = n.fontWeight ?: DEFAULT_FONT_WEIGHT

fun lineHeight(n: DesignNode): Double = n.lineHeight ?: DEFAULT_LINE_HEIGHT

/** How a text's lines sit across its box: its own `align`, else `left`, or `center` for a shape's label. */
fun textAlign(n: DesignNode): String = n.align?.takeIf { it in DesignValues.align } ?: if (n.kind in setOf("rect", "ellipse", "polygon")) "center" else "left"

/**
 * [text] as [case] draws it (design.rs `TextCase::apply`): upper and lower case one character at a
 * time by Unicode's default mapping (`ß` is `SS`; no language's own rules), title case capitalising
 * where [titleStarts] says a word starts (B74). An unknown case draws the words as stored.
 */
fun textCase(case: String?, text: String): String = cased(case, text, null)

/**
 * [textCase], and for each character of [text] (Unicode scalar values, as runs count them) and
 * one past the last, the UTF-16 index where it starts in the cased text, so a run's stretch of
 * the stored words is found again in what is drawn (a letter can become two).
 */
fun textCaseIndexed(case: String?, text: String): Pair<String, IntArray> {
    val starts = IntArray(text.codePointCount(0, text.length) + 1)
    return cased(case, text, starts) to starts
}

/** The character indices where a title-cased word starts in [text] (design.rs `title_starts`). */
fun titleStarts(text: String): List<Int> {
    val words = WordStarts()
    val out = ArrayList<Int>()
    var i = 0
    var k = 0
    while (i < text.length) {
        val cp = text.codePointAt(i)
        if (words.starts(cp)) out += k
        i += Character.charCount(cp)
        k++
    }
    return out
}

private fun cased(case: String?, text: String, starts: IntArray?): String {
    val known = case in DesignValues.cases
    val out = StringBuilder(text.length)
    val words = WordStarts()
    var i = 0
    var k = 0
    while (i < text.length) {
        val cp = text.codePointAt(i)
        starts?.set(k, out.length)
        when {
            !known -> out.appendCodePoint(cp)
            case == "upper" -> out.append(String(Character.toChars(cp)).uppercase())
            case == "lower" -> out.append(String(Character.toChars(cp)).lowercase())
            words.starts(cp) -> titleCase(cp, out)
            else -> out.appendCodePoint(cp)
        }
        i += Character.charCount(cp)
        k++
    }
    starts?.set(k, out.length)
    return out.toString()
}

/**
 * Where a word starts, a character at a time (design.rs `WordStarts`): at a letter or digit after
 * the start of the text, a space or any punctuation, but not after an apostrophe between two
 * letters (`don't`).
 */
private class WordStarts {
    private var inWord = false
    private var apostrophe = false

    fun starts(cp: Int): Boolean {
        val alnum = isAlphanumeric(cp)
        val start = alnum && !inWord && !apostrophe
        when {
            alnum -> { inWord = true; apostrophe = false }
            cp == '\''.code || cp == 0x2019 -> { apostrophe = inWord; inWord = false }
            else -> { inWord = false; apostrophe = false }
        }
        return start
    }
}

/** Rust's `char::is_alphanumeric`: Unicode's Alphabetic property, or a number (Nd, Nl, No). */
private fun isAlphanumeric(cp: Int): Boolean = Character.isAlphabetic(cp) || when (Character.getType(cp).toByte()) {
    Character.DECIMAL_DIGIT_NUMBER, Character.LETTER_NUMBER, Character.OTHER_NUMBER -> true
    else -> false
}

/** [cp] as the first letter of a word: the titlecase digraphs (`ǅ`, `ǈ`, `ǋ`, `ǲ`) for their letters, as GLib's `g_unichar_totitle`, else its capital. */
private fun titleCase(cp: Int, out: StringBuilder) {
    when (cp) {
        in 0x1C4..0x1C6 -> out.appendCodePoint(0x1C5)
        in 0x1C7..0x1C9 -> out.appendCodePoint(0x1C8)
        in 0x1CA..0x1CC -> out.appendCodePoint(0x1CB)
        in 0x1F1..0x1F3 -> out.appendCodePoint(0x1F2)
        else -> out.append(String(Character.toChars(cp)).uppercase())
    }
}

/**
 * Where a linear gradient at [angle] runs across [r] (paint.rs `gradient`): CSS's reading, 0 up,
 * 90 right, 180 (the default) down, long enough that the stops reach the box's corners.
 */
fun linearEnds(angle: Double?, r: Rect): Pair<Pt, Pt> {
    val a = Math.toRadians(angle ?: 180.0)
    val dx = sin(a)
    val dy = -cos(a)
    val len = abs(r.w * sin(a)) + abs(r.h * cos(a))
    return Pt(r.cx - dx * len / 2, r.cy - dy * len / 2) to Pt(r.cx + dx * len / 2, r.cy + dy * len / 2)
}

/** Where a picture [naturalW] by [naturalH] is drawn in [r] (paint.rs `image`): `cover` (the default, and for an unknown fit), `contain` or `stretch`. */
fun fitRect(fit: String?, r: Rect, naturalW: Double, naturalH: Double): Rect {
    if (fit == "stretch") return r
    val sx = r.w / max(naturalW, 1.0)
    val sy = r.h / max(naturalH, 1.0)
    val s = if (fit == "contain") min(sx, sy) else max(sx, sy)
    val w = naturalW * s
    val h = naturalH * s
    return Rect(r.cx - w / 2, r.cy - h / 2, w, h)
}

/**
 * A box's corner radii, top-left, top-right, bottom-right, bottom-left (paint.rs `outline`):
 * `radii`, else `radius`, else 6 for a sticky, each at most half the shorter side. An ellipse is
 * an oval instead.
 */
fun cornerRadii(n: DesignNode, r: Rect): DoubleArray {
    fun cap(v: Double) = max(min(min(v, r.w / 2), r.h / 2), 0.0)
    val radii = n.radii?.takeIf { it.size == 4 }
    return if (radii != null) {
        DoubleArray(4) { cap(radii[it]) }
    } else {
        val radius = cap(n.radius ?: if (n.kind == "sticky") 6.0 else 0.0)
        DoubleArray(4) { radius }
    }
}

/** `padding` as top, right, bottom and left (design.rs `padding_edges`); zeros when absent or of another length. */
fun paddingEdges(n: DesignNode): DoubleArray = when (val p = n.padding) {
    null -> DoubleArray(4)
    else -> when (p.size) {
        1 -> DoubleArray(4) { p[0] }
        2 -> doubleArrayOf(p[0], p[1], p[0], p[1])
        4 -> p.toDoubleArray()
        else -> DoubleArray(4)
    }
}

/** Where a rect's, ellipse's or polygon's label sits inside its box, top, right, bottom, left: `padding`, else 12 px at the sides (design.rs `label_inset`). */
fun labelInset(n: DesignNode): DoubleArray = if (n.padding != null) paddingEdges(n) else doubleArrayOf(0.0, 12.0, 0.0, 12.0)

/**
 * A shape's label box (feature-drawing §6.1 `shape_text_box`): inside [r] by [labelInset], as
 * wide as that leaves (at least 1), as tall as [height] measures its words at that width, centred
 * down the inset box.
 */
fun labelBox(n: DesignNode, r: Rect, height: (wrap: Double) -> Double): Rect {
    val (t, rt, b, l) = labelInset(n)
    val inner = Rect(r.x + l, r.y + t, max(r.w - l - rt, 1.0), max(r.h - t - b, 0.0))
    val h = height(inner.w)
    return Rect(inner.x, inner.cy - h / 2, inner.w, h)
}

/**
 * A node's stroke width in px (the field's doc): 2 for a line or path (never under 0.5, as
 * paint.rs `stroke` draws it) or an icon (in its 24-unit grid), 1 for a border or outline.
 */
fun strokeWidth(n: DesignNode): Double = when (n.kind) {
    "line", "path" -> max(n.strokeWidth ?: 2.0, 0.5)
    "icon" -> n.strokeWidth ?: 2.0
    else -> n.strokeWidth ?: 1.0
}

/** How a stroke's ends are drawn: its own `stroke_cap`, else round for a solid stroke or dots, butt for dashes (the field's doc). */
fun strokeCap(n: DesignNode): String = n.strokeCap?.takeIf { it in DesignValues.caps } ?: when {
    n.dash.isNullOrEmpty() || dashDots(n.dash) -> "round"
    else -> "butt"
}

/** How a stroke turns its corners: its own `stroke_join`, else round. */
fun strokeJoin(n: DesignNode): String = n.strokeJoin?.takeIf { it in DesignValues.joins } ?: "round"

/** A dash pattern as the drawing toolkits take it: an odd list repeated, so it alternates dash and gap; null for a solid stroke. */
fun dashIntervals(dash: List<Double>?): DoubleArray? {
    if (dash.isNullOrEmpty()) return null
    return (if (dash.size % 2 == 1) dash + dash else dash).toDoubleArray()
}

/** Whether a dash has 0-length dashes (round dots): its even entries are the dashes, and an odd list repeats, so then every entry is one in turn (style.rs `dotted`). */
fun dashDots(dash: List<Double>?): Boolean {
    val d = dash.orEmpty()
    return d.withIndex().any { (i, v) -> v == 0.0 && (i % 2 == 0 || d.size % 2 == 1) }
}

/** Whether a [kind] node casts outer shadows only, without spread: a text, line, path or icon (fields.rs `casts_outer_only`). */
fun castsOuterOnly(kind: String): Boolean = kind == "text" || kind == "line" || kind == "path" || kind == "icon"

/**
 * The shadows a node casts, in paint order (shapes.rs `effective_shadows`, MODEL-SPEC §1 #13):
 * `shadows`, else `shadow`, without the ones whose colour is fully transparent (a sticky with
 * one such shadow casts nothing, B37); a sticky naming none casts [STICKY_SHADOW]; a text, line,
 * path or icon casts outer ones only, without spread, whatever is stored or its style lays under.
 */
fun effectiveShadows(n: DesignNode): List<DesignShadow> {
    val named = n.shadows ?: listOfNotNull(n.shadow)
    if (named.isEmpty()) return if (n.kind == "sticky") listOf(STICKY_SHADOW) else emptyList()
    val shown = named.filter { s -> parseColor(s.color)?.let(::alphaOf) != 0 }
    if (!castsOuterOnly(n.kind)) return shown
    return shown.filter { !it.inset }.map { if (it.spread != 0.0) it.copy(spread = 0.0) else it }
}

/** How far past its box a node's outer shadows reach: the largest `blur × 2 + spread + max(|x|, |y|)` (feature-drawing §5.3 `shadow_reach`). */
fun shadowReach(n: DesignNode): Double =
    effectiveShadows(n).filter { !it.inset }.maxOfOrNull { it.blur * 2 + max(it.spread, 0.0) + max(abs(it.x), abs(it.y)) } ?: 0.0

/** How far past its box a node may paint: its shadows, its stroke and its blur (an arc's caps are the shape's to add). */
fun reach(n: DesignNode): Double = shadowReach(n) + strokeWidth(n) + 2 * (n.blur ?: 0.0)

/** A polygon's corners, or a star's points: `sides`, else 5 for a star and 3 for a polygon, within 3 to 24 (B57; the field's doc, shapes.rs `node_polygon`). */
fun polygonSides(n: DesignNode): Int = (n.sides ?: if (n.star != null) 5 else 3).coerceIn(3, 24)

/** The style fields, in order (style.rs `STYLE_FIELDS`). */
val STYLE_FIELDS: List<String> = listOf(
    "fill", "gradient", "stroke", "stroke_width", "dash", "radius", "radii", "shadow", "shadows", "opacity", "blur", "blend",
    "color", "font", "font_size", "font_weight", "italic", "underline", "strike", "align", "line_height", "letter_spacing",
    "text_case", "stroke_cap", "stroke_join", "stroke_gradient",
)

private val ALL_BUT_PAGE = DesignValues.kinds - "page"
private val TEXTUAL = setOf("text", "sticky", "rect", "ellipse", "polygon")
private val STROKED = setOf("frame", "rect", "ellipse", "polygon", "sticky", "image", "line", "path", "icon")
private val CAPPED = setOf("frame", "rect", "ellipse", "polygon", "sticky", "image", "line", "path")

/**
 * The kinds that use each style field (fields.rs `FIELDS`): a style lays a field only on a node
 * whose kind uses it. `style-kinds.json` in the tests holds the same table, which
 * crates/relay-bus/tests/phone_fixture.rs checks against the PC's.
 */
val STYLE_KINDS: Map<String, Set<String>> = mapOf(
    "fill" to setOf("page", "frame", "rect", "ellipse", "polygon", "sticky", "text", "line", "path"),
    "gradient" to setOf("frame", "rect", "ellipse", "polygon", "sticky", "line", "path"),
    "stroke" to STROKED,
    "stroke_width" to STROKED,
    "dash" to STROKED,
    "radius" to setOf("frame", "rect", "sticky", "image"),
    "radii" to setOf("frame", "rect", "sticky", "image"),
    "shadow" to STROKED + "text",
    "shadows" to STROKED + "text",
    "opacity" to ALL_BUT_PAGE,
    "blur" to ALL_BUT_PAGE,
    "blend" to ALL_BUT_PAGE,
    "color" to TEXTUAL + "icon",
    "font" to TEXTUAL,
    "font_size" to TEXTUAL,
    "font_weight" to TEXTUAL,
    "italic" to TEXTUAL,
    "underline" to TEXTUAL,
    "strike" to TEXTUAL,
    "align" to TEXTUAL,
    "line_height" to TEXTUAL,
    "letter_spacing" to TEXTUAL,
    "text_case" to TEXTUAL,
    "stroke_cap" to CAPPED,
    "stroke_join" to setOf("ellipse", "polygon", "line", "path"),
    "stroke_gradient" to CAPPED,
)

/** [resolveLook] with the design's named colours and styles. */
fun resolveLook(n: DesignNode, colors: Map<String, String>, styles: Map<String, DesignStyle>): DesignNode = DesignLooks(colors, styles).resolve(n)

/**
 * A design's named colours and styles, and a node as it draws with them (style.rs `Styles`).
 * The phone only reads: it resolves, as the PC's scene does, and never checks or changes them.
 */
class DesignLooks(val colors: Map<String, String> = emptyMap(), val styles: Map<String, DesignStyle> = emptyMap()) {
    val isEmpty: Boolean get() = colors.isEmpty() && styles.isEmpty()

    /**
     * [n] as it draws (style.rs `Styles::resolve`): first what it names that the design does not
     * hold taken out (a colour field cleared, so the kind's default shows; one shadow, gradient
     * stop or run colour dropped), then its style laid under it, then each `$name` as the colour it
     * names.
     */
    fun resolve(n: DesignNode): DesignNode {
        var drawn = fixColors(n) { name -> if (name in colors) Fix.Keep else Fix.Drop }
        if (drawn.style != null && drawn.style !in styles) drawn = drawn.copy(style = null)
        drawn.style?.let { styles[it] }?.let { drawn = layUnder(it, drawn) }
        return fixColors(drawn) { name -> colors[name]?.let { Fix.Write(it) } ?: Fix.Drop }
    }

    companion object {
        val NONE = DesignLooks()

        /** Whether [n] names a colour or a style, and so draws otherwise than it is stored (style.rs `Styles::uses`). */
        fun uses(n: DesignNode): Boolean =
            n.style != null || colorRef(n.fill) != null || colorRef(n.stroke) != null || colorRef(n.color) != null ||
                (listOfNotNull(n.shadow) + n.shadows.orEmpty()).any { colorRef(it.color) != null } ||
                listOfNotNull(n.gradient, n.strokeGradient).any { g -> g.stops.any { colorRef(it.color) != null } } ||
                n.runs.orEmpty().any { colorRef(it.color) != null }
    }
}

/** Whether a node or a style takes [field] (fields.rs `takes`, for the style fields). */
fun takesStyle(kind: String, field: String): Boolean = STYLE_KINDS[field]?.contains(kind) == true

private sealed interface Fix {
    data object Keep : Fix
    data object Drop : Fix
    data class Write(val value: String) : Fix
}

/** Each `$name` colour of [n] fixed as [fix] says (style.rs `fix_colors`). */
private fun fixColors(n: DesignNode, fix: (String) -> Fix): DesignNode {
    fun plain(c: String?): String? = when (val f = colorRef(c)?.let(fix)) {
        null, Fix.Keep -> c
        Fix.Drop -> null
        is Fix.Write -> f.value
    }
    fun shadow(s: DesignShadow): DesignShadow? = when (val f = colorRef(s.color)?.let(fix)) {
        null, Fix.Keep -> s
        Fix.Drop -> null
        is Fix.Write -> s.copy(color = f.value)
    }
    fun gradient(g: DesignGradient?): DesignGradient? {
        g ?: return null
        val stops = g.stops.mapNotNull { stop ->
            when (val f = colorRef(stop.color)?.let(fix)) {
                null, Fix.Keep -> stop
                Fix.Drop -> null
                is Fix.Write -> stop.copy(color = f.value)
            }
        }
        return if (stops.size == g.stops.size) g.copy(stops = stops) else if (stops.size < 2) null else g.copy(stops = stops)
    }
    val runs = n.runs?.mapNotNull { run ->
        when (val f = colorRef(run.color)?.let(fix)) {
            null, Fix.Keep -> run
            Fix.Drop -> run.copy(color = null).takeIf { it.styled }
            is Fix.Write -> run.copy(color = f.value)
        }
    }
    val shadows = n.shadows?.mapNotNull(::shadow)
    val out = n.copy(
        fill = plain(n.fill),
        stroke = plain(n.stroke),
        color = plain(n.color),
        shadow = n.shadow?.let(::shadow),
        shadows = shadows?.ifEmpty { null },
        gradient = gradient(n.gradient),
        strokeGradient = gradient(n.strokeGradient),
        runs = runs?.ifEmpty { null },
    )
    return if (out == n) n else out
}

/** Whether a run sets anything of its own (design.rs `TextRun::styled`). */
private val DesignTextRun.styled: Boolean
    get() = color != null || font != null || fontSize != null || fontWeight != null || italic != null || underline != null || strike != null || rise != null

/** Each style field and its partner: a node's own `fill` keeps its style's `gradient` off, and the other way round (style.rs `partner`). */
private val PARTNER = mapOf(
    "fill" to "gradient", "gradient" to "fill", "radius" to "radii", "radii" to "radius",
    "shadow" to "shadows", "shadows" to "shadow", "stroke" to "stroke_gradient", "stroke_gradient" to "stroke",
)

/** Whether [n] sets style field [field] itself. */
private fun sets(n: DesignNode, field: String): Boolean = when (field) {
    "fill" -> n.fill != null
    "gradient" -> n.gradient != null
    "stroke" -> n.stroke != null
    "stroke_width" -> n.strokeWidth != null
    "dash" -> n.dash != null
    "radius" -> n.radius != null
    "radii" -> n.radii != null
    "shadow" -> n.shadow != null
    "shadows" -> n.shadows != null
    "opacity" -> n.opacity != null
    "blur" -> n.blur != null
    "blend" -> n.blend != null
    "color" -> n.color != null
    "font" -> n.font != null
    "font_size" -> n.fontSize != null
    "font_weight" -> n.fontWeight != null
    "italic" -> n.italic != null
    "underline" -> n.underline != null
    "strike" -> n.strike != null
    "align" -> n.align != null
    "line_height" -> n.lineHeight != null
    "letter_spacing" -> n.letterSpacing != null
    "text_case" -> n.textCase != null
    "stroke_cap" -> n.strokeCap != null
    "stroke_join" -> n.strokeJoin != null
    "stroke_gradient" -> n.strokeGradient != null
    else -> false
}

/** Whether [s] holds style field [field]. */
private fun holds(s: DesignStyle, field: String): Boolean = when (field) {
    "fill" -> s.fill != null
    "gradient" -> s.gradient != null
    "stroke" -> s.stroke != null
    "stroke_width" -> s.strokeWidth != null
    "dash" -> s.dash != null
    "radius" -> s.radius != null
    "radii" -> s.radii != null
    "shadow" -> s.shadow != null
    "shadows" -> s.shadows != null
    "opacity" -> s.opacity != null
    "blur" -> s.blur != null
    "blend" -> s.blend != null
    "color" -> s.color != null
    "font" -> s.font != null
    "font_size" -> s.fontSize != null
    "font_weight" -> s.fontWeight != null
    "italic" -> s.italic != null
    "underline" -> s.underline != null
    "strike" -> s.strike != null
    "align" -> s.align != null
    "line_height" -> s.lineHeight != null
    "letter_spacing" -> s.letterSpacing != null
    "text_case" -> s.textCase != null
    "stroke_cap" -> s.strokeCap != null
    "stroke_join" -> s.strokeJoin != null
    "stroke_gradient" -> s.strokeGradient != null
    else -> false
}

/**
 * [style] laid under [n] (style.rs `Style::lay_under`): each field it holds that the node's kind
 * uses and the node does not set itself, a field and its [PARTNER] counting as one. Under a text,
 * line, path or icon its inner shadows are left out and its spread is 0, as those kinds cast
 * shadows. A node's own butt ends and the style's dotted dash (or the other way round) are not
 * combined, since butt ends draw nothing of a dot: the style's half is left out.
 */
private fun layUnder(style: DesignStyle, n: DesignNode): DesignNode {
    val s = forKind(style, n.kind)
    val lay = STYLE_FIELDS.filterTo(HashSet()) { field ->
        holds(s, field) && takesStyle(n.kind, field) && !sets(n, field) && PARTNER[field]?.let { !sets(n, it) } != false
    }
    val dash = if ("dash" in lay) s.dash else n.dash
    val cap = if ("stroke_cap" in lay) s.strokeCap else n.strokeCap
    if (cap == "butt" && dashDots(dash)) {
        lay -= "dash"
        lay -= "stroke_cap"
    }
    if (lay.isEmpty()) return n
    fun <T> pick(field: String, own: T, styled: T): T = if (field in lay) styled else own
    return n.copy(
        fill = pick("fill", n.fill, s.fill),
        gradient = pick("gradient", n.gradient, s.gradient),
        stroke = pick("stroke", n.stroke, s.stroke),
        strokeWidth = pick("stroke_width", n.strokeWidth, s.strokeWidth),
        dash = pick("dash", n.dash, s.dash),
        radius = pick("radius", n.radius, s.radius),
        radii = pick("radii", n.radii, s.radii),
        shadow = pick("shadow", n.shadow, s.shadow),
        shadows = pick("shadows", n.shadows, s.shadows),
        opacity = pick("opacity", n.opacity, s.opacity),
        blur = pick("blur", n.blur, s.blur),
        blend = pick("blend", n.blend, s.blend),
        color = pick("color", n.color, s.color),
        font = pick("font", n.font, s.font),
        fontSize = pick("font_size", n.fontSize, s.fontSize),
        fontWeight = pick("font_weight", n.fontWeight, s.fontWeight),
        italic = pick("italic", n.italic, s.italic),
        underline = pick("underline", n.underline, s.underline),
        strike = pick("strike", n.strike, s.strike),
        align = pick("align", n.align, s.align),
        lineHeight = pick("line_height", n.lineHeight, s.lineHeight),
        letterSpacing = pick("letter_spacing", n.letterSpacing, s.letterSpacing),
        textCase = pick("text_case", n.textCase, s.textCase),
        strokeCap = pick("stroke_cap", n.strokeCap, s.strokeCap),
        strokeJoin = pick("stroke_join", n.strokeJoin, s.strokeJoin),
        strokeGradient = pick("stroke_gradient", n.strokeGradient, s.strokeGradient),
    )
}

/** [style] as it lies under a [kind] node (style.rs `Style::for_kind`): on a text, line, path or icon, outer shadows without spread, blur within 200. */
private fun forKind(style: DesignStyle, kind: String): DesignStyle {
    val all = listOfNotNull(style.shadow) + style.shadows.orEmpty()
    if (!castsOuterOnly(kind) || all.all { !it.inset && it.spread == 0.0 }) return style
    fun fit(s: DesignShadow): DesignShadow? = if (s.inset) null else s.copy(blur = min(s.blur, 200.0), spread = 0.0)
    return style.copy(shadow = style.shadow?.let(::fit), shadows = style.shadows?.mapNotNull(::fit)?.ifEmpty { null })
}
