package com.forge.app.domain.units

import kotlin.math.roundToInt

private const val FEET_PER_METER = 3.28084

// Elevation gain is stored canonically in metres (cardio_entry.elevation_m) and converted at the
// display/input edge only — the mirror of DistanceFormatter. It rides the DISTANCE unit toggle (a
// miles user thinks in feet, a km user in metres) so it needs no preference of its own. Elevation
// reads as whole units — nobody logs a fractional metre of gain — so there is no locale-decimal
// concern here.

/** "ft" or "m". */
fun elevationUnitLabel(useMiles: Boolean): String = if (useMiles) "ft" else "m"

/** A stored metre value as the display unit's number, UNrounded — for raw math. */
fun toDisplayElevation(meters: Double, useMiles: Boolean): Double =
    if (useMiles) meters * FEET_PER_METER else meters

/** A stored metre value formatted in the display unit WITH a suffix, whole units — "120 m" / "394 ft". */
fun formatElevation(meters: Double, useMiles: Boolean): String =
    "${toDisplayElevation(meters, useMiles).roundToInt()} ${elevationUnitLabel(useMiles)}"

/** The display-unit value with NO suffix, whole units — for seeding the editable elevation field. */
fun elevationInputValue(meters: Double, useMiles: Boolean): String =
    toDisplayElevation(meters, useMiles).roundToInt().toString()

/**
 * Converts a user-entered elevation string in the display unit back to stored metres, or null if
 * blank/unparseable. Tolerates a trailing unit suffix ("120 m", "394ft") so it round-trips
 * [formatElevation] / [elevationInputValue].
 */
fun parseToMeters(input: String, useMiles: Boolean): Double? {
    val cleaned = input.trim().lowercase().removeSuffix("ft").removeSuffix("m").trim()
    val numeric = cleaned.toDoubleOrNull() ?: return null
    return if (useMiles) numeric / FEET_PER_METER else numeric
}
