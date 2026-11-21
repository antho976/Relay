package com.forge.app.domain.cardio

/**
 * Pure, Android-free shapes for the wearable side of cardio (steps-by-hour + GPS route).
 *
 * Populated from Health Connect via [com.forge.app.data.health.HealthConnectManager] —
 * `readStepsDay` fills the hourly steps, `matchSessionRoute` resolves a watch session's GPS
 * track (each behind its own opt-in grant). Vendor-neutral: any watch or ring that feeds
 * Health Connect (Samsung Health for Galaxy, Fitbit for Pixel, …) lands here identically.
 * When nothing fed Health Connect the slots stay null/empty and the UI falls back to a
 * quiet "connect a wearable" placeholder.
 *
 * Keeping these in `domain` (no Compose / no Android deps) means they're unit-testable and
 * the same instances flow from the HC read straight into the composables.
 */

/** Active steps recorded in one wall-clock hour of a day ([hour] is 0..23, local time). */
data class HourlySteps(val hour: Int, val steps: Int)

/**
 * One sample of a recorded GPS track. Rendered offline as a shape-only polyline
 * ([com.forge.app.ui.cardio.components.RouteThumbnail]) — Avex has no internet, so there
 * is deliberately no basemap; only the path's shape is drawn.
 */
data class RoutePoint(val lat: Double, val lng: Double)

/**
 * The wearable-derived companion to a day of cardio: an hour-by-hour step breakdown plus
 * any GPS tracks recorded that day. Null/empty whenever no wearable fed Health Connect.
 */
data class CardioWearableDay(
    val hourlySteps: List<HourlySteps> = emptyList(),
    val routes: List<List<RoutePoint>> = emptyList()
) {
    val totalSteps: Int get() = hourlySteps.sumOf { it.steps }
    val hasData: Boolean get() = hourlySteps.isNotEmpty() || routes.isNotEmpty()
}
