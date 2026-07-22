package com.forge.app.domain.cardio

import java.util.Locale

/**
 * The optional, per-activity metrics (GYMAP-38): a grade [INCLINE] for belt machines, pool [LAPS]
 * for a swim, and [ELEVATION] gain for outdoor distance work. An activity surfaces only the fields
 * that make sense for it — the same idea as the interval count showing for HIIT alone — so the log
 * form stays short and every field it does show is relevant.
 */
enum class CardioField { INCLINE, LAPS, ELEVATION }

/**
 * The optional fields a built-in [CardioType] surfaces. Rest and the steady types that fit none
 * (Row, HIIT, Yoga, Other) map to an empty set, so they show no per-type field at all.
 */
fun optionalFieldsFor(type: CardioType): Set<CardioField> = when (type) {
    // Belt / stepper machines log a grade, not real-world elevation.
    CardioType.TREADMILL, CardioType.ELLIPTICAL -> setOf(CardioField.INCLINE)
    // Pool work counts lengths.
    CardioType.SWIM -> setOf(CardioField.LAPS)
    // Outdoor distance work climbs.
    CardioType.RUN, CardioType.WALK, CardioType.HIKE, CardioType.CYCLE -> setOf(CardioField.ELEVATION)
    else -> emptySet()
}

/** A stored incline percent as a compact label — "6%" / "6.5%", no trailing ".0". */
fun formatInclinePct(pct: Double): String {
    val s = if (pct % 1.0 == 0.0) pct.toInt().toString() else String.format(Locale.US, "%.1f", pct)
    return "$s%"
}
