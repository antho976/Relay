package com.forge.app.domain.cardio

import com.forge.app.domain.units.toDisplayDistance
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Average pace as "M:SS" per display unit (km or mi) for a session, or null when distance or duration
 * is missing. Locale.US so the ':' separator and digits never localise (the value is a stopwatch
 * reading, not a formatted number). Callers append the unit suffix via [distanceUnitLabel].
 */
fun pacePerUnit(durationMin: Int, distanceKm: Double?, useMiles: Boolean): String? {
    if (distanceKm == null || distanceKm <= 0.0 || durationMin <= 0) return null
    val secondsPerUnit = (durationMin * 60.0 / toDisplayDistance(distanceKm, useMiles)).roundToInt()
    return String.format(Locale.US, "%d:%02d", secondsPerUnit / 60, secondsPerUnit % 60)
}

/** Pace per kilometre — the [pacePerUnit] specialisation kept for the not-yet-converted callers. */
fun pacePerKm(durationMin: Int, distanceKm: Double?): String? =
    pacePerUnit(durationMin, distanceKm, useMiles = false)
