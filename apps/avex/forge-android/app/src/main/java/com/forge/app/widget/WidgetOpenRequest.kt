package com.forge.app.widget

import com.forge.app.program.Program

/**
 * One widget tap, as an EVENT rather than a value (M-30).
 *
 * The day to open was held as a plain sticky string, and the nav host opened it from an effect
 * keyed on that string. Tapping the same widget twice — open `upper-a`, Back to Home, tap it again
 * while the `singleTask` activity is still alive — assigned the same value, produced no state
 * change, and never re-ran the effect: the widget's primary action silently did nothing on
 * ordinary repeated use. [id] is what makes the second tap a second event.
 */
data class WidgetOpenRequest(val id: Long, val dayKey: String)

/** Where a widget tap should land, once the program is known. See [widgetDestinationFor]. */
sealed interface WidgetDestination {
    /** A program day: opens on top of the hub, so Back returns home. */
    data class GymDay(val dayKey: String) : WidgetDestination

    /** A cardio widget: selects the hub's Cardio page rather than stacking a screen. */
    data object CardioTab : WidgetDestination

    /**
     * A key the loaded program does not have — a day removed since the widget last updated. The
     * request is still consumed: leaving it pending is what made the cold custom-program race
     * unrecoverable, and re-firing it forever would be worse than landing on Home.
     */
    data object Unrecognised : WidgetDestination
}

/**
 * What to do with a widget tap right now, given how far the program load has got (M-30).
 *
 * The distinction the four-second poll could not make: a program still loading and a program that
 * does not contain the day looked identical once the timer expired, so a slow cold start discarded
 * the tap. Only [Program.Readiness.LOADED] can answer the question at all.
 */
sealed interface WidgetRouting {
    /**
     * Not answerable yet — keep the request. Covers a load in flight AND one that failed: neither
     * says anything about whether the day exists, and consuming the tap on either is how the
     * widget's primary action silently did nothing.
     */
    data object Wait : WidgetRouting

    /** A real program answered: route [destination] and retire the request either way. */
    data class Decided(val destination: WidgetDestination) : WidgetRouting
}

/** [widgetDestinationFor], gated on readiness. */
fun widgetRoutingFor(
    readiness: Program.Readiness,
    dayKey: String,
    programDayKeys: List<String>
): WidgetRouting =
    if (readiness != Program.Readiness.LOADED) WidgetRouting.Wait
    else WidgetRouting.Decided(widgetDestinationFor(dayKey, programDayKeys))

/** The cardio widget's key prefix. Its taps select a hub page instead of opening a day. */
private const val CARDIO_KEY_PREFIX = "cardio"

/**
 * Where [dayKey] belongs, validated against [programDayKeys] — which must be the LOADED program,
 * not the seed split.
 *
 * The cold-launch race is why that distinction matters: `Program` reports the hard-coded seed
 * until the database program is loaded into it, so a widget tap for a custom builder day was
 * validated against a split that had never contained it, rejected, and — because the sticky string
 * did not change afterwards — never retried. The caller waits for readiness before asking.
 */
fun widgetDestinationFor(dayKey: String, programDayKeys: List<String>): WidgetDestination = when {
    dayKey in programDayKeys -> WidgetDestination.GymDay(dayKey)
    dayKey.startsWith(CARDIO_KEY_PREFIX) -> WidgetDestination.CardioTab
    else -> WidgetDestination.Unrecognised
}
