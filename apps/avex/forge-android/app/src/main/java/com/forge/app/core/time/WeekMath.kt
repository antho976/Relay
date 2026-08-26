package com.forge.app.core.time

import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit

/**
 * Epoch-ms of the start (00:00) of the ISO week (Monday-anchored) containing [nowMs], in [zone].
 * The single source for the app's "this week" boundary — used by the cardio log, the Stats PR
 * highlight, and anywhere else that buckets by week, so the anchor can't drift between surfaces.
 */
fun mondayStartMs(nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long =
    Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        .with(DayOfWeek.MONDAY).atStartOfDay(zone).toInstant().toEpochMilli()

/**
 * Epoch-ms of the start (00:00 on the 1st) of the calendar month containing [nowMs], in [zone].
 * The "this month" boundary for month-scoped custom goals — mirrors [mondayStartMs].
 */
fun monthStartMs(nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long =
    Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        .withDayOfMonth(1).atStartOfDay(zone).toInstant().toEpochMilli()

/**
 * The half-open window [start, end) that a deload applied at [appliedMs] governs, anchored to local
 * Monday midnights like every other week in the coach.
 *
 * It used to be a rolling 7 x 24 h from the instant the user tapped Apply, and the result is
 * PERSISTED into `session.deload_marked_here`. A deload applied Monday 19:00 therefore still
 * counted the following Monday at 08:00 — the first session of the NEXT block — as a deload
 * session. That moved `WeeklyReview.mesocycleFocus`'s block anchor forward a week, and made
 * DeloadAdvisor's stall and fatigue reads treat a normal heavy session as a deload one when
 * deciding the next deload. Whether history was corrupted came down to what time of day the user
 * happened to tap a button.
 *
 * Applied Monday to THURSDAY, the window is that ISO week — Thursday still leaves four days, which
 * is the floor. Applied Friday to Sunday, where an ISO week alone would leave three days or fewer,
 * it runs to the end of the following week. Either way the reduced program governs at least four
 * days and every boundary lands on a Monday midnight. (This paragraph said "Thursday to Sunday"
 * until WeekMathTest pinned the branch and showed the code had always split at Friday.)
 */
fun deloadWeekStartMs(appliedMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long =
    mondayStartMs(appliedMs, zone)

/** End (exclusive) of the window described by [deloadWeekStartMs]. */
fun deloadWeekEndMs(appliedMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long {
    val applied = Instant.ofEpochMilli(appliedMs).atZone(zone).toLocalDate()
    val monday = applied.with(DayOfWeek.MONDAY)
    val daysLeftInWeek = ChronoUnit.DAYS.between(applied, monday.plusWeeks(1))
    val weeks = if (daysLeftInWeek >= 4) 1L else 2L
    return monday.plusWeeks(weeks).atStartOfDay(zone).toInstant().toEpochMilli()
}
