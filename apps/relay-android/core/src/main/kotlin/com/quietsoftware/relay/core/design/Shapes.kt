package com.quietsoftware.relay.core.design

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/*
 * Geometry of the richer nodes, a port of crates/relay-client/src/design/shapes.rs and
 * bounds.rs: a polygon's or star's corners, an arc's capped ends, the pieces a line or path is
 * stroked along and its arrowheads, rotation about a node's centre, where a connector runs once
 * its ends are attached to other nodes, and the box everything a node paints fits in. The phone
 * draws with these, so a design is drawn where the PC draws it. A change there changes this file
 * in the same piece of work, with a case in core/src/test/resources/design/layout-cases.json,
 * which DesignShapesTest runs here and crates/relay-client/tests/phone_fixtures.rs runs against
 * the PC's code. Hit testing, snapping and the editing helpers are the PC's alone: the phone only
 * shows a design.
 *
 * Numbers are compared as the PC compares them (a point is (0, 0) when both its numbers are 0,
 * whatever their sign), never through a [Pt]'s equality.
 */

/** A turn a node is drawn through: its centre and degrees clockwise. */
data class Turn(val cx: Double, val cy: Double, val degrees: Double)

/** Where a node's centre is drawn on the page and its whole turn, its own and its ancestors' (shapes.rs `page_frame`). */
data class PageFrame(val centre: Pt, val turn: Double)

/** A line's points as drawn, and whether they are a cubic curve's `[start, control, control, end]`. */
data class LinePoints(val points: List<Pt>, val curved: Boolean)

/** An ellipse's arc: start and sweep in degrees (clockwise from the top, the sweep within ±360), its hole a fraction of the radius. */
data class ArcSpec(val start: Double, val sweep: Double, val inner: Double)

/** One end of a ring segment: where it meets the outer and the inner edge, and the unit tangent pointing away from the drawn part. */
data class ArcEnd(val outer: Pt, val inner: Pt, val out: Pt) {
    /** The middle of the end, between its edges: a round cap's centre. */
    val mid: Pt get() = Pt((outer.x + inner.x) / 2.0, (outer.y + inner.y) / 2.0)

    /** Half the ring's thickness at this end: how far its cap reaches past it. */
    val capRadius: Double get() = hypot(outer.x - inner.x, outer.y - inner.y) / 2.0
}

/** A ring segment's two ends and how they are capped, `round` or `square`. */
data class ArcEnds(val cap: String, val start: ArcEnd, val end: ArcEnd)

/** One piece of a stroke, from where the piece before it ended. */
sealed interface Seg {
    /** Straight to a point. */
    data class Line(val to: Pt) : Seg

    /** A quadratic curve: its control point and its end. */
    data class Quad(val c: Pt, val to: Pt) : Seg

    /** A cubic curve: its two control points and its end. */
    data class Cubic(val c1: Pt, val c2: Pt, val to: Pt) : Seg
}

/** A stroke: where it starts and its pieces from there. */
data class PathSegs(val start: Pt, val segs: List<Seg>)

/** An arrowhead: its tip and its two wings, a triangle. */
data class Arrowhead(val tip: Pt, val left: Pt, val right: Pt) {
    val points: List<Pt> get() = listOf(tip, left, right)
}

/** How far an arrowhead's wings reach to each side of its line, as a fraction of its length. */
const val ARROW_WING = 0.55

private const val TAU = 2.0 * PI
private const val QUARTER = PI / 2.0

/**
 * A line's or path's own points, relative to its box; null when it has none, or when one is not
 * an x, y pair (the PC sets such a value aside and draws the node without it, design/read.rs).
 */
fun pointsOf(n: DesignNode): List<Pt>? {
    val points = n.points ?: return null
    if (points.any { it.size != 2 }) return null
    return points.map { Pt(it[0], it[1]) }
}

/** A polygon's (or star's) corners inside [r], first at the top centre, clockwise (shapes.rs `polygon`). */
fun polygon(r: Rect, sides: Int, star: Double?): List<Pt> {
    val n = sides.coerceIn(3, 24)
    val count = if (star != null) n * 2 else n
    return (0 until count).map { i ->
        val angle = -QUARTER + TAU * i / count
        val k = if (star != null && i % 2 == 1) star else 1.0
        Pt(r.cx + r.w / 2.0 * k * cos(angle), r.cy + r.h / 2.0 * k * sin(angle))
    }
}

/** The corners of a polygon node in its placed box (a star has 5 points unless it says, B57), mirrored as it is flipped (shapes.rs `node_polygon`). */
fun nodePolygon(n: DesignNode, r: Rect): List<Pt> = polygon(r, polygonSides(n), n.star).map { flipped(n, r, it) }

/** [p] mirrored inside [r] as [n] is flipped (`flip_x`, `flip_y`). */
fun flipped(n: DesignNode, r: Rect, p: Pt): Pt =
    Pt(if (n.flipX == true) 2.0 * r.cx - p.x else p.x, if (n.flipY == true) 2.0 * r.cy - p.y else p.y)

/** An ellipse's arc; null for a whole ellipse (shapes.rs `arc`). */
fun arcOf(n: DesignNode): ArcSpec? {
    val a = n.arc?.takeIf { n.kind == "ellipse" } ?: return null
    val sweep = a.sweep.coerceIn(-360.0, 360.0)
    val inner = a.inner.coerceIn(0.0, 0.99)
    return if (abs(sweep) < 360.0 || inner > 0.0) ArcSpec(a.start, sweep, inner) else null
}

/**
 * A ring segment's two ends, at `start` and at `start + sweep`, and how they are capped: only for
 * a ring segment (`inner` above 0, `sweep` under 360) whose `arc.cap` is round or square; a cap
 * this phone does not know is butt (shapes.rs `arc_ends`).
 */
fun arcEnds(n: DesignNode, r: Rect): ArcEnds? {
    val cap = n.arc?.cap?.takeIf { it == "round" || it == "square" } ?: return null
    val arc = arcOf(n) ?: return null
    if (arc.inner <= 0.0 || abs(arc.sweep) >= 360.0) return null
    val cx = r.cx
    val cy = r.cy
    val rx = r.w / 2.0
    val ry = r.h / 2.0
    fun end(deg: Double, away: Double): ArcEnd {
        val a = Math.toRadians(deg)
        val s = sin(a)
        val c = cos(a)
        fun at(k: Double) = Pt(cx + rx * k * s, cy - ry * k * c)
        // The way the ring runs at this angle, clockwise.
        val tx = rx * c
        val ty = ry * s
        val len = max(hypot(tx, ty), 1e-9)
        return ArcEnd(at(1.0), at(arc.inner), Pt(away * tx / len, away * ty / len))
    }
    val way = if (arc.sweep >= 0.0) 1.0 else -1.0
    return ArcEnds(cap, end(arc.start, -way), end(arc.start + arc.sweep, way))
}

/** How far a ring segment's capped ends may reach past its box [r]: the larger end's cap radius, else 0 (shapes.rs `cap_reach`). */
fun capReach(n: DesignNode, r: Rect): Double = arcEnds(n, r)?.let { max(it.start.capRadius, it.end.capRadius) } ?: 0.0

/** Whether [n] is a connector: a line drawn from its ends (`from`, `to`) or along a `route`, not from its own points. */
fun routed(n: DesignNode): Boolean = n.kind == "line" && (n.from != null || n.to != null || n.route != null)

/** Whether [n] is drawn as a closed shape: a free line or path of three or more points with `closed`. */
fun closedShape(n: DesignNode): Boolean = fillableLine(n) && n.closed == true

/** Whether a free line or path has an inside to fill: three or more points, closed or not. */
fun fillableLine(n: DesignNode): Boolean = (n.kind == "line" || n.kind == "path") && !routed(n) && (pointsOf(n)?.size ?: 0) > 2

/** A smooth curve through [pts] (Catmull-Rom) as cubic pieces from `pts[0]`; closed, it runs back to the first point (shapes.rs `smooth_segments`). */
fun smoothSegments(pts: List<Pt>, closed: Boolean): List<Seg.Cubic> {
    val n = pts.size
    if (n < 2) return emptyList()
    fun at(i: Int): Pt = if (closed) pts[Math.floorMod(i, n)] else pts[i.coerceIn(0, n - 1)]
    val count = if (closed) n else n - 1
    return (0 until count).map { i ->
        val p0 = at(i - 1)
        val p1 = at(i)
        val p2 = at(i + 1)
        val p3 = at(i + 2)
        val c1 = Pt(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0)
        val c2 = Pt(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0)
        Seg.Cubic(c1, c2, p2)
    }
}

/**
 * A free line's or path's stroke as the PC paints it, from its first point ([origin] plus each
 * of its points): one point is a dot; a `smooth` line is a Catmull-Rom curve through its points;
 * a path of three or more points runs in quadratic curves turning at each point between the
 * midpoints, then straight to its last; anything else runs straight from point to point; and a
 * closed shape runs back to its start. Null without points (shapes.rs `path_segments`).
 */
fun pathSegments(n: DesignNode, origin: Pt): PathSegs? {
    val points = pointsOf(n)?.takeIf { it.isNotEmpty() } ?: return null
    fun at(p: Pt) = Pt(origin.x + p.x, origin.y + p.y)
    val start = at(points[0])
    val closed = closedShape(n)
    val segs = ArrayList<Seg>(points.size + 1)
    when {
        points.size == 1 -> segs += Seg.Line(Pt(start.x + 0.01, start.y))
        n.kind == "line" && n.smooth == true && points.size > 2 -> segs += smoothSegments(points.map(::at), closed)
        n.kind == "path" && points.size > 2 -> {
            for (i in 1 until points.size - 1) {
                val c = at(points[i])
                val next = at(points[i + 1])
                segs += Seg.Quad(c, Pt((c.x + next.x) / 2.0, (c.y + next.y) / 2.0))
            }
            segs += Seg.Line(at(points[points.size - 1]))
        }
        else -> for (i in 1 until points.size) segs += Seg.Line(at(points[i]))
    }
    if (closed) segs += Seg.Line(start)
    return PathSegs(start, segs)
}

/** A free line's or path's outline in page coordinates as a polyline: [pathSegments] from its box's corner, curves flattened (shapes.rs `stroke_outline`). */
fun strokeOutline(n: DesignNode, r: Rect): List<Pt> {
    val path = pathSegments(n, Pt(r.x, r.y)) ?: return emptyList()
    val out = arrayListOf(path.start)
    var from = path.start
    for (seg in path.segs) {
        val to = when (seg) {
            is Seg.Line -> {
                out += seg.to
                seg.to
            }
            is Seg.Quad -> {
                // The same curve as a cubic: each control two thirds of the way to `c`.
                val c = seg.c
                fun third(p: Pt) = Pt(p.x + 2.0 / 3.0 * (c.x - p.x), p.y + 2.0 / 3.0 * (c.y - p.y))
                flattenInto(out, from, third(from), third(seg.to), seg.to)
                seg.to
            }
            is Seg.Cubic -> {
                flattenInto(out, from, seg.c1, seg.c2, seg.to)
                seg.to
            }
        }
        from = to
    }
    return out
}

/** A cubic's points after its start, in as few steps as keep it within about a quarter pixel (1 to 16). */
private fun flattenInto(out: MutableList<Pt>, a: Pt, c1: Pt, c2: Pt, b: Pt) {
    val bend = max(segmentDistance(c1, a, b), segmentDistance(c2, a, b))
    val steps = ceil(sqrt(bend / 0.25)).toInt().coerceIn(1, 16)
    for (i in 1..steps) out += cubicAt(a, c1, c2, b, i.toDouble() / steps)
}

private fun cubicAt(a: Pt, c1: Pt, c2: Pt, b: Pt, t: Double): Pt {
    val u = 1.0 - t
    fun f(a: Double, b: Double, c: Double, d: Double) = u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
    return Pt(f(a.x, c1.x, c2.x, b.x), f(a.y, c1.y, c2.y, b.y))
}

/** How far [p] is from the segment [a] to [b] (geom.rs `segment_distance`). */
private fun segmentDistance(p: Pt, a: Pt, b: Pt): Double {
    val dx = b.x - a.x
    val dy = b.y - a.y
    val len2 = dx * dx + dy * dy
    val t = if (len2 == 0.0) 0.0 else (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).coerceIn(0.0, 1.0)
    val cx = a.x + t * dx
    val cy = a.y + t * dy
    return sqrt((p.x - cx) * (p.x - cx) + (p.y - cy) * (p.y - cy))
}

/** Points along a cubic curve `[start, control, control, end]`, 17 of them. */
fun flattenCurve(p: List<Pt>): List<Pt> = (0..16).map { cubicAt(p[0], p[1], p[2], p[3], it / 16.0) }

/** The weight a line or path is stroked at: `stroke_width`, else 2, at least 0.5 (shapes.rs `line_width`). */
fun lineWidth(n: DesignNode): Double = max(n.strokeWidth ?: 2.0, 0.5)

/** An arrowhead's length along its line: three and a half times the weight, at least 8 px. */
fun arrowSize(n: DesignNode): Double = max(lineWidth(n) * 3.5, 8.0)

/**
 * The arrowheads a line or path draws, end first, in the coordinates its points are drawn in,
 * with [r] its placed box: one per end that asks for one (`arrow_end`, `arrow_start`) on the last
 * or first segment. A closed shape draws none (shapes.rs `arrowheads`).
 */
fun arrowheads(scene: DesignScene, placed: Placed, n: DesignNode, r: Rect): List<Arrowhead> {
    if (!(n.kind == "line" || n.kind == "path") || closedShape(n) || !(n.arrowEnd == true || n.arrowStart == true)) return emptyList()
    val pts = if (routed(n)) linePoints(scene, placed, n).points else pointsOf(n).orEmpty().map { Pt(r.x + it.x, r.y + it.y) }
    if (pts.size < 2) return emptyList()
    val last = pts.size - 1
    val size = arrowSize(n)
    return listOf(Triple(n.arrowEnd, pts[last - 1], pts[last]), Triple(n.arrowStart, pts[1], pts[0]))
        .filter { it.first == true }
        .mapNotNull { (_, from, tip) ->
            val dx = tip.x - from.x
            val dy = tip.y - from.y
            val len = hypot(dx, dy)
            if (len < 0.001) return@mapNotNull null
            val ux = dx / len
            val uy = dy / len
            val back = Pt(tip.x - ux * size, tip.y - uy * size)
            val px = -uy * size * ARROW_WING
            val py = ux * size * ARROW_WING
            Arrowhead(tip, Pt(back.x + px, back.y + py), Pt(back.x - px, back.y - py))
        }
}

/** A node's turn, 0 for none. */
fun rotation(n: DesignNode): Double = n.rotation ?: 0.0

/** The box [r] covers once turned [degrees] clockwise about ([cx], [cy]) (shapes.rs `rotated_bounds_about`). */
fun rotatedBoundsAbout(r: Rect, degrees: Double, cx: Double, cy: Double): Rect {
    if (degrees == 0.0) return r
    val corners = listOf(Pt(r.x, r.y), Pt(r.right, r.y), Pt(r.right, r.bottom), Pt(r.x, r.bottom))
    return around(corners.map { rotate(it.x, it.y, cx, cy, degrees) }) ?: r
}

/** The smallest box holding [points], null for none (geom.rs `Rect::around`). */
fun around(points: Iterable<Pt>): Rect? {
    var x0 = Double.POSITIVE_INFINITY
    var y0 = Double.POSITIVE_INFINITY
    var x1 = Double.NEGATIVE_INFINITY
    var y1 = Double.NEGATIVE_INFINITY
    for (p in points) {
        x0 = min(x0, p.x)
        y0 = min(y0, p.y)
        x1 = max(x1, p.x)
        y1 = max(y1, p.y)
    }
    return if (x0 <= x1 && y0 <= y1) Rect(x0, y0, x1 - x0, y1 - y0) else null
}

/** [id]'s own turn about its placed centre, when it has one. */
private fun turnOf(scene: DesignScene, placed: Placed, id: String): Turn? {
    val turn = rotation(scene.node(id) ?: return null)
    val r = placed.rect(id) ?: return null
    return if (turn != 0.0) Turn(r.cx, r.cy, turn) else null
}

/** The turns [id] is drawn inside, outermost first: each turned ancestor's centre and degrees (shapes.rs `turned_ancestors`). */
fun turnedAncestors(scene: DesignScene, placed: Placed, id: String): List<Turn> =
    scene.ancestors(id).mapNotNull { turnOf(scene, placed, it) }.asReversed()

/** The turns a point inside [id] is drawn through: its ancestors' and its own, outermost first. */
private fun frameTurns(scene: DesignScene, placed: Placed, id: String): List<Turn> =
    turnedAncestors(scene, placed, id) + listOfNotNull(turnOf(scene, placed, id))

/**
 * Where [id] is drawn on the page: its placed box moved to where its ancestors' turns carry its
 * centre, and its whole turn, its own and theirs (shapes.rs `visual`).
 */
fun visual(scene: DesignScene, placed: Placed, id: String): Pair<Rect, Double>? {
    val r = placed.rect(id) ?: return null
    var turn = rotation(scene.node(id) ?: return null)
    val ancestors = turnedAncestors(scene, placed, id)
    if (ancestors.isEmpty()) return r to turn
    var c = Pt(r.cx, r.cy)
    for (t in ancestors.asReversed()) {
        c = rotate(c.x, c.y, t.cx, t.cy, t.degrees)
        turn += t.degrees
    }
    return Rect(c.x - r.w / 2.0, c.y - r.h / 2.0, r.w, r.h) to turn
}

/** Where [id]'s centre is drawn on the page and its whole turn: the frame a connector meets it in. */
fun pageFrame(scene: DesignScene, placed: Placed, id: String): PageFrame? =
    visual(scene, placed, id)?.let { (r, turn) -> PageFrame(Pt(r.cx, r.cy), turn) }

/**
 * What [n] draws before its own turn, as a box in its parent's coordinates: a line's or path's
 * drawn points, wherever its stored box is, and any other node's placed box; null for what is not
 * placed (shapes.rs `drawn_box`).
 */
fun drawnBox(scene: DesignScene, placed: Placed, n: DesignNode): Rect? {
    val r = placed.rect(n.id) ?: return null
    val pts = when {
        routed(n) -> linePoints(scene, placed, n).let { if (it.curved) flattenCurve(it.points) else it.points }
        n.kind == "line" || n.kind == "path" -> strokeOutline(n, r)
        else -> return r
    }
    return around(pts) ?: r
}

/** A rounded corner: its radius, its circle's centre and the way out of the box there. */
private class Corner(val radius: Double, val cx: Double, val cy: Double, val sx: Double, val sy: Double)

/** Each rounded corner of [r], top-left first, clockwise. */
private fun corners(r: Rect, radii: DoubleArray): List<Corner> = listOf(
    Corner(radii[0], r.x + radii[0], r.y + radii[0], -1.0, -1.0),
    Corner(radii[1], r.right - radii[1], r.y + radii[1], 1.0, -1.0),
    Corner(radii[2], r.right - radii[2], r.bottom - radii[2], 1.0, 1.0),
    Corner(radii[3], r.x + radii[3], r.bottom - radii[3], -1.0, 1.0),
)

/**
 * Where the ray from [from] along [dir] (page coordinates) last leaves node [n]'s outline as it
 * is drawn, with [r] its placed box and [frame] its [pageFrame]: its box with its rounded corners,
 * its ellipse or its polygon, turned. Null when the ray misses it (shapes.rs `exit_point`).
 */
private fun exitPoint(n: DesignNode, r: Rect, frame: PageFrame, from: Pt, dir: Pt): Pt? {
    if (dir.x == 0.0 && dir.y == 0.0) return null
    val centre = frame.centre
    val turn = frame.turn
    // In the node's own frame: unturned, its box around its drawn centre.
    val o = if (turn == 0.0) from else rotate(from.x, from.y, centre.x, centre.y, -turn)
    val d = if (turn == 0.0) dir else rotate(dir.x, dir.y, 0.0, 0.0, -turn)
    val own = if (centre.x == r.cx && centre.y == r.cy) r else Rect(centre.x - r.w / 2.0, centre.y - r.h / 2.0, r.w, r.h)
    val rel = Pt(o.x - centre.x, o.y - centre.y)
    val hw = r.w / 2.0
    val hh = r.h / 2.0
    val t = when (n.kind) {
        "ellipse" -> ellipseExit(hw, hh, rel, d)
        "polygon" -> polygonExit(nodePolygon(n, own), o, d)
        else -> boxExit(hw, hh, cornerRadii(n, own), rel, d)
    } ?: return null
    val p = Pt(o.x + d.x * t, o.y + d.y * t)
    return if (turn == 0.0) p else rotate(p.x, p.y, centre.x, centre.y, turn)
}

/**
 * How far along [d] the ray from [o] (both from a box's centre) leaves the box of half sizes
 * [hw] by [hh] with corners rounded by [radii]; null when it misses, a ray that crosses only the
 * empty part of a rounded corner included.
 */
private fun boxExit(hw: Double, hh: Double, radii: DoubleArray, o: Pt, d: Pt): Double? {
    var near = Double.NEGATIVE_INFINITY
    var far = Double.POSITIVE_INFINITY
    for ((oo, dd, half) in listOf(Triple(o.x, d.x, hw), Triple(o.y, d.y, hh))) {
        if (dd == 0.0) {
            if (abs(oo) > half) return null
            continue
        }
        val a = (-half - oo) / dd
        val b = (half - oo) / dd
        near = max(near, min(a, b))
        far = min(far, max(a, b))
    }
    if (near > far || far < 0.0) return null
    // Leaving through a rounded corner: where the ray crosses the corner's circle instead.
    val p = Pt(o.x + d.x * far, o.y + d.y * far)
    val corner = corners(Rect(-hw, -hh, 2.0 * hw, 2.0 * hh), radii)
        .firstOrNull { c -> c.radius > 0.0 && (p.x - c.cx) * c.sx > 0.0 && (p.y - c.cy) * c.sy > 0.0 }
    return if (corner != null) circleExit(Pt(o.x - corner.cx, o.y - corner.cy), d, corner.radius) else far
}

/**
 * How far [n]'s outline as drawn reaches along the unit direction [u] from its centre, unturned,
 * with [r] its placed box: a polygon's farthest corner, an ellipse's edge, or a box's farthest
 * corner circle (shapes.rs `reach_along`).
 */
private fun reachAlong(n: DesignNode, r: Rect, u: Pt): Double {
    val hw = r.w / 2.0
    val hh = r.h / 2.0
    val own = Rect(-hw, -hh, r.w, r.h)
    fun dot(x: Double, y: Double) = x * u.x + y * u.y
    return when (n.kind) {
        "ellipse" -> hypot(hw * u.x, hh * u.y)
        "polygon" -> nodePolygon(n, own).fold(Double.NEGATIVE_INFINITY) { a, p -> max(a, dot(p.x, p.y)) }
        else -> corners(own, cornerRadii(n, own)).fold(Double.NEGATIVE_INFINITY) { a, c -> max(a, dot(c.cx, c.cy) + c.radius) }
    }
}

/** How far along [d] the ray from [o] (relative to a circle's centre) last crosses the circle of radius [c]. */
private fun circleExit(o: Pt, d: Pt, c: Double): Double? {
    val a = d.x * d.x + d.y * d.y
    val b = 2.0 * (o.x * d.x + o.y * d.y)
    val disc = b * b - 4.0 * a * (o.x * o.x + o.y * o.y - c * c)
    val t = (-b + sqrt(max(disc, -1.0))) / (2.0 * a)
    return if (disc >= 0.0 && t >= 0.0) t else null
}

/** How far along [d] the ray from [o] (both from the centre) leaves an ellipse of half sizes [hw] by [hh]. */
private fun ellipseExit(hw: Double, hh: Double, o: Pt, d: Pt): Double? {
    val w = max(hw, 1e-9)
    val h = max(hh, 1e-9)
    if (o.x == 0.0 && o.y == 0.0) {
        val u = d.x / w
        val v = d.y / h
        return 1.0 / sqrt(u * u + v * v)
    }
    // Scaled to a unit circle.
    return circleExit(Pt(o.x / w, o.y / h), Pt(d.x / w, d.y / h), 1.0)
}

/** How far along [d] the ray from [o] last leaves the closed polygon [pts]; a star is star-shaped about its centre, so a ray from there leaves it once. */
private fun polygonExit(pts: List<Pt>, o: Pt, d: Pt): Double? {
    fun cross(ax: Double, ay: Double, bx: Double, by: Double) = ax * by - ay * bx
    var best: Double? = null
    for (i in pts.indices) {
        val p = pts[i]
        val q = pts[(i + 1) % pts.size]
        val ex = q.x - p.x
        val ey = q.y - p.y
        val den = cross(d.x, d.y, ex, ey)
        if (abs(den) < 1e-12) continue
        val wx = p.x - o.x
        val wy = p.y - o.y
        val t = cross(wx, wy, ex, ey) / den
        val s = cross(wx, wy, d.x, d.y) / den
        if (t >= 0.0 && s >= -1e-9 && s <= 1.0 + 1e-9) best = best?.let { max(it, t) } ?: t
    }
    return best
}

/** The node an end of [n] is attached to, its placed box and where it is drawn: one that is not [n] itself, still in the scene, shown and placed. */
private class End(val node: DesignNode, val rect: Rect, val frame: PageFrame)

private fun attached(scene: DesignScene, placed: Placed, n: DesignNode, end: String?): End? {
    val id = end?.takeIf { it != n.id } ?: return null
    val m = scene.node(id)?.takeIf { !it.isHidden } ?: return null
    val mr = placed.rect(id) ?: return null
    return End(m, mr, pageFrame(scene, placed, id) ?: PageFrame(Pt(mr.cx, mr.cy), rotation(m)))
}

/**
 * Whether each end of a line, `from` then `to`, is attached to a shown node, so that it is drawn
 * from that node's edge rather than from the line's own box and points. A line attached at both
 * ends draws nothing of its own `x`, `y`, `w`, `h` or `points` (shapes.rs `attached_ends`).
 */
fun attachedEnds(scene: DesignScene, placed: Placed, n: DesignNode): Pair<Boolean, Boolean> =
    (attached(scene, placed, n, n.from) != null) to (attached(scene, placed, n, n.to) != null)

/** Rust's `f64::signum`: 1 for +0 and above, -1 for -0 and below. */
private fun signum(v: Double): Double = if (v.isNaN()) v else Math.copySign(1.0, v)

/**
 * A line's points in the coordinates its own points are drawn in (its parent's, page
 * coordinates for a line on the page): its own, or, where an end is attached (`from`, `to`) to a
 * node still on the page, from that node's outline as it is drawn, turned or not. Each attached
 * end aims at the other end's centre. An elbow route adds its corners, except between two
 * attached nodes whose outlines as drawn overlap across the run, which it joins straight through
 * the middle of the overlap (E-board-22), nor does it jog by under a pixel; a curve is
 * `[start, control, control, end]`. A route this phone does not know is straight (shapes.rs
 * `line_points`). A connector written without points is drawn from its ends (B5).
 */
fun linePoints(scene: DesignScene, placed: Placed, n: DesignNode): LinePoints {
    val r = placed.rect(n.id) ?: Rect.ZERO
    val own = pointsOf(n).orEmpty().map { Pt(r.x + it.x, r.y + it.y) }
    val from = attached(scene, placed, n, n.from)
    val to = attached(scene, placed, n, n.to)
    val route = n.route?.takeIf { it in DesignValues.routes } ?: "straight"
    if (from == null && to == null && route == "straight") return LinePoints(own, false)
    var a = own.firstOrNull() ?: Pt(r.x, r.y)
    var b = own.lastOrNull() ?: Pt(r.right, r.bottom)
    // The nodes it joins are met on the page; a line with no end attached needs no turn.
    val turns = if (from != null || to != null) frameTurns(scene, placed, n.id) else emptyList()
    var turned = 0.0
    for (t in turns) turned += t.degrees
    fun toPage(p: Pt): Pt = turns.foldRight(p) { t, q -> rotate(q.x, q.y, t.cx, t.cy, t.degrees) }
    fun toOwn(p: Pt): Pt = turns.fold(p) { q, t -> rotate(q.x, q.y, t.cx, t.cy, -t.degrees) }
    // Each attached end aims at the other end's centre (or its free point).
    val aimA = to?.frame?.centre ?: toPage(b)
    val aimB = from?.frame?.centre ?: toPage(a)
    if (from != null) a = toOwn(edgePoint(from.node, from.rect, from.frame, aimA))
    if (to != null) b = toOwn(edgePoint(to.node, to.rect, to.frame, aimB))
    val horizontal = abs(b.x - a.x) >= abs(b.y - a.y)
    return when (route) {
        "elbow" -> {
            val through = if (from == null || to == null) null else level(from, to, a, b, horizontal, turned, ::toPage, ::toOwn)
            val pts = through ?: when {
                abs(if (horizontal) b.y - a.y else b.x - a.x) < 1.0 -> listOf(a, b)
                horizontal -> {
                    val mid = (a.x + b.x) / 2.0
                    listOf(a, Pt(mid, a.y), Pt(mid, b.y), b)
                }
                else -> {
                    val mid = (a.y + b.y) / 2.0
                    listOf(a, Pt(a.x, mid), Pt(b.x, mid), b)
                }
            }
            LinePoints(pts, false)
        }
        "curve" -> {
            val c1: Pt
            val c2: Pt
            if (horizontal) {
                val k = (b.x - a.x) / 2.0
                c1 = Pt(a.x + k, a.y)
                c2 = Pt(b.x - k, b.y)
            } else {
                val k = (b.y - a.y) / 2.0
                c1 = Pt(a.x, a.y + k)
                c2 = Pt(b.x, b.y - k)
            }
            LinePoints(listOf(a, c1, c2, b), true)
        }
        else -> LinePoints(listOf(a, b), false)
    }
}

/**
 * An elbow between two attached nodes beside each other whose outlines as drawn overlap across
 * the run: straight through the middle of the overlap, each end where that run last leaves its
 * node on the side facing the other; null when they do not overlap or the run misses an outline.
 */
private fun level(from: End, to: End, a: Pt, b: Pt, horizontal: Boolean, turned: Double, toPage: (Pt) -> Pt, toOwn: (Pt) -> Pt): List<Pt>? {
    // The span each node's outline as drawn covers across the run, in the line's coordinates:
    // the node is turned by its turn less the line's there.
    val ax = if (horizontal) 0.0 else 1.0
    val ay = if (horizontal) 1.0 else 0.0
    fun span(e: End): Pair<Double, Double> {
        val c = toOwn(e.frame.centre)
        val mid = c.x * ax + c.y * ay
        val u = rotate(ax, ay, 0.0, 0.0, turned - e.frame.turn)
        return mid - reachAlong(e.node, e.rect, Pt(-u.x, -u.y)) to mid + reachAlong(e.node, e.rect, u)
    }
    val (f0, f1) = span(from)
    val (t0, t1) = span(to)
    val lo = max(f0, t0)
    val hi = min(f1, t1)
    if (lo > hi) return null
    val m = (lo + hi) / 2.0
    val alongX = if (horizontal) signum(b.x - a.x) else 0.0
    val alongY = if (horizontal) 0.0 else signum(b.y - a.y)
    fun meet(e: End, way: Double): Pt? {
        val c = toOwn(e.frame.centre)
        val back = way * hypot(e.rect.w, e.rect.h)
        val start = if (horizontal) Pt(c.x - alongX * back, m) else Pt(m, c.y - alongY * back)
        val dir = rotate(way * alongX, way * alongY, 0.0, 0.0, turned)
        return exitPoint(e.node, e.rect, e.frame, toPage(start), dir)?.let(toOwn)
    }
    return listOf(meet(from, 1.0) ?: return null, meet(to, -1.0) ?: return null)
}

/** Where a connector's end meets node [n]: the ray from its drawn centre towards [aim] leaves its outline there, or its centre when the ray has no direction. */
private fun edgePoint(n: DesignNode, r: Rect, frame: PageFrame, aim: Pt): Pt {
    val centre = frame.centre
    return exitPoint(n, r, frame, centre, Pt(aim.x - centre.x, aim.y - centre.y)) ?: centre
}

/**
 * Everything [id] paints, in its parent's coordinates (page coordinates under unturned
 * parents): what it draws before its turn (its box; a line's or path's drawn points with its
 * stroke's reach and its arrowheads; a label's, text's or sticky's words where they overflow the
 * box), turned about its box's centre, then its outer shadows and a ring segment's capped ends; a
 * group's, page's or unclipped frame's children, turned with it, less those hidden or wholly
 * clear; and its blur over all of it. Null for what is not placed. For a page, everything on it:
 * the box the phone fits its canvas to (bounds.rs `painted_bounds`).
 */
fun paintedBounds(scene: DesignScene, placed: Placed, measure: Measure, id: String): Rect? {
    val n = scene.node(id) ?: return null
    val r = placed.rect(id) ?: return null
    val turn = rotation(n)
    val own: Rect? = when (n.kind) {
        "page", "group" -> null
        "line", "path" -> {
            val drawn = (drawnBox(scene, placed, n) ?: return null).inflate(strokeReach(n))
            arrowheads(scene, placed, n, r).mapNotNull { around(it.points) }.fold(drawn) { b, head -> b.union(head) }
        }
        "rect", "ellipse", "polygon" -> {
            val label = if (n.text.isNullOrEmpty()) null else labelBox(n, r) { measure.measure(n, it).h }
            if (label == null) r else r.union(words(label.x, label.y, label.w, measure.measure(n, label.w)))
        }
        "text" -> {
            val wrap = wrapWidth(n, r)
            val size = measure.measure(n, wrap)
            r.union(words(r.x, r.y, wrap ?: size.w, size))
        }
        "sticky" -> if (n.text.isNullOrEmpty()) r else {
            val wrap = wrapWidth(n, r) ?: 1.0
            r.union(words(r.x + STICKY_PAD, r.y + STICKY_PAD, wrap, measure.measure(n, wrap)))
        }
        // An icon's stroke is drawn in its 24-unit grid, scaled to the box's shorter side.
        "icon" -> r.inflate(max(n.strokeWidth ?: 2.0, 0.0) / 2.0 * min(r.w, r.h) / 24.0)
        else -> r
    }
    val shown = own?.let { rotatedBoundsAbout(it, turn, r.cx, r.cy).inflate(shadowReach(n) + capReach(n, r)) }
    // What overflows a group or an unclipped frame shows, so it is pictured too.
    val clips = n.kind == "frame" && n.clip != false
    val holds = n.kind == "page" || n.kind == "frame" || n.kind == "group"
    val kids = if (holds && !clips) {
        scene.children(id)
            .filter { c -> scene.node(c)?.let { k -> !k.isHidden && k.opacity?.let { it <= 0.0 } != true } == true }
            .mapNotNull { paintedBounds(scene, placed, measure, it) }
            .reduceOrNull { a, b -> a.union(b) }
            ?.let { rotatedBoundsAbout(it, turn, r.cx, r.cy) }
    } else {
        null
    }
    val all = when {
        shown != null && kids != null -> shown.union(kids)
        else -> shown ?: kids ?: (if (n.kind == "group") r else null) ?: return null
    }
    return all.inflate(max(n.blur ?: 0.0, 0.0) * 2.0)
}

/** Words laid out [w] wide from ([x], [y]) and measured [size]: a word too long to wrap passes the sides by what it is over. */
private fun words(x: Double, y: Double, w: Double, size: TextSize): Rect {
    val over = max(size.w - w, 0.0)
    return Rect(x - over, y, w + 2.0 * over, size.h)
}

/** How far a line's or path's stroke reaches past its points: half its weight, a square cap's corner √2 times that, a mitred corner up to four times. */
private fun strokeReach(n: DesignNode): Double {
    val half = lineWidth(n) / 2.0
    return when {
        n.strokeJoin == "miter" -> half * 4.0
        n.strokeCap == "square" -> half * sqrt(2.0)
        else -> half
    }
}
