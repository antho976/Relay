package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate

class RecurrenceTest {

    private fun d(m: Int, day: Int, y: Int = 2026) = LocalDate.of(y, m, day)

    @Test fun `month-end anchors do not drift`() {
        val r = Recurrence(d(1, 31), Frequency.MONTHLY)
        assertEquals(d(2, 28), r.occurrence(1))
        assertEquals(d(3, 31), r.occurrence(2))
        assertEquals(d(3, 31), r.after(d(2, 28)))
    }

    @Test fun `on or after returns the anchor before it starts`() {
        val r = Recurrence(d(5, 10), Frequency.MONTHLY)
        assertEquals(d(5, 10), r.onOrAfter(d(1, 1)))
    }

    @Test fun `biweekly salary`() {
        val r = Recurrence(d(10, 2), Frequency.WEEKLY, 2)
        assertEquals(d(10, 16), r.after(d(10, 2)))
        assertEquals(listOf(d(10, 2), d(10, 16), d(10, 30)), r.between(d(10, 1), d(10, 31)))
    }

    @Test fun `yearly on a leap day lands on Feb 28 then back`() {
        val r = Recurrence(LocalDate.of(2028, 2, 29), Frequency.YEARLY)
        assertEquals(LocalDate.of(2029, 2, 28), r.occurrence(1))
        assertEquals(LocalDate.of(2032, 2, 29), r.occurrence(4))
    }

    @Test fun `between is capped`() {
        val r = Recurrence(d(1, 1, 2000), Frequency.WEEKLY)
        assertEquals(10, r.between(d(1, 1, 2000), d(1, 1, 2026), limit = 10).size)
    }

    @Test fun `per month factor`() {
        assertEquals(1.0, Recurrence(d(1, 1), Frequency.MONTHLY).perMonthFactor(), 1e-9)
        assertEquals(1.0 / 12, Recurrence(d(1, 1), Frequency.YEARLY).perMonthFactor(), 1e-9)
    }

    // ── Where a saved bill is next due ───────────────────────────────────────

    private val monthlyRent = Recurrence(d(7, 1), Frequency.MONTHLY)

    @Test fun `a new bill starts at its first date from today and never back-fills`() {
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, autoPost = true, active = true, stored = null, today = d(10, 4)))
        assertEquals("An anchor ahead is its own first date", d(7, 1), nextDueOnSave(monthlyRent, true, true, null, today = d(5, 15)))
        assertEquals("Due today starts today", d(10, 1), nextDueOnSave(monthlyRent, true, true, null, today = d(10, 1)))
    }

    @Test fun `a new bill keeps a later start it was given and ignores an earlier one`() {
        // A repeat made from an entry dated today starts after the entry, not on it.
        val weekly = Recurrence(d(10, 4), Frequency.WEEKLY)
        assertEquals(d(10, 11), nextDueOnSave(weekly, true, true, null, today = d(10, 4), notBefore = d(10, 11)))
        assertEquals(d(10, 4), nextDueOnSave(weekly, true, true, null, today = d(10, 4), notBefore = d(9, 1)))
    }

    @Test fun `an unchanged bill that posts itself keeps the stored date, ahead or behind`() {
        // The poster already moved it on: whatever an editor held, the stored date stands.
        val posted = BillSchedule(monthlyRent, next = d(11, 1))
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, true, true, posted, today = d(10, 1)))
        // Still owed: the poster catches these up, so saving must not drop them.
        val behind = BillSchedule(monthlyRent, next = d(9, 1))
        assertEquals(d(9, 1), nextDueOnSave(monthlyRent, true, true, behind, today = d(10, 4)))
    }

    @Test fun `switching a reminder to post itself starts from today, not from its old date`() {
        val reminder = BillSchedule(monthlyRent, next = d(7, 1), autoPost = false)
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, autoPost = true, active = true, stored = reminder, today = d(10, 4)))
        val current = BillSchedule(monthlyRent, next = d(11, 1), autoPost = false)
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, true, true, current, today = d(10, 4)))
    }

    @Test fun `a reminder moves past dates that are over and keeps today's`() {
        val behind = BillSchedule(monthlyRent, next = d(7, 1), autoPost = false)
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, autoPost = false, active = true, stored = behind, today = d(10, 4)))
        val dueToday = BillSchedule(monthlyRent, next = d(10, 1), autoPost = false)
        assertEquals(d(10, 1), nextDueOnSave(monthlyRent, false, true, dueToday, today = d(10, 1)))
    }

    @Test fun `resuming starts from today and staying paused keeps the date`() {
        val paused = BillSchedule(monthlyRent, next = d(8, 1), active = false)
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, autoPost = true, active = true, stored = paused, today = d(10, 4)))
        assertEquals(d(8, 1), nextDueOnSave(monthlyRent, autoPost = true, active = false, stored = paused, today = d(10, 4)))
        val pausedAhead = BillSchedule(monthlyRent, next = d(12, 1), active = false)
        assertEquals(d(12, 1), nextDueOnSave(monthlyRent, true, true, pausedAhead, today = d(10, 4)))
    }

    @Test fun `a rule changed on a day the bill posted starts after that day`() {
        val phone = Recurrence(d(1, 15), Frequency.MONTHLY)
        val stored = BillSchedule(phone, next = d(11, 15))
        val quarterly = phone.copy(interval = 3)
        // Jan 15 every three months lands on Oct 15, today, which already posted.
        assertEquals(d(1, 15, 2027), nextDueOnSave(quarterly, true, true, stored, today = d(10, 15), lastPosted = d(10, 15)))
        // 273 days from Jan 15 is exactly 39 weeks: weekly would land on today too.
        val weekly = phone.copy(frequency = Frequency.WEEKLY)
        assertEquals(d(10, 22), nextDueOnSave(weekly, true, true, stored, today = d(10, 15), lastPosted = d(10, 15)))
        // On a day it did not post, a changed rule may start today.
        assertEquals(d(10, 15), nextDueOnSave(quarterly, true, true, stored, today = d(10, 15), lastPosted = d(9, 15)))
    }

    @Test fun `switching to post itself never lands on a date already posted`() {
        val reminder = BillSchedule(monthlyRent, next = d(10, 1), autoPost = false)
        assertEquals(d(11, 1), nextDueOnSave(monthlyRent, true, true, reminder, today = d(10, 1), lastPosted = d(10, 1)))
    }
}
