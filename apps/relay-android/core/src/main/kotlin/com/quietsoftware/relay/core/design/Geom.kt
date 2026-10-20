package com.quietsoftware.relay.core.design

import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/** A point in page pixels. */
data class Pt(val x: Double, val y: Double)

/**
 * A box in page pixels (crates/relay-client/src/design/geom.rs `Rect`), a port line for line so
 * layout and paint read the same numbers on both sides.
 */
data class Rect(val x: Double, val y: Double, val w: Double, val h: Double) {
    val right: Double get() = x + w
    val bottom: Double get() = y + h
    val cx: Double get() = x + w / 2
    val cy: Double get() = y + h / 2

    fun contains(px: Double, py: Double): Boolean = px >= x && px <= right && py >= y && py <= bottom

    fun intersects(o: Rect): Boolean = x <= o.right && o.x <= right && y <= o.bottom && o.y <= bottom

    fun union(o: Rect): Rect {
        val ux = min(x, o.x)
        val uy = min(y, o.y)
        return Rect(ux, uy, max(right, o.right) - ux, max(bottom, o.bottom) - uy)
    }

    fun inflate(by: Double): Rect = Rect(x - by, y - by, w + 2 * by, h + 2 * by)

    fun translate(dx: Double, dy: Double): Rect = Rect(x + dx, y + dy, w, h)

    companion object {
        val ZERO = Rect(0.0, 0.0, 0.0, 0.0)

        /** The box two corners span, whichever way round they are. */
        fun spanning(a: Pt, b: Pt): Rect = Rect(min(a.x, b.x), min(a.y, b.y), kotlin.math.abs(a.x - b.x), kotlin.math.abs(a.y - b.y))

        /** The union of [rects]; null for none. */
        fun bounds(rects: Iterable<Rect>): Rect? = rects.fold(null as Rect?) { acc, r -> acc?.union(r) ?: r }
    }
}

/** ([x], [y]) turned [degrees] clockwise about ([cx], [cy]) (shapes.rs `rotate`). */
fun rotate(x: Double, y: Double, cx: Double, cy: Double, degrees: Double): Pt {
    val a = Math.toRadians(degrees)
    val s = sin(a)
    val c = cos(a)
    val dx = x - cx
    val dy = y - cy
    return Pt(cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/** The box [r] covers turned [degrees] about its centre (shapes.rs `rotated_bounds`). */
fun rotatedBounds(r: Rect, degrees: Double): Rect {
    if (degrees == 0.0) return r
    val corners = listOf(Pt(r.x, r.y), Pt(r.right, r.y), Pt(r.right, r.bottom), Pt(r.x, r.bottom)).map { rotate(it.x, it.y, r.cx, r.cy, degrees) }
    val x0 = corners.minOf { it.x }
    val x1 = corners.maxOf { it.x }
    val y0 = corners.minOf { it.y }
    val y1 = corners.maxOf { it.y }
    return Rect(x0, y0, x1 - x0, y1 - y0)
}
