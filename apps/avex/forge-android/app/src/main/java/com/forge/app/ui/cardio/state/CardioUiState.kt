package com.forge.app.ui.cardio.state

import com.forge.app.data.db.entities.CardioEntry

/**
 * Cardio screen state. Entries are the raw rows from the DB (newest first); the UI
 * decodes type/effort/restReason strings to enums when rendering.
 */
data class CardioUiState(
    val isLoading: Boolean = true,
    val weekMinutes: Int = 0,
    val weekEntryCount: Int = 0,
    /** Minutes per day for the current Mon–Sun week, index 0 = Monday. Future days = 0. */
    val weekDailyMinutes: List<Int> = emptyList(),
    val entries: List<CardioEntry> = emptyList(),
    val sheetOpen: Boolean = false,
    val pendingDeleteId: Long? = null
)
