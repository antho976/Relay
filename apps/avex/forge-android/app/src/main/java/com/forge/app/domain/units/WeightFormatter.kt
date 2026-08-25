package com.forge.app.domain.units

import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

private const val KG_PER_LB = 0.45359237
private const val LB_PER_STONE = 14.0

// Number formatting here is pinned to Locale.US so the decimal separator is always '.', never a
// locale-dependent ',' — a comma would both read wrong ("1,2k") and break parseToLb's round-trip.

/**
 * The weight unit the app displays in (GYMAP-72). Weights are ALWAYS stored in pounds; a unit only
 * changes how a stored lb value renders and how a typed value is read back. Stones (British) render
 * as a `st + lb` compound ("12 st 4 lb"); kg and lb render as one decimal number ("45.4 kg").
 * Aggregates (volume) stay decimal in every unit ("1.2k st") — a stone+lb total would be nonsense.
 */
enum class WeightUnit(val label: String) {
    LB("lb"), KG("kg"), ST("st");

    /** Metric = kilograms. Drives the distance/length default (imperial lb/st → miles/inches). */
    val isMetric: Boolean get() = this == KG

    companion object {
        /** Parse a persisted key (the [label]); anything unknown/null falls back to [LB]. */
        fun fromKey(key: String?): WeightUnit = entries.firstOrNull { it.label == key } ?: LB
        /** Bridge from the legacy boolean unit flag (true = kg, false = lb). */
        fun ofKg(useKg: Boolean): WeightUnit = if (useKg) KG else LB
    }
}

private fun trimDecimal(v: Double): String =
    if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)

/** A non-negative lb value as a stone+lb compound: "12 st 4 lb" / "12 st" / "8 lb" (rounded to lb). */
private fun formatStoneLb(lb: Double): String {
    val totalLb = lb.roundToInt()
    val st = totalLb / 14
    val rem = totalLb % 14
    return when {
        st > 0 && rem > 0 -> "$st st $rem lb"
        st > 0 -> "$st st"
        else -> "$rem lb"   // 0 st (incl. a bare 0) → "N lb"
    }
}

/** Converts a stored lb value to the display unit and formats it WITH a unit suffix (e.g. "20 kg"). */
fun formatWeight(lb: Double, unit: WeightUnit): String = when (unit) {
    WeightUnit.KG -> {
        val kg = lb * KG_PER_LB
        if (kg % 1.0 == 0.0) "${kg.toInt()} kg" else String.format(Locale.US, "%.1f kg", kg)
    }
    WeightUnit.ST -> formatStoneLb(lb)
    WeightUnit.LB -> if (lb % 1.0 == 0.0) "${lb.toInt()} lb" else String.format(Locale.US, "%.1f lb", lb)
}

/**
 * Session volume (stored lb) converted to the display unit and abbreviated past 1k — "850 lb" /
 * "1.2k kg" / "1.2k st". Volume stays a single decimal figure in every unit (a stone+lb aggregate
 * would be nonsense). Used by the session-detail surfaces, which honour the unit alongside their
 * per-set weights.
 */
fun formatVolume(volumeLb: Double, unit: WeightUnit): String {
    val v = toDisplayWeight(volumeLb, unit)
    val u = unit.label
    // roundToInt, not toInt: truncation runs AFTER the kg/st conversion, so it bit hardest in the
    // unit that needs it least — a 500 lb session is 226.796 kg and used to read "226 kg".
    return if (v >= 1000) String.format(Locale.US, "%.1fk %s", v / 1000, u) else "${v.roundToInt()} $u"
}

/**
 * Compact volume with trailing zeros trimmed — "1.2k lb" / "950 lb", or unit-less when [withUnit] is
 * false ("412k" / "950"). The single source for the Coach Brief / Profile ledger / Recap compact
 * volume labels. Decimal in every unit, like [formatVolume].
 */
fun formatVolumeCompact(volumeLb: Double, unit: WeightUnit, withUnit: Boolean = true): String {
    val v = toDisplayWeight(volumeLb, unit)
    val suffix = if (withUnit) " ${unit.label}" else ""
    return if (v >= 1000)
        "${String.format(Locale.US, "%.1f", v / 1000).trimEnd('0').trimEnd('.')}k$suffix"
    else "${v.roundToInt()}$suffix"
}

/** The numeric value in the display unit, UNformatted — for driving animated counters / raw math.
 *  Stones is a single decimal (lb/14) here — the stone+lb split is only a display concern. */
fun toDisplayWeight(lb: Double, unit: WeightUnit): Double = when (unit) {
    WeightUnit.KG -> lb * KG_PER_LB
    WeightUnit.ST -> lb / LB_PER_STONE
    WeightUnit.LB -> lb
}

/** Inverse of [toDisplayWeight]: a value the user picked in their display unit, back to stored lb. */
fun fromDisplayWeight(value: Double, unit: WeightUnit): Double = when (unit) {
    WeightUnit.KG -> value / KG_PER_LB
    WeightUnit.ST -> value * LB_PER_STONE
    WeightUnit.LB -> value
}

/**
 * The numeric value in the display unit with NO unit suffix — for seeding editable weight fields.
 * The single-field log/edit inputs hold a bare number in the display unit; conversion to lb happens
 * on submit via [toStoredWeightText]. Stones seeds a single decimal ("9.6") — the compound st+lb
 * entry is only used by the bodyweight sheet's two-field input, which computes lb directly.
 */
fun weightInputValue(lb: Double, unit: WeightUnit): String = trimDecimal(toDisplayWeight(lb, unit))

/** A weight *difference* (in lb) formatted in the display unit with a unit label, e.g. "2.5 kg"
 *  / "-1 st 2 lb" (no leading "+" on gains, matching the kg/lb form). */
fun formatWeightDelta(lbDiff: Double, unit: WeightUnit): String = when (unit) {
    WeightUnit.ST -> if (lbDiff < 0) "-${formatStoneLb(abs(lbDiff))}" else formatStoneLb(lbDiff)
    WeightUnit.KG -> "${trimDecimal(lbDiff * KG_PER_LB)} kg"
    WeightUnit.LB -> "${trimDecimal(lbDiff)} lb"
}

private val STONE_LB_REGEX =
    Regex("""^\s*(\d+(?:\.\d+)?)\s*st\s*(\d+(?:\.\d+)?)?\s*(?:lb)?\s*$""")

/** Parse a typed stones value to lb: compound "12 st 4 lb"/"12 st 4"/"12 st", or bare decimal
 *  stones "9.6"/"9.6 st" (the single-field lift input). */
private fun parseStonesToLb(s: String): Double? {
    STONE_LB_REGEX.matchEntire(s)?.let { m ->
        val st = m.groupValues[1].toDoubleOrNull() ?: return null
        val lb = m.groupValues[2].toDoubleOrNull() ?: 0.0
        return st * LB_PER_STONE + lb
    }
    val bare = s.removeSuffix("st").trim().toDoubleOrNull() ?: return null
    return bare * LB_PER_STONE
}

/**
 * Converts a user-entered string in the display unit back to lb for storage. Tolerates a trailing
 * unit suffix ("20 kg", "20kg", "20 lb") and the stones compound ("12 st 4 lb") so it round-trips
 * [formatWeight]/[weightInputValue] output.
 */
fun parseToLb(input: String, unit: WeightUnit): Double? {
    // See [normalizeDecimalInput] — a comma-locale keyboard produces "82,5", which toDoubleOrNull
    // rejects. Normalise before any of the unit branches touch it.
    val cleaned = normalizeDecimalInput(input).lowercase()
    // A suffix the user actually typed states the unit they MEAN, and it wins over the setting.
    // Each branch used to strip only its own suffix and then apply the setting's conversion, so
    // "20 kg" typed while the app was in lb matched no branch at all, fell through to null, and was
    // logged as a weightless bodyweight set. A "st"-suffixed value in kg mode was read as kilos.
    explicitUnitToLb(cleaned)?.let { return it }
    return when (unit) {
        WeightUnit.KG -> cleaned.removeSuffix("kg").trim().toDoubleOrNull()?.let { it / KG_PER_LB }
        WeightUnit.LB -> cleaned.removeSuffix("lb").trim().toDoubleOrNull()
        WeightUnit.ST -> parseStonesToLb(cleaned)
    }
}

/** `"20 kg"` / `"9.6 st"` / `"135lb"` → lb, whatever the display unit is. Null when the text carries
 *  no unit of its own (the overwhelmingly common case), leaving the setting to decide. */
private fun explicitUnitToLb(cleaned: String): Double? {
    val m = EXPLICIT_UNIT_REGEX.matchEntire(cleaned) ?: return null
    val value = m.groupValues[1].toDoubleOrNull() ?: return null
    return when (m.groupValues[2]) {
        "kg", "kgs" -> value / KG_PER_LB
        "st" -> value * LB_PER_STONE
        else -> value // lb / lbs
    }
}

private val EXPLICIT_UNIT_REGEX =
    Regex("""^\s*(\d+(?:\.\d+)?)\s*(kgs?|lbs?|st)\s*$""")

/**
 * Canonical stored weight text (always lb) for what the user typed in the display unit. A numeric
 * kg/stones entry is converted to its lb value; anything non-numeric ("BW", "2 plates") passes
 * through unchanged for [com.forge.app.domain.parser.WeightParser] to interpret. Used by BOTH the log
 * and edit paths so unit handling can never diverge between them.
 */
fun toStoredWeightText(input: String, unit: WeightUnit): String {
    val trimmed = input.trim()
    if (unit == WeightUnit.LB) return trimmed
    val lb = parseToLb(trimmed, unit) ?: return trimmed
    return if (lb % 1.0 == 0.0) "${lb.toInt()}" else String.format(Locale.US, "%.1f", lb)
}

fun unitLabel(unit: WeightUnit): String = unit.label

// ─── Legacy boolean-flag overloads (true = kg, false = lb) ───────────────────────────────────────
// The app long modelled the weight unit as a `useKg: Boolean`. These bridges keep every existing
// call site compiling and behaving exactly as before while sites migrate to the tri-state
// [WeightUnit]; a boolean can only ever be kg or lb, so an un-migrated site safely never shows stones.

fun formatWeight(lb: Double, useKg: Boolean): String = formatWeight(lb, WeightUnit.ofKg(useKg))
fun formatVolume(volumeLb: Double, useKg: Boolean): String = formatVolume(volumeLb, WeightUnit.ofKg(useKg))
fun formatVolumeCompact(volumeLb: Double, useKg: Boolean, withUnit: Boolean = true): String =
    formatVolumeCompact(volumeLb, WeightUnit.ofKg(useKg), withUnit)
fun toDisplayWeight(lb: Double, useKg: Boolean): Double = toDisplayWeight(lb, WeightUnit.ofKg(useKg))
fun fromDisplayWeight(value: Double, useKg: Boolean): Double = fromDisplayWeight(value, WeightUnit.ofKg(useKg))
fun weightInputValue(lb: Double, useKg: Boolean): String = weightInputValue(lb, WeightUnit.ofKg(useKg))
fun formatWeightDelta(lbDiff: Double, useKg: Boolean): String = formatWeightDelta(lbDiff, WeightUnit.ofKg(useKg))
fun parseToLb(input: String, useKg: Boolean): Double? = parseToLb(input, WeightUnit.ofKg(useKg))
fun toStoredWeightText(input: String, useKg: Boolean): String = toStoredWeightText(input, WeightUnit.ofKg(useKg))
fun unitLabel(useKg: Boolean): String = WeightUnit.ofKg(useKg).label
