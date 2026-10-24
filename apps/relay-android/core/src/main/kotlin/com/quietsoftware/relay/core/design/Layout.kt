package com.quietsoftware.relay.core.design

import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/*
 * Where every node of a page lands: auto layout (row, column and wrap), hugging text, stickies
 * that grow to their words, groups as big as their children, pins, absolute children, min and
 * max sizes, and scrolling frames. A port, rule for rule and in the same order, of
 * crates/relay-client/src/design/layout.rs, so a page lays out on the phone where it lays out on
 * the PC. A change there changes this file in the same piece of work, with a case in
 * core/src/test/resources/design/layout-cases.json, which DesignLayoutTest runs here and
 * crates/relay-client/tests/phone_fixtures.rs runs against the PC's code.
 *
 * Where the phone differs, each on purpose:
 * - It places a still picture (MODEL-SPEC §5 #2): every scrolling frame at its start, so there
 *   are no scroll offsets to apply, and [Placed.scrolls] holds only what each one's content spans.
 * - What it cannot read is not placed, and `Omissions` counts it: a node of a kind it does not
 *   know, one more than [DesignScene.MAX_DEPTH] below its page, and what a frame with a layout
 *   it does not know holds. Such a frame is placed at its own size, as an unlaid frame is.
 * - A sizing, pin, `items` or `justify` value it does not know reads as left out.
 * - The editing helpers (drop indexes, fits, unpinning) are not here: the phone only shows.
 */

/** How far from the page's origin a box is placed, either way (layout.rs `PLACED_LIMIT`). */
const val PLACED_LIMIT = 4_194_304.0

/** Inner margin of a sticky's text. */
const val STICKY_PAD = 16.0

/** Size a sticky starts at. */
const val STICKY_SIZE = 200.0

/** The largest coordinate and size a node holds (design.rs `MAX_COORD`). */
const val MAX_COORD = 1_000_000.0

/** Slack when a wrap line is checked for room, so a line that fits exactly is not broken by rounding. */
private const val SLACK = 1e-6

/** A text's measured size in page px. A class, not a pair, so a baseline can join it (W57). */
data class TextSize(val w: Double, val h: Double)

/** Measures a text's or sticky's words: their size when wrapped at [wrap] px, or unwrapped for null (layout.rs `Measure`). */
fun interface Measure {
    fun measure(n: DesignNode, wrap: Double?): TextSize
}

/** How a node's width or height is decided (design.rs `Sizing`). */
enum class Sizing { Fixed, Hug, Fill }

/** How a frame places its children (design.rs `Layout`); [Unknown] is a layout this phone does not know. */
enum class LayoutMode { None, Row, Column, Wrap, Unknown }

/** One axis of a pin, either way round: [Start] is left or top (design/pins.rs `Pin`). */
enum class Pin {
    Start, End, Both, Center, Scale;

    /** Whether the box's length changes with its parent's. */
    val stretches: Boolean get() = this == Both || this == Scale
}

private fun sizingOf(text: String?): Sizing? = when (text) {
    "fixed" -> Sizing.Fixed
    "hug" -> Sizing.Hug
    "fill" -> Sizing.Fill
    else -> null
}

/** How [n]'s width is decided: `w_sizing`, else fixed, or hug for a text without a width (design.rs `Node::w_rule`). */
fun wSizing(n: DesignNode): Sizing = sizingOf(n.wSizing) ?: if (n.kind == "text" && n.w <= 0.0) Sizing.Hug else Sizing.Fixed

/** How [n]'s height is decided: `h_sizing`, else fixed, or hug for a text (design.rs `Node::h_rule`). */
fun hSizing(n: DesignNode): Sizing = sizingOf(n.hSizing) ?: if (n.kind == "text") Sizing.Hug else Sizing.Fixed

/** How [n] lays out its children: only a frame does (design.rs `Node::layout_mode`). */
fun layoutMode(n: DesignNode): LayoutMode = if (n.kind != "frame") LayoutMode.None else when (n.layout) {
    null, "none" -> LayoutMode.None
    "row" -> LayoutMode.Row
    "column" -> LayoutMode.Column
    "wrap" -> LayoutMode.Wrap
    else -> LayoutMode.Unknown
}

/** Out of its frame's auto layout (`absolute: true`). */
fun isAbsolute(n: DesignNode): Boolean = n.absolute == true

/** Its pins across and down, [Pin.Start] when unset (design/pins.rs `Node::pins`). */
fun pinsOf(n: DesignNode): Pair<Pin, Pin> {
    val x = when (n.pinX) {
        "right" -> Pin.End
        "left_right" -> Pin.Both
        "center" -> Pin.Center
        "scale" -> Pin.Scale
        else -> Pin.Start
    }
    val y = when (n.pinY) {
        "bottom" -> Pin.End
        "top_bottom" -> Pin.Both
        "center" -> Pin.Center
        "scale" -> Pin.Scale
        else -> Pin.Start
    }
    return x to y
}

/** Pinned on either axis to anything but the left or top. */
fun isPinned(n: DesignNode): Boolean = pinsOf(n) != (Pin.Start to Pin.Start)

/** [w] within `min_w` and `max_w`, a minimum winning over a maximum as in CSS (design/pins.rs `Node::clamp_w`). */
fun clampW(n: DesignNode, w: Double): Double = clampTo(w, n.minW, n.maxW)

/** [h] within `min_h` and `max_h`. */
fun clampH(n: DesignNode, h: Double): Double = clampTo(h, n.minH, n.maxH)

private fun clampTo(v: Double, lo: Double?, hi: Double?): Double {
    val capped = if (hi != null) min(v, hi) else v
    return if (lo != null) max(capped, lo) else capped
}

/**
 * Where a box pinned by [pin] sits and how long it is along one axis when its parent is [now]
 * long, given its offset [pos] and length [len] while the parent is [was] long (design/pins.rs
 * `follow_axis`).
 */
fun followAxis(pin: Pin, pos: Double, len: Double, was: Double, now: Double): Pair<Double, Double> {
    val d = now - was
    return when (pin) {
        Pin.Start -> Pair(pos, len)
        Pin.End -> Pair(pos + d, len)
        Pin.Center -> Pair(pos + d / 2.0, len)
        Pin.Both -> Pair(pos, max(len + d, 0.0))
        Pin.Scale -> if (was > 0.0) Pair(pos * now / was, len * now / was) else Pair(pos, len)
    }
}

/** Every placed node's box in page coordinates, as one [place] left them. */
class Placed internal constructor() {
    internal val rectMap = HashMap<String, Rect>()
    internal val originMap = HashMap<String, Pt>()
    internal val scrollMap = HashMap<String, Rect>()
    internal val wrapMap = HashMap<String, Double>()

    val rects: Map<String, Rect> get() = rectMap

    /**
     * Where a group's children count their `x`/`y` from: its parent's origin plus its own `x`/`y`
     * (or where a layout put it). Its box in [rects] is its children's bounds instead.
     */
    val origins: Map<String, Pt> get() = originMap

    /** Every scrolling frame placed, by the box its content spans from its corner (at least the frame), at the start of its scroll. */
    val scrolls: Map<String, Rect> get() = scrollMap

    /** The width a hugging text was wrapped at because a column stretched it (TM-7): read through [wrapWidth]. */
    val wraps: Map<String, Double> get() = wrapMap

    fun rect(id: String): Rect? = rectMap[id]

    /** The page point [id]'s children count their `x`/`y` from: a group's own point, any other node's box corner, (0, 0) for what is not placed. */
    fun origin(id: String): Pt = originMap[id] ?: rectMap[id]?.let { Pt(it.x, it.y) } ?: Pt(0.0, 0.0)

    /** Where a text's or sticky's words wrap in its placed box [rect]: the width layout measured them at. */
    fun wrapWidth(n: DesignNode, rect: Rect): Double? = wrapMap[n.id] ?: freeWrap(n, rect)
}

/**
 * Where a text's words wrap, given its placed box: null while it hugs them, unless a min or max
 * width bounds it; a sticky's inside its margins (layout.rs `wrap_width`). A hugging text a
 * column stretches wraps too, which only [Placed.wrapWidth] knows.
 */
fun wrapWidth(n: DesignNode, rect: Rect): Double? = freeWrap(n, rect)

private fun freeWrap(n: DesignNode, rect: Rect): Double? = when (n.kind) {
    "sticky" -> max(rect.w - 2.0 * STICKY_PAD, 1.0)
    "text" -> if (wSizing(n) != Sizing.Hug || n.minW != null || n.maxW != null) max(rect.w, 1.0) else null
    else -> null
}

/**
 * The lines a wrap layout breaks children [widths] wide into, [gap] apart: a new line when the
 * next would pass [room]; a child wider than [room] sits alone on its line; with no [room], one
 * line (layout.rs `break_lines`).
 */
fun breakLines(widths: List<Double>, room: Double?, gap: Double): List<IntRange> {
    val lines = ArrayList<IntRange>()
    var start = 0
    var used = 0.0
    for ((i, w) in widths.withIndex()) {
        if (i > start && room != null && used + gap + w > room + SLACK) {
            lines += start until i
            start = i
            used = w
        } else {
            used += if (i > start) gap + w else w
        }
    }
    if (start < widths.size) lines += start until widths.size
    return lines
}

/** Place every node of [page], every scrolling frame at its start: the still picture the phone shows (layout.rs `place`). */
fun place(scene: DesignScene, page: String, measure: Measure): Placed {
    val pass = LayoutPass(scene, measure)
    pass.placed.rectMap[page] = Rect.ZERO
    for (child in scene.children(page)) pass.freeChild(child, Pt(0.0, 0.0), null)
    return pass.placed
}

/**
 * Equal shares of [room] (none below 0) for filling children bounded by [bounds] (min, max): a
 * share a bound changes is kept at the bound and the others share what is left, the way CSS
 * resolves flexible lengths (layout.rs `shares`). Each round keeps at least one share, so it ends
 * within one round a child; the count of rounds holds it to that whatever the numbers.
 */
internal fun shares(room: Double, bounds: List<Pair<Double?, Double?>>): List<Double> {
    fun clamp(v: Double, b: Pair<Double?, Double?>): Double = clampTo(v, b.first, b.second)
    val kept = arrayOfNulls<Double>(bounds.size)
    var rounds = bounds.size + 1
    while (rounds-- > 0) {
        val open = bounds.indices.filter { kept[it] == null }
        if (open.isEmpty()) break
        var held = 0.0
        for (k in kept) if (k != null) held += k
        // At least 0, a share that is no number included, as Rust's f64::max gives the other.
        val each = ((room - held) / open.size).let { if (it.isNaN()) 0.0 else max(it, 0.0) }
        var pull = 0.0
        for (i in open) pull += clamp(each, bounds[i]) - each
        for (i in open) {
            val bound = clamp(each, bounds[i])
            if (pull == 0.0 || (pull > 0.0 && bound > each) || (pull < 0.0 && bound < each)) {
                kept[i] = if (pull == 0.0) each else bound
            }
        }
    }
    return kept.map { it ?: 0.0 }
}

/** [v] within [PLACED_LIMIT] of the page's origin. */
private fun limit(v: Double): Double = v.coerceIn(-PLACED_LIMIT, PLACED_LIMIT)

/** [r] placed within [PLACED_LIMIT]. */
private fun bounded(r: Rect): Rect =
    Rect(limit(r.x), limit(r.y), r.w.coerceIn(0.0, 2.0 * PLACED_LIMIT), r.h.coerceIn(0.0, 2.0 * PLACED_LIMIT))

private fun itemsOf(n: DesignNode): String = n.items?.takeIf { it in DesignValues.items } ?: "start"

private fun justifyOf(n: DesignNode): String = n.justify?.takeIf { it in DesignValues.justify } ?: "start"

/** A width and a height. */
private data class Dims(val w: Double, val h: Double)

/** How a frame is drawn against how it is stored, which its pinned children follow. */
private class Fit(val storedW: Double, val storedH: Double, val placedW: Double, val placedH: Double)

/** One child's place in a line of auto layout: its length along the line and depth across, whether it fills along it and whether it stretches across. */
private class Slot(var main: Double, var cross: Double, val fill: Boolean, val stretch: Boolean)

/** One line of a wrap layout: which children, their slots, how long it is and how deep. */
private class WrapLine(val range: IntRange, val slots: List<Slot>, val along: Double, val depth: Double)

/** A size asked of a pass: the node, the width offered it (only a filling one reads it), and the rules it is measured under. */
private data class SizeKey(val id: String, val offered: Long?, val rules: As)

/**
 * The rules a size is asked under, besides the node's own: stretched across a column (its width
 * read as filling), or measured as it hugs along an axis it fills, where the laid-out parent hugs
 * that axis and so offers no room to fill (a menu's items in a menu that hugs them).
 */
private data class As(val stretch: Boolean = false, val hugW: Boolean = false, val hugH: Boolean = false)

/**
 * One [place]: the scene, the measure, what is placed so far, and every size already worked out.
 * A size depends only on the node, the width offered it and the scene, so each is measured once
 * however many ancestors ask (B45).
 */
private class LayoutPass(val scene: DesignScene, val measure: Measure) {
    val placed = Placed()
    private val sizes = HashMap<SizeKey, Dims>()

    /** Each group's children's bounds, worked out once a pass. */
    private val bounds = HashMap<String, Rect?>()

    /** [id] when it is placed at all: shown, of a kind this phone knows, and hanging from a page within [DesignScene.MAX_DEPTH]. */
    fun visible(id: String): DesignNode? {
        val n = scene.node(id) ?: return null
        if (n.isHidden || n.kind !in DesignValues.kinds || scene.depthOf(id) == null || scene.tooDeep(id)) return null
        return n
    }

    /** The children [id] lays out: those shown and not absolute. */
    fun flow(id: String): List<String> = scene.children(id).filter { c -> visible(c)?.let { !isAbsolute(it) } == true }

    /** A node's own size, given the width a laid-out parent offers it. */
    fun size(id: String, offered: Double?): Dims = sizeAs(id, offered, As())

    /** A node's size when a column stretches it [width] wide: as a filling width reads it, keeping its height rule (B23). */
    fun stretched(id: String, width: Double): Dims = sizeAs(id, width, As(stretch = true))

    private fun sizeAs(id: String, offered: Double?, rules: As): Dims {
        val n = scene.node(id) ?: return Dims(0.0, 0.0)
        val ownW = wSizing(n)
        val ownH = hSizing(n)
        val ws = when {
            rules.stretch -> Sizing.Fill
            ownW == Sizing.Fill && rules.hugW -> Sizing.Hug
            else -> ownW
        }
        val hs = if (ownH == Sizing.Fill && rules.hugH) Sizing.Hug else ownH
        // Only a filling width reads what is offered: every other offer is the same question.
        val offer = if (ws == Sizing.Fill) offered else null
        // Rules that change nothing for this node ask the same question as none.
        val asked = As(stretch = rules.stretch && ownW != Sizing.Fill, hugW = ws != ownW && !rules.stretch, hugH = hs != ownH)
        val key = SizeKey(id, offer?.toRawBits(), asked)
        sizes[key]?.let { return it }
        val size = sizeOf(n, ws, hs, offer)
        sizes[key] = size
        return size
    }

    /** A node's size under width rule [ws] and height rule [hs], offered [offered] px across: its kind's rule, then its min and max, and no more than [MAX_COORD] (B111). */
    private fun sizeOf(n: DesignNode, ws: Sizing, hs: Sizing, offered: Double?): Dims {
        val (w, h) = when {
            // A group is as big as its children, whatever its own w/h or sizing say (B2).
            n.kind == "group" -> return groupBounds(n)?.let { Dims(it.w, it.h) } ?: Dims(0.0, 0.0)
            n.kind == "text" -> textSize(n, ws, hs, offered)
            n.kind == "sticky" -> stickySize(n, ws, hs, offered)
            n.kind == "frame" && lays(n) -> frameSize(n, ws, hs, offered)
            // A filling width takes what is offered; nothing offers one outside a layout.
            else -> Dims(if (ws == Sizing.Fill) offered ?: n.w else n.w, n.h)
        }
        return Dims(clampW(n, w).coerceIn(0.0, MAX_COORD), clampH(n, h).coerceIn(0.0, MAX_COORD))
    }

    /** Whether [n] lays out its children (a frame with a layout this phone knows). */
    private fun lays(n: DesignNode): Boolean = layoutMode(n).let { it != LayoutMode.None && it != LayoutMode.Unknown }

    /** A text's words measured wrapped at [wrap] px (never past [MAX_COORD]), or unwrapped. */
    private fun words(n: DesignNode, wrap: Double?): TextSize = measure.measure(n, wrap?.let { min(it, MAX_COORD) })

    /** A text: its words wrapped at its width (unwrapped while it hugs them, up to `max_w`), as tall as its lines unless its height is its own. */
    private fun textSize(n: DesignNode, ws: Sizing, hs: Sizing, offered: Double?): Dims {
        val (w, lines) = when (ws) {
            Sizing.Hug -> {
                val m = words(n, null)
                // Past its widest it wraps there: a chat bubble's text.
                val widest = n.maxW?.takeIf { m.w > it }
                if (widest != null) Dims(widest, words(n, widest).h) else Dims(m.w, m.h)
            }
            Sizing.Fixed -> {
                val w = clampW(n, n.w)
                Dims(w, words(n, w).h)
            }
            // A filling text that no width is offered yet measures at its own w, or unwrapped.
            Sizing.Fill -> when (val w = (offered ?: n.w.takeIf { it > 0.0 })?.let { clampW(n, it) }) {
                null -> words(n, null).let { Dims(it.w, it.h) }
                else -> Dims(w, words(n, w).h)
            }
        }
        return Dims(w, if (hs == Sizing.Hug) lines else n.h)
    }

    /** A sticky: as wide as it says (or as it fills, or its words while it hugs), and as tall as it says, growing to fit its words; hugging, exactly as tall as they are (B119). */
    private fun stickySize(n: DesignNode, ws: Sizing, hs: Sizing, offered: Double?): Dims {
        val pad = 2.0 * STICKY_PAD
        val own = if (n.w > 0.0) n.w else STICKY_SIZE
        val w = clampW(
            n,
            when (ws) {
                Sizing.Fill -> offered ?: own
                Sizing.Hug -> words(n, null).w + pad
                Sizing.Fixed -> own
            },
        )
        val words = words(n, max(w - pad, 1.0)).h + pad
        val h = if (hs == Sizing.Hug) words else max(if (n.h > 0.0) n.h else STICKY_SIZE, words)
        return Dims(w, h)
    }

    /** A laid-out frame: its width first (its own, what is offered, or what its children take while it hugs, within its min and max), then its children laid at that width, its height hugging what they take. */
    private fun frameSize(n: DesignNode, ws: Sizing, hs: Sizing, offered: Double?): Dims {
        val hugW = ws == Sizing.Hug
        val hugH = hs == Sizing.Hug
        val own: Double? = when (ws) {
            Sizing.Fixed -> n.w
            Sizing.Fill -> offered ?: n.w.takeIf { it > 0.0 }
            Sizing.Hug -> null
        }
        // A box of its own size: what is inside does not change it.
        if (!hugW && !hugH) return Dims(own ?: n.w, n.h)
        val (pt, pr, pb, pl) = paddingEdges(n)
        fun inner(w: Double) = max(w - pl - pr, 0.0)
        // A wrap that hugs its width breaks its lines at its max_w (one line without).
        val room = own ?: n.maxW?.takeIf { layoutMode(n) == LayoutMode.Wrap }
        val content = content(n, room?.let { inner(clampW(n, it)) })
        var contentH = content.h
        var w = own ?: n.w
        if (hugW) {
            val hugged = content.w + pl + pr
            w = min(clampW(n, hugged), MAX_COORD)
            // Past its min or max it is laid out again at the width it takes.
            if (w != hugged) contentH = content(n, inner(w)).h
        }
        return Dims(w, if (hugH) contentH + pt + pb else n.h)
    }

    /** What a laid-out frame's children take across and down when laid in [room] px of inner width (none while the frame hugs it). */
    private fun content(n: DesignNode, room: Double?): Dims {
        val kids = flow(n.id)
        val gap = n.gap ?: 0.0
        if (layoutMode(n) == LayoutMode.Wrap) {
            val lines = wrapLines(n, kids, room)
            val wide = lines.fold(0.0) { a, l -> max(a, l.along) }
            val rows = (n.rowGap ?: gap) * max(lines.size - 1, 0)
            var deep = 0.0
            for (l in lines) deep += l.depth
            return Dims(max(wide, 0.0), max(deep + rows, 0.0))
        }
        val row = layoutMode(n) == LayoutMode.Row
        val slots = lineSizes(kids, if (row) room else null, if (row) null else room, row, false, itemsOf(n), gap)
        var mains = 0.0
        for (s in slots) mains += s.main
        val along = max(mains + gap * max(slots.size - 1, 0), 0.0)
        val across = slots.fold(0.0) { a, s -> max(a, s.cross) }
        return if (row) Dims(along, across) else Dims(across, along)
    }

    /**
     * A wrap layout's lines in [room] px (none: one line). Children break onto lines by what a
     * row measures them at (a filling one by its own w, within its min and max), then each line is
     * laid as a row: fills share its room, and it is as tall as its tallest child, stretched ones
     * taking that.
     */
    private fun wrapLines(n: DesignNode, kids: List<String>, room: Double?): List<WrapLine> {
        val gap = n.gap ?: 0.0
        val items = itemsOf(n)
        val basis = kids.map { c ->
            val k = scene.node(c)
            if (k != null && k.kind != "group" && wSizing(k) == Sizing.Fill) clampW(k, k.w) else size(c, null).w
        }
        return breakLines(basis, room, gap).map { range ->
            val line = kids.subList(range.first, range.last + 1)
            val slots = lineSizes(line, room, null, true, false, items, gap)
            val depth = slots.fold(0.0) { a, s -> max(a, s.cross) }
            for ((i, s) in slots.withIndex()) {
                if (s.stretch) s.cross = scene.node(line[i])?.let { clampH(it, depth) } ?: depth
            }
            var mains = 0.0
            for (s in slots) mains += s.main
            WrapLine(range, slots, mains + gap * (line.size - 1), depth)
        }
    }

    /**
     * One line of auto layout: each child's length along it and depth across. Filling children
     * share what [main] leaves, each within its min and max; stretched ones take [cross],
     * measured at it across a column (B23); a group does neither (B2). Without [main] or [cross]
     * (a frame that hugs), each takes its own, one that fills measured as it would hug.
     * [hugsMain]: the frame hugs along the line, so [main] is what its children take, and those
     * that fill keep what they measured unless a min or max gave it more or less room.
     */
    private fun lineSizes(kids: List<String>, main: Double?, cross: Double?, row: Boolean, hugsMain: Boolean, items: String, gap: Double): List<Slot> {
        val slots = kids.map { c ->
            val k = scene.node(c)!!
            val mainRule = if (row) wSizing(k) else hSizing(k)
            val crossRule = if (row) hSizing(k) else wSizing(k)
            val group = k.kind == "group"
            val stretch = !group && (crossRule == Sizing.Fill || items == "stretch")
            // Along an axis the frame offers no room on, filling is hugging.
            val hugMain = main == null || hugsMain
            val hugCross = cross == null
            val rules = if (row) As(hugW = hugMain, hugH = hugCross) else As(hugW = hugCross, hugH = hugMain)
            val (w, h) = when {
                row -> sizeAs(c, null, rules)
                cross != null && stretch -> sizeAs(c, cross, rules.copy(stretch = true))
                else -> sizeAs(c, cross, rules)
            }
            var across = if (row) h else w
            if (row && stretch && cross != null) across = clampH(k, cross)
            Slot(if (row) w else h, across, !group && mainRule == Sizing.Fill, stretch)
        }
        val fills = slots.indices.filter { slots[it].fill }
        if (main == null || fills.isEmpty()) return slots
        var taken = 0.0
        for (s in slots) if (!s.fill) taken += s.main
        val gaps = gap * (slots.size - 1)
        if (hugsMain) {
            // The frame took what they measured: they keep it, rather than equal shares of it.
            var measured = 0.0
            for (i in fills) measured += slots[i].main
            if (abs(main - taken - gaps - measured) < 0.5) return slots
        }
        val limits = fills.map { i -> scene.node(kids[i])?.let { k -> if (row) k.minW to k.maxW else k.minH to k.maxH } ?: (null to null) }
        for ((i, share) in fills.zip(shares(main - taken - gaps, limits))) {
            slots[i].main = share
            if (row) {
                // A filling text wraps at its share, a filling frame lays out at it: their
                // height follows, unless the row stretches them.
                val h = size(kids[i], share).h
                if (!(slots[i].stretch && cross != null)) slots[i].cross = h
            }
        }
        return slots
    }

    /** A group's children's bounds from its own point, shown children only (null with none shown): each child's box at its x/y, a child group's own bounds where it sits. */
    fun groupBounds(g: DesignNode): Rect? {
        if (bounds.containsKey(g.id)) return bounds[g.id]
        var out: Rect? = null
        for (c in scene.children(g.id)) {
            val k = visible(c) ?: continue
            val r = if (k.kind == "group") {
                (groupBounds(k) ?: continue).translate(k.x, k.y)
            } else {
                val s = size(c, null)
                Rect(k.x, k.y, s.w, s.h)
            }
            out = out?.union(r) ?: r
        }
        bounds[g.id] = out
        return out
    }

    /**
     * A child of a page, group or free frame, or an absolute one of a laid-out frame: at its own
     * x/y from [origin] (a group: its children's bounds where they sit), carried by its pins when
     * its frame is drawn at another size than it stores ([fit]).
     */
    fun freeChild(id: String, origin: Pt, fit: Fit?) {
        val n = visible(id) ?: return
        val f = fit?.takeIf { isPinned(n) }
        val (pinX, pinY) = pinsOf(n)
        if (n.kind == "group") {
            // A group's size is its children's: a pin only moves it.
            val b = groupBounds(n) ?: Rect.ZERO
            val x = if (f == null) n.x else followAxis(pinX, n.x, b.w, f.storedW, f.placedW).first
            val y = if (f == null) n.y else followAxis(pinY, n.y, b.h, f.storedH, f.placedH).first
            arrange(id, Rect(origin.x + x + b.x, origin.y + y + b.y, b.w, b.h))
            return
        }
        val size = size(id, null)
        var w = size.w
        var h = size.h
        var x = n.x
        var y = n.y
        if (f != null) {
            val (fx, fw0) = followAxis(pinX, x, w, f.storedW, f.placedW)
            x = fx
            val fw = if (pinX.stretches) min(clampW(n, fw0), MAX_COORD) else w
            if (fw != w) {
                // Stretched wider or narrower, its words or children reflow there and its height follows.
                h = stretched(id, fw).h
                w = fw
            }
            val (fy, fh) = followAxis(pinY, y, h, f.storedH, f.placedH)
            y = fy
            if (pinY.stretches) h = min(clampH(n, fh), MAX_COORD)
        }
        arrange(id, Rect(origin.x + x, origin.y + y, w, h))
    }

    /** Places [id] at [at] (a group: where its children's bounds land), then what is inside it. */
    private fun arrange(id: String, at: Rect) {
        val n = scene.node(id) ?: return
        // Its children are placed from where it is placed, so paint and the phone's fit agree on them too.
        val rect = bounded(at)
        val children = scene.children(id)
        val mode = layoutMode(n)
        when {
            n.kind == "group" -> {
                // `rect` is where its children's bounds land, freely or in a layout's slot (B2):
                // they count from there less where their bounds start.
                val b = groupBounds(n) ?: Rect.ZERO
                val o = Pt(limit(rect.x - b.x), limit(rect.y - b.y))
                placed.originMap[id] = o
                for (child in children) freeChild(child, o, null)
                placed.rectMap[id] = Rect.bounds(children.mapNotNull { placed.rectMap[it] }) ?: Rect(o.x, o.y, 0.0, 0.0)
            }
            mode == LayoutMode.None -> {
                placed.rectMap[id] = rect
                val fit = if (n.kind == "frame") Fit(n.w, n.h, rect.w, rect.h) else null
                for (child in children) freeChild(child, Pt(rect.x, rect.y), fit)
                scroll(n, rect)
            }
            // A layout this phone does not know: the frame at its own size, what it holds left out.
            mode == LayoutMode.Unknown -> placed.rectMap[id] = rect
            else -> {
                placed.rectMap[id] = rect
                layOut(n, rect)
                // Absolute children after the flow, from the frame's corner (not its padding), in no one's way.
                val fit = Fit(n.w, n.h, rect.w, rect.h)
                for (child in children) if (scene.node(child)?.let(::isAbsolute) == true) freeChild(child, Pt(rect.x, rect.y), fit)
                scroll(n, rect)
            }
        }
    }

    /** A scrolling frame, its children placed: the box its content spans, to its furthest child and a laid-out frame's padding after its flow. The phone never scrolls it. */
    private fun scroll(n: DesignNode, rect: Rect) {
        if (n.scroll == null || n.kind != "frame") return
        val laid = layoutMode(n) != LayoutMode.None
        val pad = if (laid) paddingEdges(n) else DoubleArray(4)
        var right = rect.right
        var bottom = rect.bottom
        for (c in scene.children(n.id)) {
            val r = placed.rectMap[c] ?: continue
            val padded = laid && scene.node(c)?.let(::isAbsolute) != true
            right = max(right, r.right + if (padded) pad[1] else 0.0)
            bottom = max(bottom, r.bottom + if (padded) pad[2] else 0.0)
        }
        placed.scrollMap[n.id] = Rect(rect.x, rect.y, right - rect.x, bottom - rect.y)
    }

    /** Auto layout: children one after another along the frame's direction, inside its padding. */
    private fun layOut(frame: DesignNode, rect: Rect) {
        val (pt, pr, pb, pl) = paddingEdges(frame)
        val inner = Rect(rect.x + pl, rect.y + pt, max(rect.w - pl - pr, 0.0), max(rect.h - pt - pb, 0.0))
        val kids = flow(frame.id)
        if (layoutMode(frame) == LayoutMode.Wrap) {
            // Lines from the top, row_gap apart (default gap), each placed as a row.
            val rowGap = frame.rowGap ?: (frame.gap ?: 0.0)
            var at = 0.0
            for (line in wrapLines(frame, kids, inner.w)) {
                val band = Rect(inner.x, inner.y + at, inner.w, line.depth)
                placeLine(frame, kids.subList(line.range.first, line.range.last + 1), line.slots, band, true)
                at += line.depth + rowGap
            }
            return
        }
        val row = layoutMode(frame) == LayoutMode.Row
        val hugsMain = (if (row) wSizing(frame) else hSizing(frame)) == Sizing.Hug
        val slots = lineSizes(kids, if (row) inner.w else inner.h, if (row) inner.h else inner.w, row, hugsMain, itemsOf(frame), frame.gap ?: 0.0)
        placeLine(frame, kids, slots, inner, row)
    }

    /** Places one line's children in [inner]: `justify` along it, `items` across. */
    private fun placeLine(frame: DesignNode, kids: List<String>, slots: List<Slot>, inner: Rect, row: Boolean) {
        val length = if (row) inner.w else inner.h
        val depth = if (row) inner.h else inner.w
        var gap = frame.gap ?: 0.0
        var mains = 0.0
        for (s in slots) mains += s.main
        val free = length - mains - gap * max(slots.size - 1, 0)
        var at = when (justifyOf(frame)) {
            "center" -> free / 2.0
            "end" -> free
            "space_between" -> {
                // What does not fit overflows from the start, as in CSS, rather than overlapping (B114).
                if (slots.size > 1 && slots.none { it.fill }) gap += max(free, 0.0) / (slots.size - 1)
                0.0
            }
            else -> 0.0
        }
        val items = itemsOf(frame)
        for ((i, c) in kids.withIndex()) {
            val s = slots[i]
            val offset = when (items) {
                "center" -> (depth - s.cross) / 2.0
                "end" -> depth - s.cross
                else -> 0.0
            }
            val r = if (row) Rect(inner.x + at, inner.y + offset, s.main, s.cross) else Rect(inner.x + offset, inner.y + at, s.cross, s.main)
            if (!row && s.stretch && scene.node(c)?.let { it.kind == "text" && wSizing(it) == Sizing.Hug } == true) {
                // A hugging text the column stretched was measured wrapped at its width.
                placed.wrapMap[c] = s.cross
            }
            arrange(c, r)
            at += s.main + gap
        }
    }
}
