package com.forge.app.ui.common

/**
 * The most points a sparkline plots (P-13).
 *
 * A phone-width chart has on the order of a thousand pixels across it, and this reduction keeps two
 * points per bucket, so 512 is roughly four pixels per bucket — finer than the 2dp stroke drawn over
 * it. Above that the extra points cost path work nobody can see.
 */
const val SPARKLINE_MAX_POINTS = 512

/**
 * [values] reduced to at most [maxPoints] plotted points, preserving each bucket's extremes (P-13).
 *
 * The lifetime-volume curve is one point per finished session with no cap. During its 900 ms reveal
 * — about 54 frames — every frame walked all N points twice to allocate and fill both the line and
 * the area path: roughly 216N point visits and 108 path allocations for one reveal, which at 5,000
 * sessions is over a million visits to draw a chart that cannot show 5,000 independent x positions.
 *
 * Each bucket contributes its lowest and highest value, emitted in the order they occur so a spike
 * still reads as a spike rather than being averaged away — a mean would flatten exactly the
 * moments a lifetime curve exists to show. The first and last values are kept verbatim: the curve
 * has to start and end where the data does, and the end dot sits on the figure printed beside it.
 *
 * A series already within the cap is returned untouched, so the common case allocates nothing.
 */
fun sparklineSeries(values: List<Double>, maxPoints: Int = SPARKLINE_MAX_POINTS): List<Double> {
    if (maxPoints < 4 || values.size <= maxPoints) return values
    val buckets = maxPoints / 2
    val out = ArrayList<Double>(buckets * 2)
    for (b in 0 until buckets) {
        val from = (b.toLong() * values.size / buckets).toInt()
        val to = (((b + 1).toLong() * values.size / buckets).toInt()).coerceIn(from + 1, values.size)
        var lo = values[from]
        var hi = values[from]
        var loAt = from
        var hiAt = from
        for (i in from until to) {
            val v = values[i]
            if (v < lo) { lo = v; loAt = i }
            if (v > hi) { hi = v; hiAt = i }
        }
        // In the order they happened, so the shape of the bucket survives the reduction.
        if (loAt <= hiAt) { out += lo; out += hi } else { out += hi; out += lo }
    }
    out[0] = values.first()
    out[out.lastIndex] = values.last()
    return out
}
