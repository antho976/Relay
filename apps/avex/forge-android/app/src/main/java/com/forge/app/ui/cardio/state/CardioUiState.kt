package com.forge.app.ui.cardio.state

import com.forge.app.data.db.entities.CardioEntry

/**
 * Cardio screen state. Entries are the raw rows from the DB (newest first); the UI
 * decodes type/effort/restReason strings to enums when rendering.
 */
/** One data point for the pace trend chart (#78): avg pace in min/km for a run entry. */
data class PaceTrendPoint(val dateMs: Long, val paceMinPerKm: Double)

data class CardioUiState(
    val isLoading: Boolean = true,
    val weekMinutes: Int = 0,
    val weekEntryCount: Int = 0,
    /** Minutes per day for the current Mon–Sun week, index 0 = Monday. Future days = 0. */
    val weekDailyMinutes: List<Int> = emptyList(),
    val entries: List<CardioEntry> = emptyList(),
    val sheetOpen: Boolean = false,
    val pendingDeleteId: Long? = null,
    /** Active type filter. Null = All (#63). */
    val selectedTypeFilter: String? = null,
    /** Lifetime total distance across all entries in km (#79). */
    val lifetimeDistanceKm: Double = 0.0,
    /** Run pace trend — min/km per run entry (#78). */
    val paceTrend: List<PaceTrendPoint> = emptyList()
) {
    val filteredEntries: List<CardioEntry>
        get() = if (selectedTypeFilter == null) entries
                else entries.filter { it.type == selectedTypeFilter }
}
