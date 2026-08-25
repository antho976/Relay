package com.forge.app.domain.units

import java.util.Locale

private const val KM_PER_MILE = 1.609344

// Distance is stored canonically in km everywhere (cardio_entry.distance_km); these convert at the
// display/input boundary only — the mirror of WeightFormatter for the lb↔stored-weight split.
// Number formatting is pinned to Locale.US so the decimal separator is always '.', never a
// locale-dependent ',' — a comma would both read wrong and break parseToKm's round-trip with the
// cardio log field (which only accepts '.').

/** "mi" or "km". */
fun distanceUnitLabel(useMiles: Boolean): String = if (useMiles) "mi" else "km"

/** A stored km value as the display unit's number, UNformatted — for driving counters / raw math. */
fun toDisplayDistance(km: Double, useMiles: Boolean): Double = if (useMiles) km / KM_PER_MILE else km

/** Inverse of [toDisplayDistance]: a value the user entered in their display unit, back to stored km. */
fun fromDisplayDistance(value: Double, useMiles: Boolean): Double = if (useMiles) value * KM_PER_MILE else value

/** A stored km value formatted in the display unit WITH a unit suffix, one decimal — "5.0 km" / "3.1 mi". */
fun formatDistance(km: Double, useMiles: Boolean): String =
    String.format(Locale.US, "%.1f %s", toDisplayDistance(km, useMiles), distanceUnitLabel(useMiles))

/**
 * The display-unit value with NO suffix — for seeding an editable distance field. Whole values lose
 * the trailing ".0" so the field reads "5" not "5.0". Pairs with [parseToKm] so the edit field
 * round-trips: a 5 km entry shown as "3.1" (mi) parses back to ~5 km on submit.
 */
fun distanceInputValue(km: Double, useMiles: Boolean): String {
    val v = toDisplayDistance(km, useMiles)
    return if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
}

/**
 * Converts a user-entered string in the display unit back to stored km, or null if blank/unparseable.
 * Tolerates a trailing unit suffix ("5 mi", "5mi", "8 km") so it round-trips [formatDistance].
 */
fun parseToKm(input: String, useMiles: Boolean): Double? {
    val cleaned = input.trim().lowercase()
    // A suffix the user typed states the unit they MEAN, and it wins over the setting. Stripping
    // BOTH suffixes and then applying the setting's conversion meant "5 km" typed while the app was
    // in miles stored 8.05 km — a 61 % overstatement carried into pace and the calorie estimate.
    DISTANCE_UNIT_REGEX.matchEntire(cleaned)?.let { m ->
        val v = m.groupValues[1].toDoubleOrNull() ?: return null
        return if (m.groupValues[2] == "mi") v * KM_PER_MILE else v
    }
    val numeric = cleaned.toDoubleOrNull() ?: return null
    return fromDisplayDistance(numeric, useMiles)
}

private val DISTANCE_UNIT_REGEX = Regex("""^\s*(\d+(?:\.\d+)?)\s*(mi|km)\s*$""")
