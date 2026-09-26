package com.forge.app.service

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.DayOfWeek
import java.time.ZoneId
import java.time.ZonedDateTime

/**
 * The WorkManager re-arm rules that used to cancel or shift the reminder, the weekly recap and the
 * midnight widget redraw (2026-09-26 audit, 11: W08, W09, D3).
 */
class WorkScheduleTest {

    private val toronto = ZoneId.of("America/Toronto")
    private fun at(y: Int, mo: Int, d: Int, h: Int, mi: Int = 0, zone: ZoneId = toronto) =
        ZonedDateTime.of(y, mo, d, h, mi, 0, 0, zone)

    // ── Training reminder (W09) ───────────────────────────────────────────────

    @Test
    fun `boot keeps the reminder that is running now`() {
        assertTrue(TrainingReminderWorker.keepsQueuedRun(true, Long.MAX_VALUE, 18, toronto))
    }

    @Test
    fun `boot keeps a due reminder that is still on its hour`() {
        // The 18:00 run that woke a dead process: REPLACE here pushed it to tomorrow.
        val due = at(2026, 9, 26, 18).toInstant().toEpochMilli()
        assertTrue(TrainingReminderWorker.keepsQueuedRun(false, due, 18, toronto))
        // Whole-minute delay rounding lands it seconds early; a late periodic a few minutes after.
        assertTrue(TrainingReminderWorker.keepsQueuedRun(false, due - 30_000, 18, toronto))
        assertTrue(TrainingReminderWorker.keepsQueuedRun(false, due + 10 * 60_000, 18, toronto))
    }

    @Test
    fun `boot re-anchors a reminder the DST change moved off its hour`() {
        // 24 h of elapsed time across the spring-forward (2026-03-08): 18:00 became 19:00 local.
        val drifted = at(2026, 3, 9, 19).toInstant().toEpochMilli()
        assertFalse(TrainingReminderWorker.keepsQueuedRun(false, drifted, 18, toronto))
        val early = at(2026, 11, 2, 17).toInstant().toEpochMilli()
        assertFalse(TrainingReminderWorker.keepsQueuedRun(false, early, 18, toronto))
    }

    @Test
    fun `a flight re-anchors the reminder`() {
        val queuedInToronto = at(2026, 9, 26, 18).toInstant().toEpochMilli()
        assertFalse(TrainingReminderWorker.keepsQueuedRun(false, queuedInToronto, 18, ZoneId.of("Europe/London")))
    }

    @Test
    fun `a midnight reminder is on time across the day boundary`() {
        val justBefore = at(2026, 9, 26, 23, 59).plusSeconds(30).toInstant().toEpochMilli()
        assertTrue(TrainingReminderWorker.keepsQueuedRun(false, justBefore, 0, toronto))
        assertTrue(TrainingReminderWorker.keepsQueuedRun(false, at(2026, 9, 27, 0, 5).toInstant().toEpochMilli(), 0, toronto))
    }

    @Test
    fun `a queued run with no next time is replaced`() {
        assertFalse(TrainingReminderWorker.keepsQueuedRun(false, Long.MAX_VALUE, 18, toronto))
    }

    // ── Weekly recap (W08) ────────────────────────────────────────────────────

    @Test
    fun `an install on Saturday recaps the coming Monday, not the one after`() {
        val runAt = WeeklyRecapWorker.nextRecapAt(at(2026, 9, 12, 12))
        assertEquals(at(2026, 9, 14, 12), runAt)
    }

    @Test
    fun `reopening on Sunday re-arms the same Monday`() {
        val saturday = WeeklyRecapWorker.nextRecapAt(at(2026, 9, 12, 12))
        val sunday = WeeklyRecapWorker.nextRecapAt(at(2026, 9, 13, 20))
        assertEquals(DayOfWeek.MONDAY, sunday.dayOfWeek)
        assertEquals(WeeklyRecapWorker.workNameFor(saturday), WeeklyRecapWorker.workNameFor(sunday))
    }

    @Test
    fun `the Monday run arms next Monday under a different name`() {
        val thisRun = at(2026, 9, 14, 12)
        val next = WeeklyRecapWorker.nextRecapAt(thisRun.plusSeconds(5))
        assertEquals(at(2026, 9, 21, 12), next)
        assertNotEquals(WeeklyRecapWorker.workNameFor(thisRun), WeeklyRecapWorker.workNameFor(next))
    }

    @Test
    fun `the recap stays at Monday noon across a DST change`() {
        // Toronto springs forward on Sunday 2026-03-08.
        assertEquals(at(2026, 3, 9, 12), WeeklyRecapWorker.nextRecapAt(at(2026, 3, 7, 9)))
    }

    // ── Midnight widget redraw (D3) ───────────────────────────────────────────

    @Test
    fun `the midnight run arms its successor under a different name`() {
        val midnight = at(2026, 9, 27, 0).toInstant().toEpochMilli()
        // Armed the evening before, then run at that midnight.
        val armed = WidgetMidnightWorker.nextMidnightMs(at(2026, 9, 26, 21).toInstant().toEpochMilli(), toronto)
        assertEquals(midnight, armed)
        val successor = WidgetMidnightWorker.nextMidnightMs(midnight, toronto)
        assertEquals(at(2026, 9, 28, 0).toInstant().toEpochMilli(), successor)
        assertNotEquals(WidgetMidnightWorker.workNameFor(armed), WidgetMidnightWorker.workNameFor(successor))
    }

    @Test
    fun `launches on the same day re-arm the same midnight`() {
        val morning = WidgetMidnightWorker.nextMidnightMs(at(2026, 9, 26, 7).toInstant().toEpochMilli(), toronto)
        val evening = WidgetMidnightWorker.nextMidnightMs(at(2026, 9, 26, 22).toInstant().toEpochMilli(), toronto)
        assertEquals(WidgetMidnightWorker.workNameFor(morning), WidgetMidnightWorker.workNameFor(evening))
    }

    @Test
    fun `a flight arms the new zone's midnight`() {
        val now = at(2026, 9, 26, 7).toInstant().toEpochMilli()
        assertNotEquals(
            WidgetMidnightWorker.nextMidnightMs(now, toronto),
            WidgetMidnightWorker.nextMidnightMs(now, ZoneId.of("Europe/London"))
        )
    }
}
