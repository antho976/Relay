package com.forge.app.domain.cardio

import com.forge.app.domain.units.toDisplayDistance
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Average pace in whole seconds per display unit (km or mi), or null when distance or duration is
 * missing. Rounds ONCE, straight off the raw duration + distance — the single source every pace
 * reading rounds through (the formatted chip and the session-compare "best/prev" reads), so two
 * views of the same session can't disagree by a second.
 */
fun paceSecPerUnit(durationMin: Int, distanceKm: Double?, useMiles: Boolean): Int? {
    if (distanceKm == null || distanceKm <= 0.0 || durationMin <= 0) return null
    return (durationMin * 60.0 / toDisplayDistance(distanceKm, useMiles)).roundToInt()
}

/** "M:SS" for a pace reading or gap in seconds. Locale.US — a stopwatch reading never localises. */
fun formatPaceSec(sec: Int): String = String.format(Locale.US, "%d:%02d", sec / 60, sec % 60)

/**
 * Average pace as "M:SS" per display unit (km or mi) for a session, or null when distance or duration
 * is missing. Callers append the unit suffix via [distanceUnitLabel].
 */
fun pacePerUnit(durationMin: Int, distanceKm: Double?, useMiles: Boolean): String? =
    paceSecPerUnit(durationMin, distanceKm, useMiles)?.let(::formatPaceSec)

/** Pace per kilometre — the [pacePerUnit] specialisation kept for the not-yet-converted callers. */
fun pacePerKm(durationMin: Int, distanceKm: Double?): String? =
    pacePerUnit(durationMin, distanceKm, useMiles = false)
