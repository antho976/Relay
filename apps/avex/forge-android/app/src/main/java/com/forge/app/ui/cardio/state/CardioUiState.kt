package com.forge.app.ui.cardio.state

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.cardio.CardioActivityRecord
import com.forge.app.domain.cardio.CardioPaceSeries
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint

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
    /** Total distance (km) across this week's active sessions — the hero's distance figure. */
    val weekDistanceKm: Double = 0.0,
    /** Custom goals on cardio metrics (distance / minutes) — the page's GOALS trim. */
    val cardioGoals: List<ExtendedGoalRepository.Progress> = emptyList(),
    /** All-time per-activity bests (GYMAP-34) — the RECORDS block; empty until a distance session lands. */
    val cardioRecords: List<CardioActivityRecord> = emptyList(),
    /** Per-activity pace series (GYMAP-35) — the week overlay's pace-trend chart. A type appears once it
     *  has two paced sessions; empty until then. */
    val cardioPaceSeries: List<CardioPaceSeries> = emptyList(),
    val entries: List<CardioEntry> = emptyList(),
    val sheetOpen: Boolean = false,
    /** Non-null when the open sheet is editing an existing entry (vs logging a new one). */
    val editing: CardioEntry? = null,
    /** The swipeable week-stats overlay (per-week bars, graphs, numbers). */
    val detailOpen: Boolean = false,
    /** Non-null → the per-session stats overlay is open for this entry id. */
    val sessionDetailId: Long? = null,
    /** Watch steps for the open session's day (Health Connect); null until loaded / when none. */
    val sessionWearable: CardioWearableDay? = null,
    /** GPS route for the open session, once available (already-consented or just confirmed); else null. */
    val sessionRoute: List<RoutePoint>? = null,
    /** Non-null → a matching watch session has a route that needs Health Connect consent; this is the
     *  record id to hand the consent contract. Drives the "Show GPS route" button on the session sheet. */
    val sessionRouteConsentId: String? = null,
    /** Watch steps for today, shown on the current-week stats page; null until loaded / when none. */
    val weekWearable: CardioWearableDay? = null,
    /** False → the main list shows only the 5 most-recent entries; true → the full history. */
    val historyExpanded: Boolean = false,
    /** True once the user permanently dismisses the "connect a watch/ring" hint banner. */
    val wearableHintDismissed: Boolean = false,
    /** Avex holds the StepsRecord read grant — drives the steps placeholder (and hides the banner). */
    val stepsConnected: Boolean = false,
    /** Today's watch step total (GYMAP-64) — the hero's quiet "TODAY · N steps" line. Null when no watch
     *  is connected or the read failed, so a non-null value (incl. 0) means "connected, show it". */
    val todaySteps: Int? = null,
    /** Avex holds the ExerciseSession read grant — for GPS-route matching (also hides the banner). */
    val routesConnected: Boolean = false,
    /** Distance/pace unit — true shows miles, false km. Derives from the weight unit when unset. */
    val useMiles: Boolean = false,
    /** The last cardio activity code logged (GYMAP-40); the log sheet seeds a new entry to it instead
     *  of always defaulting to Run. Null until the first non-rest session. */
    val lastCardioType: String? = null
)
