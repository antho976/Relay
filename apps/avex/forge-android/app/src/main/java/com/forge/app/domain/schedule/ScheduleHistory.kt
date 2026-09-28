package com.forge.app.domain.schedule

import org.json.JSONArray
import org.json.JSONObject
import java.time.LocalDate

/**
 * What the weekly schedule was on any given day, so a past day can be told apart as a PLANNED rest
 * day or a MISSED training day.
 *
 * The schedule itself is only ever "now": paint last month's calendar from it and a user who moved
 * from Mon/Wed/Fri to Tue/Thu/Sat would see every old Monday turn into a rest day after the fact.
 * So every change is recorded with the day it took effect, and a day is judged by the schedule that
 * was in force on it. A day before the first record has no known plan and is never called a rest
 * day — the history starts when it starts, rather than guessing backwards.
 *
 * Only weekday mode plans rest. "Whenever I can" (sequence) has no fixed days, so none of its days
 * are planned rest.
 */
object ScheduleHistory {

    /** From [fromEpochDay] on (until the next entry), the schedule was [mode] with [slots] (Mon..Sun). */
    data class Entry(val fromEpochDay: Long, val mode: String, val slots: List<String>)

    /** A long user keeps a bounded record: a year of weekly changes is still far below this. */
    private const val MAX_ENTRIES = 200

    fun parse(json: String?): List<Entry> {
        if (json.isNullOrBlank()) return emptyList()
        return runCatching {
            val arr = JSONArray(json)
            (0 until arr.length()).map { i ->
                val o = arr.getJSONObject(i)
                Entry(o.getLong("from"), o.getString("mode"), WeeklySchedule.parse(o.optString("slots", "")))
            }.sortedBy { it.fromEpochDay }
        }.getOrDefault(emptyList())
    }

    fun encode(entries: List<Entry>): String = JSONArray(entries.map { e ->
        JSONObject().apply {
            put("from", e.fromEpochDay)
            put("mode", e.mode)
            put("slots", WeeklySchedule.encode(e.slots))
        }
    }).toString()

    /**
     * [entries] with the schedule as of [today] recorded. A second change on the same day replaces
     * that day's entry (the day is judged by where it ended up), and a "change" that matches what is
     * already in force records nothing.
     */
    fun record(entries: List<Entry>, today: Long, mode: String, slots: List<String>): List<Entry> {
        val normalized = WeeklySchedule.parse(WeeklySchedule.encode(slots))
        val kept = entries.filter { it.fromEpochDay < today }
        val inForce = kept.lastOrNull()
        if (inForce != null && inForce.mode == mode && inForce.slots == normalized) return kept
        return (kept + Entry(today, mode, normalized)).takeLast(MAX_ENTRIES)
    }

    /** The entry in force on [epochDay], or null before the history starts. */
    fun entryOn(entries: List<Entry>, epochDay: Long): Entry? =
        entries.lastOrNull { it.fromEpochDay <= epochDay }

    /**
     * What the plan in force on [date] said about it: true = a rest day, false = a training day,
     * null = no fixed plan (before the history starts, sequence mode, or a weekday schedule with
     * nothing on it — that plans no week at all, not seven rest days).
     */
    fun planOn(entries: List<Entry>, date: LocalDate): Boolean? {
        val entry = entryOn(entries, date.toEpochDay()) ?: return null
        if (entry.mode != WeeklySchedule.MODE_WEEKDAY) return null
        if (entry.slots.all { it.isBlank() }) return null
        return entry.slots.getOrElse(date.dayOfWeek.value - 1) { "" }.isBlank()
    }

    /** True when [date] was a planned rest day under the schedule in force on it. */
    fun isPlannedRest(entries: List<Entry>, date: LocalDate): Boolean = planOn(entries, date) == true

    /** Every planned rest day in [from]..[to] inclusive, as epoch days. */
    fun plannedRestDays(entries: List<Entry>, from: LocalDate, to: LocalDate): Set<Long> {
        if (entries.isEmpty() || to.isBefore(from)) return emptySet()
        val out = HashSet<Long>()
        var d = maxOf(from, LocalDate.ofEpochDay(entries.first().fromEpochDay))
        while (!d.isAfter(to)) {
            if (isPlannedRest(entries, d)) out += d.toEpochDay()
            d = d.plusDays(1)
        }
        return out
    }
}
