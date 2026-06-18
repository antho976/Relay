package com.forge.app.domain.cardio

import java.util.Locale
import kotlin.math.roundToInt

/**
 * Average pace as "M:SS" per km for a session, or null when distance or duration is missing.
 * Locale.US so the ':' separator and digits never localise (the value is a stopwatch reading,
 * not a formatted number).
 */
fun pacePerKm(durationMin: Int, distanceKm: Double?): String? {
    if (distanceKm == null || distanceKm <= 0.0 || durationMin <= 0) return null
    val secondsPerKm = (durationMin * 60.0 / distanceKm).roundToInt()
    return String.format(Locale.US, "%d:%02d", secondsPerKm / 60, secondsPerKm % 60)
}
