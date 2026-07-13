package com.forge.app.domain.notify

import org.json.JSONArray
import org.json.JSONObject
import java.time.DayOfWeek
import java.time.LocalDateTime

/**
 * One day's quiet-hours window (GYMAP-75). Hours are 0–23, whole-hour granularity (matching the
 * `HourPickerRow` the settings UI edits with). [start] == [end] is a zero-length window meaning
 * **no quiet hours that day**; [start] > [end] crosses midnight into the following day.
 */
data class QuietWindow(val start: Int, val end: Int) {
    /** True when this day has no quiet window (a zero-length span). */
    val isOff: Boolean get() = start == end
}

/**
 * Per-day quiet-hours schedule (GYMAP-75) — a window for each weekday so the mute band can differ
 * by day (e.g. a later start on weekends). Stored as one JSON blob in DataStore, seeded on first
 * read from the app's previous single global window so upgrading users keep their setting.
 *
 * [windows] is always exactly 7 entries, **Monday-first** and indexed by `DayOfWeek.value - 1`
 * (MONDAY=0 … SUNDAY=6) regardless of the user's week-start display preference — the storage order
 * is canonical so flipping "week starts Monday" never rewrites the data.
 */
data class QuietHoursSchedule(val windows: List<QuietWindow>) {

    fun windowFor(day: DayOfWeek): QuietWindow = windows[day.value - 1]

    /** A copy with [day]'s window replaced — the edit primitive the settings screen writes through. */
    fun withWindow(day: DayOfWeek, window: QuietWindow): QuietHoursSchedule =
        QuietHoursSchedule(windows.mapIndexed { i, w -> if (i == day.value - 1) window else w })

    /** True when a single [start]/[end] pair applies to every day (the migrated-but-untouched case),
     *  used to keep the settings summary line terse. */
    val isUniform: Boolean get() = windows.all { it == windows[0] }

    /**
     * True when [now] falls inside the quiet window for its weekday. A window that crosses midnight
     * is owned by the day it STARTS on, so this also checks the previous day's window spilling into
     * the current early hours — matching how the old single global window behaved for all-days-equal
     * schedules (no regression for upgraded users).
     */
    fun isQuietAt(now: LocalDateTime): Boolean {
        val h = now.hour
        val today = windowFor(now.dayOfWeek)
        if (!today.isOff) {
            val inToday = if (today.start <= today.end) h in today.start until today.end
                          else h >= today.start        // evening side of a midnight-crossing window
            if (inToday) return true
        }
        val yesterday = windowFor(now.dayOfWeek.minus(1))
        // Morning spill-over from a window that started yesterday and crossed midnight.
        if (!yesterday.isOff && yesterday.start > yesterday.end && h < yesterday.end) return true
        return false
    }

    companion object {
        const val DAYS = 7
        const val DEFAULT_START = 22
        const val DEFAULT_END = 7

        /** Every day sharing one window — the schedule's starting point and the upgrade seed. */
        fun uniform(start: Int, end: Int) =
            QuietHoursSchedule(List(DAYS) { QuietWindow(start, end) })

        fun default() = uniform(DEFAULT_START, DEFAULT_END)

        /** Bump if the persisted shape changes so an older build's blob is discarded, not misread. */
        private const val SCHEMA = 1

        fun toJson(schedule: QuietHoursSchedule): String = JSONObject().apply {
            put("schema", SCHEMA)
            put("days", JSONArray(schedule.windows.map { w ->
                JSONObject().apply {
                    put("s", w.start)
                    put("e", w.end)
                }
            }))
        }.toString()

        /**
         * Tolerant parse. When [json] is null/blank/corrupt/stale-schema (the pre-per-day state, or a
         * downgrade artifact) the schedule is seeded uniformly from [legacyStart]/[legacyEnd] — the
         * user's previous single window — so nothing is lost on upgrade.
         */
        fun fromJson(json: String?, legacyStart: Int, legacyEnd: Int): QuietHoursSchedule {
            if (json.isNullOrBlank()) return uniform(legacyStart, legacyEnd)
            return runCatching {
                val root = JSONObject(json)
                if (root.optInt("schema", -1) != SCHEMA) return uniform(legacyStart, legacyEnd)
                val arr = root.optJSONArray("days") ?: return uniform(legacyStart, legacyEnd)
                if (arr.length() != DAYS) return uniform(legacyStart, legacyEnd)
                QuietHoursSchedule(List(DAYS) { i ->
                    val o = arr.optJSONObject(i)
                    val s = (o?.optInt("s", legacyStart) ?: legacyStart).coerceIn(0, 23)
                    val e = (o?.optInt("e", legacyEnd) ?: legacyEnd).coerceIn(0, 23)
                    QuietWindow(s, e)
                })
            }.getOrDefault(uniform(legacyStart, legacyEnd))
        }
    }
}
