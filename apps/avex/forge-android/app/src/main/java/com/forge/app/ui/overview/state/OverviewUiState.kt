package com.forge.app.ui.overview.state

data class OverviewUiState(
    val workoutsThisWeek: Int = 0,
    val volumeThisWeekLb: Double = 0.0,
    val cardioMinutesThisWeek: Int = 0,
    val totalFinishedSessions: Int = 0,
    val lastDeloadAtSessionCount: Int = 0
) {
    val sessionsSinceLastDeload: Int
        get() = (totalFinishedSessions - lastDeloadAtSessionCount).coerceAtLeast(0)

    val needsDeload: Boolean
        get() = sessionsSinceLastDeload >= DELOAD_THRESHOLD

    companion object {
        /** Per the React prototype — after 24 sessions, suggest a deload week. */
        const val DELOAD_THRESHOLD: Int = 24
    }
}
