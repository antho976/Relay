package com.forge.wear.data

import android.content.Context

/**
 * How far this watch's wall clock sits from the phone's, in milliseconds.
 *
 * The phone stamps absolute instants — a rest timer's `endAtMs` above all — on ITS clock, and the
 * two devices' clocks are only periodically reconciled. The in-app countdown already handles this
 * by working in durations ([com.forge.wear.ui.RestCountdown]), which needs the moment a payload
 * ARRIVED, and a complication has no such moment: it renders in a short-lived binder callback that
 * reads the DataItem cold, so it was comparing a phone instant against `System.currentTimeMillis()`
 * directly. Every millisecond of skew was a millisecond of error in the countdown on the watch
 * face, in the opposite direction from the same countdown inside the app.
 *
 * One number, persisted, measured wherever a payload carries both clocks. Stored in
 * SharedPreferences rather than held in memory because the surfaces that need it — tiles,
 * complications — routinely run in a process that has never opened the app.
 */
object WearClockSkew {

    private const val PREFS = "avex_wear_clock"
    private const val KEY_OFFSET_MS = "phone_offset_ms"
    private const val KEY_MEASURED_AT_MS = "measured_at_ms"

    /**
     * How long a measurement is trusted.
     *
     * Wear's own time sync moves the watch clock, which invalidates any earlier reading. A day is
     * far longer than any rest timer and short enough that a stale correction cannot outlive the
     * session that produced it; past it the raw phone instant is used, which is the pre-existing
     * behaviour.
     */
    const val MAX_AGE_MS: Long = 24 * 60 * 60 * 1000L

    /** Record a reading of both clocks taken at (near enough) the same instant. */
    fun record(context: Context, phoneNowMs: Long, watchNowMs: Long) {
        if (phoneNowMs <= 0L) return
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putLong(KEY_OFFSET_MS, watchNowMs - phoneNowMs)
            .putLong(KEY_MEASURED_AT_MS, watchNowMs)
            .apply()
    }

    /**
     * Turn an instant on the PHONE's clock into one on this watch's, or return it unchanged when
     * there is no usable measurement — the honest fallback, and exactly what every caller did
     * before this existed.
     */
    fun toWatchInstant(context: Context, phoneInstantMs: Long, watchNowMs: Long): Long {
        val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val measuredAt = prefs.getLong(KEY_MEASURED_AT_MS, 0L)
        val offset = prefs.getLong(KEY_OFFSET_MS, 0L)
        return phoneInstantMs + usableOffset(offset, measuredAt, watchNowMs)
    }

    /**
     * The offset to actually apply: the stored one, or zero when it was never measured or is too
     * old to trust. Split out from the storage so the rule is testable without a Context.
     */
    fun usableOffset(offsetMs: Long, measuredAtMs: Long, watchNowMs: Long): Long =
        // A measurement stamped in the future is a clock that moved backwards since — the one case
        // where the recorded offset is certainly wrong rather than merely old.
        if (measuredAtMs <= 0L || watchNowMs < measuredAtMs || watchNowMs - measuredAtMs > MAX_AGE_MS) 0L
        else offsetMs
}
