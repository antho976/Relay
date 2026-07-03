package com.forge.app.core.time

import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId

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
