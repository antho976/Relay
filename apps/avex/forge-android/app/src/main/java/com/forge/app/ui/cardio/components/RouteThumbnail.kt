package com.forge.app.ui.cardio.components

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.forge.app.domain.cardio.RoutePoint

/**
 * An offline, shape-only render of a recorded GPS track — the path's outline drawn as a
 * polyline on a plain canvas. Avex has no internet, so there is deliberately **no basemap
 * / map tiles**: this is the route's *shape*, not a map (think a Strava thumbnail with the
 * streets removed). Points are normalised into the canvas box; renders nothing for an
 * empty / single-point route, so a wearable-less entry simply shows no thumbnail.
 *
 * Today nothing populates [route] — the Health Connect `ExerciseSessionRecord` route read is
 * a deferred follow-up — so this stays dormant until that data is wired in.
 */
@Composable
internal fun RouteThumbnail(
    route: List<RoutePoint>,
    color: Color,
    modifier: Modifier = Modifier
) {
    if (route.size < 2) return
    Canvas(modifier) {
        val lats = route.map { it.lat }
        val lngs = route.map { it.lng }
        val minLat = lats.min(); val maxLat = lats.max()
        val minLng = lngs.min(); val maxLng = lngs.max()
        // Guard a degenerate (straight-line) track so we never divide by ~0.
        val latRange = (maxLat - minLat).coerceAtLeast(1e-9)
        val lngRange = (maxLng - minLng).coerceAtLeast(1e-9)

        val pad = 3.dp.toPx()
        val w = (size.width - pad * 2).coerceAtLeast(0f)
        val h = (size.height - pad * 2).coerceAtLeast(0f)

        val path = Path()
        route.forEachIndexed { i, p ->
            val x = pad + ((p.lng - minLng) / lngRange * w).toFloat()
            // Latitude grows northward → invert Y so north renders at the top.
            val y = pad + ((maxLat - p.lat) / latRange * h).toFloat()
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(
            path = path,
            color = color,
            style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
        )
    }
}
