package com.forge.app.domain.cardio

/**
 * Pure, Android-free shapes for the wearable side of cardio (steps-by-hour + GPS route).
 *
 * The cardio UI is built against these now, but they are NOT yet populated from Health
 * Connect — that read path (a separate opt-in `StepsRecord` / `ExerciseSessionRecord`
 * permission on [com.forge.app.data.health.HealthConnectManager]) lands in a focused
 * follow-up. Until then every wearable slot in the UI is null and falls back to a quiet
 * "connect a wearable" placeholder, so wiring the real data is a drop-in, not a refactor.
 *
 * Keeping these in `domain` (no Compose / no Android deps) means they're unit-testable and
 * the same instances flow from the eventual HC read straight into the composables.
 */

/** Active steps recorded in one wall-clock hour of a day ([hour] is 0..23, local time). */
data class HourlySteps(val hour: Int, val steps: Int)

/**
 * One sample of a recorded GPS track. Rendered offline as a shape-only polyline
 * ([com.forge.app.ui.cardio.components.RouteThumbnail]) — Forge has no internet, so there
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
