package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry

/**
 * One paced session in a type's pace history (GYMAP-35). The raw duration + distance are carried (not
 * a pre-rounded pace) so the chart formats each point through the single [paceSecPerUnit] rounding
 * path in the viewer's unit — the trend can't disagree with a session's own pace read.
 */
data class CardioPacePoint(val dateMs: Long, val durationMin: Int, val distanceKm: Double)

/** One activity type's pace-over-time series (GYMAP-35), points oldest→newest, at least two of them. */
data class CardioPaceSeries(val typeCode: String, val points: List<CardioPacePoint>)

/**
 * Per-activity pace series for the pace-trend chart (GYMAP-35). A type qualifies once it has at least
 * two sessions carrying a real distance and duration — a single point is no trend — so rest days and
 * distance-less sessions never appear. Series order is most-sessions first (stable tie-break by type
 * code); each series' points run oldest→newest.
 */
fun cardioPaceSeries(entries: List<CardioEntry>): List<CardioPaceSeries> =
    entries
        .filter { it.type != CardioType.REST.code && (it.distanceKm ?: 0.0) > 0.0 && it.durationMin > 0 }
        .groupBy { it.type }
        .mapNotNull { (code, group) ->
            val points = group
                .sortedBy { it.date }
                .map { CardioPacePoint(it.date, it.durationMin, it.distanceKm ?: 0.0) }
            if (points.size >= 2) CardioPaceSeries(code, points) else null
        }
        .sortedWith(compareByDescending<CardioPaceSeries> { it.points.size }.thenBy { it.typeCode })
