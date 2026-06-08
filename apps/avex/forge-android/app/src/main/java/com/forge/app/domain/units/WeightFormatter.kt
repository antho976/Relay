package com.forge.app.domain.units

private const val KG_PER_LB = 0.45359237

/** Converts a stored lb value to the display unit and formats it WITH a unit suffix (e.g. "20 kg"). */
fun formatWeight(lb: Double, useKg: Boolean): String {
    if (useKg) {
        val kg = lb * KG_PER_LB
        return if (kg % 1.0 == 0.0) "${kg.toInt()} kg" else "%.1f kg".format(kg)
    }
    return if (lb % 1.0 == 0.0) "${lb.toInt()} lb" else "%.1f lb".format(lb)
}

/**
 * The numeric value in the display unit with NO unit suffix — for seeding editable weight fields.
 * The log/edit input fields hold a bare number in the display unit; conversion to lb happens on
 * submit via [toStoredWeightText]. (Pairing this with [toStoredWeightText] is what lets the field
 * round-trip — tapping a kg suggestion no longer logs a set with no weight.)
 */
fun weightInputValue(lb: Double, useKg: Boolean): String {
    val v = if (useKg) lb * KG_PER_LB else lb
    return if (v % 1.0 == 0.0) "${v.toInt()}" else "%.1f".format(v)
}

/** A weight *difference* (in lb) formatted in the display unit with a unit label, e.g. "2.5 kg". */
fun formatWeightDelta(lbDiff: Double, useKg: Boolean): String {
    val v = if (useKg) lbDiff * KG_PER_LB else lbDiff
    val num = if (v % 1.0 == 0.0) "${v.toInt()}" else "%.1f".format(v)
    return "$num ${unitLabel(useKg)}"
}

/**
 * Converts a user-entered string in the display unit back to lb for storage. Tolerates a trailing
 * unit suffix ("20 kg", "20kg", "20 lb") so it round-trips [formatWeight]/[weightInputValue] output.
 */
fun parseToLb(input: String, useKg: Boolean): Double? {
    val cleaned = input.trim().lowercase().removeSuffix("kg").removeSuffix("lb").trim()
    val numeric = cleaned.toDoubleOrNull() ?: return null
    return if (useKg) numeric / KG_PER_LB else numeric
}

/**
 * Canonical stored weight text (always lb) for what the user typed in the display unit. A numeric
 * kg entry is converted to its lb value; anything non-numeric ("BW", "2 plates") passes through
 * unchanged for [com.forge.app.domain.parser.WeightParser] to interpret. Used by BOTH the log and
 * edit paths so unit handling can never diverge between them.
 */
fun toStoredWeightText(input: String, useKg: Boolean): String {
    val trimmed = input.trim()
    if (!useKg) return trimmed
    val lb = parseToLb(trimmed, useKg = true) ?: return trimmed
    return if (lb % 1.0 == 0.0) "${lb.toInt()}" else "%.1f".format(lb)
}

fun unitLabel(useKg: Boolean): String = if (useKg) "kg" else "lb"
