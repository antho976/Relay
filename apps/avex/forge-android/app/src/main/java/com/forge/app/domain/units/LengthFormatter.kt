package com.forge.app.domain.units

import java.util.Locale

private const val CM_PER_INCH = 2.54

// Length (body measurements) mirrors WeightFormatter: values are stored canonically in cm and
// converted at the display edge, gated on the user's `use_cm` preference (independent of the kg/lb
// weight toggle). Number formatting is pinned to Locale.US so the decimal separator is always '.',
// never a locale ',' — a comma would read wrong and break parseToCm's round-trip.

/** The value in the display unit, UNformatted — for driving figures / raw math. */
fun toDisplayLength(cm: Double, useCm: Boolean): Double = if (useCm) cm else cm / CM_PER_INCH

/** Inverse of [toDisplayLength]: a value the user picked in their display unit, back to stored cm. */
fun fromDisplayLength(value: Double, useCm: Boolean): Double = if (useCm) value else value * CM_PER_INCH

/** "cm" / "in" — the display unit label (uppercased at the call site for mono captions). */
fun lengthUnitLabel(useCm: Boolean): String = if (useCm) "cm" else "in"

/** A stored cm value converted to the display unit and formatted WITH a unit suffix ("81 cm" / "32.5 in"). */
fun formatLength(cm: Double, useCm: Boolean): String {
    val v = toDisplayLength(cm, useCm)
    val num = if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
    return "$num ${lengthUnitLabel(useCm)}"
}

/**
 * The value in the display unit with NO unit suffix — for seeding an editable measurement field.
 * Pairs with [parseToCm] so tapping into the log sheet round-trips the stored value exactly.
 */
fun lengthInputValue(cm: Double, useCm: Boolean): String {
    val v = toDisplayLength(cm, useCm)
    return if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
}

/** A length *difference* (in cm) formatted in the display unit with a unit label, e.g. "1.5 in". */
fun formatLengthDelta(cmDiff: Double, useCm: Boolean): String {
    val v = toDisplayLength(cmDiff, useCm)
    val num = if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
    return "$num ${lengthUnitLabel(useCm)}"
}

/**
 * Converts a user-entered string in the display unit back to cm for storage. Tolerates a trailing
 * unit suffix ("81 cm", "81cm", "32 in", "32\"") so it round-trips [formatLength]/[lengthInputValue].
 */
fun parseToCm(input: String, useCm: Boolean): Double? {
    val cleaned = input.trim().lowercase()
        .removeSuffix("cm").removeSuffix("in").removeSuffix("\"").trim()
    val numeric = cleaned.toDoubleOrNull() ?: return null
    return if (useCm) numeric else numeric * CM_PER_INCH
}
