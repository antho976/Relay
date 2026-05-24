package com.forge.app.domain.units

private const val KG_PER_LB = 0.45359237

/** Converts a stored lb value to the display unit and formats it. */
fun formatWeight(lb: Double, useKg: Boolean): String {
    if (useKg) {
        val kg = lb * KG_PER_LB
        return if (kg % 1.0 == 0.0) "${kg.toInt()} kg" else "%.1f kg".format(kg)
    }
    return if (lb % 1.0 == 0.0) "${lb.toInt()} lb" else "%.1f lb".format(lb)
}

/** Converts a user-entered string in the display unit back to lb for storage. */
fun parseToLb(input: String, useKg: Boolean): Double? {
    val numeric = input.toDoubleOrNull() ?: return null
    return if (useKg) numeric / KG_PER_LB else numeric
}

fun unitLabel(useKg: Boolean): String = if (useKg) "kg" else "lb"
