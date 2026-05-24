package com.forge.app.ui.theme

import androidx.compose.runtime.compositionLocalOf

/**
 * App-level UI preferences propagated from MainActivity via [LocalForgeSettings].
 * All screens read from this instead of directly from DataStore so the entire
 * composition re-renders atomically when a setting changes.
 */
data class ForgeUiSettings(
    val amoledMode: Boolean = false,
    val useKg: Boolean = false,
    val hiddenOverviewTiles: Set<String> = emptySet(),
    val showEncouragement: Boolean = true,
    val compactSetLogging: Boolean = false,
    val overviewTileOrder: List<String> = listOf("gym", "cardio", "trophies"),
    val dateFormat: String = "MMM d, yyyy",
    val timeFormat24h: Boolean = false,
    val firstDayMonday: Boolean = true,
    val hapticStrength: String = "strong",   // "off" | "light" | "medium" | "strong"
    val quietHoursEnabled: Boolean = false,
    val quietHoursStart: Int = 22,
    val quietHoursEnd: Int = 7,
    val accentColorHex: String = ""          // empty = AccentNavy default
)

val LocalForgeSettings = compositionLocalOf { ForgeUiSettings() }
