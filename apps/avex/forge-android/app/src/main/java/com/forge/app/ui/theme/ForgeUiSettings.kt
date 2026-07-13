package com.forge.app.ui.theme

import androidx.compose.runtime.compositionLocalOf
import com.forge.app.domain.units.WeightUnit

/**
 * App-level UI preferences propagated from MainActivity via [LocalForgeSettings].
 * All screens read from this instead of directly from DataStore so the entire
 * composition re-renders atomically when a setting changes.
 */
data class ForgeUiSettings(
    val amoledMode: Boolean = false,
    /** Weight display unit (GYMAP-72) — lb | kg | st. Read this everywhere weight is shown. */
    val weightUnit: WeightUnit = WeightUnit.LB,
    /** Cardio distance/pace unit — true = miles, false = km. Derives from the weight unit when unset. */
    val useMiles: Boolean = false,
    val hiddenOverviewTiles: Set<String> = emptySet(),
    val compactSetLogging: Boolean = false,
    val overviewTileOrder: List<String> = listOf("gym", "cardio", "trophies"),
    val dateFormat: String = "MMM d, yyyy",
    val timeFormat24h: Boolean = false,
    val firstDayMonday: Boolean = true,
    val hapticStrength: String = "strong",   // "off" | "light" | "medium" | "strong"
    val keepScreenOn: Boolean = true,        // hold the display awake while logging (GYMAP-74)
    val accentColorHex: String = "",         // empty = AccentNavy default
    val accentEnabled: Boolean = true,       // false = monochrome (neutral highlights, no accent)
    val plateWeightLb: Double = 15.0,        // weight of one plate (lb) for plate-loaded exercises
    /** True once the user has finished a workout — first-touch onboarding cards hide once set. */
    val firstWorkoutDone: Boolean = false
) {
    /** Legacy convenience — true only for kilograms (lb/stones read false). Prefer [weightUnit];
     *  kept so sites not yet migrated off the boolean unit flag still compile. */
    val useKg: Boolean get() = weightUnit == WeightUnit.KG
}

val LocalForgeSettings = compositionLocalOf { ForgeUiSettings() }
