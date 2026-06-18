package com.forge.app.ui.cardio.state

import com.forge.app.data.db.entities.CardioEntry

/** One Mon–Sun cell of the cardio week row — active minutes plus whether a rest day was logged.
 *  A single self-contained cell (vs two index-aligned parallel lists) can't drift out of sync. */
data class CardioDayCell(val minutes: Int = 0, val isRest: Boolean = false)

/**
 * Cardio screen state. Entries are the raw rows from the DB (newest first); the UI
 * decodes type/effort/restReason strings to enums when rendering.
 */
data class CardioUiState(
    val isLoading: Boolean = true,
    val weekMinutes: Int = 0,
    /** Weekly cardio-minutes goal (Settings). 0 = no goal set → no progress bar. */
    val weekTargetMin: Int = 0,
    /** Total active (non-rest) distance this ISO week, km. */
    val weekDistanceKm: Double = 0.0,
    /** Last ISO week's active minutes + distance, for the hero trend line. */
    val lastWeekMinutes: Int = 0,
    val lastWeekDistanceKm: Double = 0.0,
    /** Consecutive days (ending today or yesterday) with an active cardio session. */
    val cardioStreakDays: Int = 0,
    /** Per-day Mon–Sun cells (index 0 = Monday); future days are empty. Drives the week row. */
    val weekDays: List<CardioDayCell> = emptyList(),
    val entries: List<CardioEntry> = emptyList(),
    val sheetOpen: Boolean = false,
    /** Non-null when the open sheet is editing an existing entry (vs logging a new one). */
    val editing: CardioEntry? = null,
    val pendingDeleteId: Long? = null
)
