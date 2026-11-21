package com.forge.app.domain.session

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.DayOfWeek
import java.time.LocalDate

class StreakMathTest {
    private val mon = LocalDate.of(2026, 10, 5)
    private fun d(offset: Long) = mon.plusDays(offset)

    @Test fun `back-to-back days make a run`() {
        assertEquals(3, longestBridgedStreak(listOf(d(0), d(1), d(2), d(5)), bridged = { false }))
    }

    @Test fun `planned rest days bridge without counting`() {
        // Mon, Tue, Thu, Fri, next Mon on a plan that rests Wed and the weekend: one run of five.
        val rest = setOf(DayOfWeek.WEDNESDAY, DayOfWeek.SATURDAY, DayOfWeek.SUNDAY)
        val days = listOf(d(0), d(1), d(3), d(4), d(7))
        assertEquals(5, longestBridgedStreak(days) { it.dayOfWeek in rest })
        // Without the bridge the same weeks were a run of two.
        assertEquals(2, longestBridgedStreak(days) { false })
    }

    @Test fun `an unplanned gap still breaks the run`() {
        val days = listOf(d(0), d(1), d(4), d(5))
        assertEquals(2, longestBridgedStreak(days) { it == d(2) })
    }

    @Test fun `duplicates and order don't matter, and nothing is zero`() {
        assertEquals(2, longestBridgedStreak(listOf(d(1), d(0), d(1)), bridged = { false }))
        assertEquals(0, longestBridgedStreak(emptyList(), bridged = { false }))
    }
}
