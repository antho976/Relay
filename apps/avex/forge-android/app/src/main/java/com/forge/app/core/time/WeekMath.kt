package com.forge.app.core.time

import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
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
 * The first day of the week containing [date] in the USER's week: Monday when [firstDayMonday],
 * otherwise Sunday.
 *
 * The Home week strip and the weekly counts it sits above follow Settings → Format → Week starts;
 * [mondayStartMs] stays ISO for everything that must not move when the preference does — the
 * coach's volume weeks, the deload window and every stored week id. Home used to be Monday-first
 * whatever the setting said (2026-09-26 audit, "Settings that do nothing").
 */
fun userWeekStart(date: LocalDate, firstDayMonday: Boolean): LocalDate =
    date.minusDays(userWeekDayIndex(date, firstDayMonday).toLong())

/** Epoch-ms of local midnight on [userWeekStart] for the day containing [nowMs], in [zone]. */
fun userWeekStartMs(nowMs: Long, zone: ZoneId = ZoneId.systemDefault(), firstDayMonday: Boolean): Long =
    userWeekStart(Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate(), firstDayMonday)
        .atStartOfDay(zone).toInstant().toEpochMilli()

/**
 * [date]'s 0-based position in the user's week: 0=Mon..6=Sun when [firstDayMonday], otherwise
 * 0=Sun..6=Sat (`DayOfWeek.value` is 1=Mon..7=Sun, so `% 7` puts Sunday at 0).
 */
fun userWeekDayIndex(date: LocalDate, firstDayMonday: Boolean): Int =
    if (firstDayMonday) date.dayOfWeek.value - 1 else date.dayOfWeek.value % 7

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
