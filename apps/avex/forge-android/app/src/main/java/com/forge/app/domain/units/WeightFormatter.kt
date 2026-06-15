package com.forge.app.domain.units

import java.util.Locale

private const val KG_PER_LB = 0.45359237

// Number formatting here is pinned to Locale.US so the decimal separator is always '.', never a
// locale-dependent ',' — a comma would both read wrong ("1,2k") and break parseToLb's round-trip.

/** Converts a stored lb value to the display unit and formats it WITH a unit suffix (e.g. "20 kg"). */
fun formatWeight(lb: Double, useKg: Boolean): String {
    if (useKg) {
        val kg = lb * KG_PER_LB
        return if (kg % 1.0 == 0.0) "${kg.toInt()} kg" else String.format(Locale.US, "%.1f kg", kg)
    }
    return if (lb % 1.0 == 0.0) "${lb.toInt()} lb" else String.format(Locale.US, "%.1f lb", lb)
}

/**
 * Session volume (stored lb) converted to the display unit and abbreviated past 1k — "850 lb" /
 * "1.2k kg". Mirrors [formatWeight]'s unit handling; used by the session-detail surfaces, which
 * (unlike the lb-only overview/history rows) honour the kg setting alongside their per-set weights.
 */
fun formatVolume(volumeLb: Double, useKg: Boolean): String {
    val v = toDisplayWeight(volumeLb, useKg)
    val unit = unitLabel(useKg)
    return if (v >= 1000) String.format(Locale.US, "%.1fk %s", v / 1000, unit) else "${v.toInt()} $unit"
}

/**
 * Compact volume with trailing zeros trimmed — "1.2k lb" / "950 lb", or unit-less when [withUnit] is
 * false ("412k" / "950"). The single source for the Coach Brief / Profile ledger / Recap compact
 * volume labels, which each used to keep their own (locale-unsafe) copy of this.
 */
fun formatVolumeCompact(volumeLb: Double, useKg: Boolean, withUnit: Boolean = true): String {
    val v = toDisplayWeight(volumeLb, useKg)
    val suffix = if (withUnit) " ${unitLabel(useKg)}" else ""
    return if (v >= 1000)
        "${String.format(Locale.US, "%.1f", v / 1000).trimEnd('0').trimEnd('.')}k$suffix"
    else "${v.toInt()}$suffix"
}

/** The numeric value in the display unit, UNformatted — for driving animated counters / raw math. */
fun toDisplayWeight(lb: Double, useKg: Boolean): Double = if (useKg) lb * KG_PER_LB else lb

/**
 * The numeric value in the display unit with NO unit suffix — for seeding editable weight fields.
 * The log/edit input fields hold a bare number in the display unit; conversion to lb happens on
 * submit via [toStoredWeightText]. (Pairing this with [toStoredWeightText] is what lets the field
 * round-trip — tapping a kg suggestion no longer logs a set with no weight.)
 */
fun weightInputValue(lb: Double, useKg: Boolean): String {
    val v = if (useKg) lb * KG_PER_LB else lb
    return if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
}

/** A weight *difference* (in lb) formatted in the display unit with a unit label, e.g. "2.5 kg". */
fun formatWeightDelta(lbDiff: Double, useKg: Boolean): String {
    val v = if (useKg) lbDiff * KG_PER_LB else lbDiff
    val num = if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)
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
    return if (lb % 1.0 == 0.0) "${lb.toInt()}" else String.format(Locale.US, "%.1f", lb)
}

fun unitLabel(useKg: Boolean): String = if (useKg) "kg" else "lb"
