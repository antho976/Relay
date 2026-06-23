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
    /** Distinct calendar days this ISO week with an active (non-rest) session — the hero headline. */
    val cardioDaysThisWeek: Int = 0,
    /** Weekly cardio-minutes goal (Settings). 0 = no goal set → no progress bar. */
    val weekTargetMin: Int = 0,
    /** Consecutive days (ending today or yesterday) with an active cardio session. */
    val cardioStreakDays: Int = 0,
    /** Per-day Mon–Sun cells (index 0 = Monday); future days are empty. Drives the week row. */
    val weekDays: List<CardioDayCell> = emptyList(),
    val entries: List<CardioEntry> = emptyList(),
    /** Latest logged bodyweight (lb), or null — scales the per-entry calorie estimate. */
    val bodyweightLb: Double? = null,
    val sheetOpen: Boolean = false,
    /** Non-null when the open sheet is editing an existing entry (vs logging a new one). */
    val editing: CardioEntry? = null,
    val pendingDeleteId: Long? = null,
    /** The swipeable week-stats overlay (per-week bars, graphs, numbers). */
    val detailOpen: Boolean = false,
    /** Non-null → the per-session stats overlay is open for this entry id. */
    val sessionDetailId: Long? = null,
    /** False → the main list shows only the 5 most-recent entries; true → the full history. */
    val historyExpanded: Boolean = false,
    /** True once the user permanently dismisses the "connect a watch/ring" hint banner. */
    val wearableHintDismissed: Boolean = false
)
