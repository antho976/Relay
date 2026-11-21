package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RouteMatchingTest {

    private fun min(n: Long) = n * 60_000L
    private fun hours(n: Long) = n * 60 * 60_000L

    @Test
    fun noSessionsYieldsNoMatch() {
        assertNull(bestSessionMatch(entryStartMs = 0, entryDurationMin = 30, sessions = emptyList()))
    }

    @Test
    fun overlappingSessionWins() {
        // Entry 10:00–10:40; two sessions, one overlapping, one far away.
        val entryStart = hours(10)
        val sessions = listOf(
            SessionWindow(index = 0, startMs = hours(6), endMs = hours(6) + min(40)),       // morning, no overlap
            SessionWindow(index = 1, startMs = hours(10) + min(5), endMs = hours(10) + min(45)) // overlaps
        )
        assertEquals(1, bestSessionMatch(entryStart, 40, sessions))
    }

    @Test
    fun mostOverlappingSessionWinsOverMerelyNearestStart() {
        val entryStart = hours(10) // entry 10:00–10:30
        val sessions = listOf(
            // Starts closest to 10:00 but only clips the first minute.
            SessionWindow(index = 0, startMs = hours(10) - min(1), endMs = hours(10) + min(1)),
            // Starts a bit later but overlaps almost the whole entry.
            SessionWindow(index = 1, startMs = hours(10) + min(2), endMs = hours(10) + min(35))
        )
        assertEquals(1, bestSessionMatch(entryStart, 30, sessions))
    }

    @Test
    fun fallsBackToNearestStartWhenNothingOverlaps() {
        val entryStart = hours(10)
        val sessions = listOf(
            SessionWindow(index = 0, startMs = hours(7), endMs = hours(7) + min(30)),   // 3h before start
            SessionWindow(index = 1, startMs = hours(12), endMs = hours(12) + min(30))  // 2h after start (nearer)
        )
        // Entry is 0-min logged (a point); no overlap → nearest start within tolerance → index 1.
        assertEquals(1, bestSessionMatch(entryStart, 0, sessions))
    }

    @Test
    fun rejectsNearestWhenBeyondTolerance() {
        val entryStart = hours(10)
        val sessions = listOf(
            SessionWindow(index = 0, startMs = hours(2), endMs = hours(2) + min(30)) // 8h away, no overlap
        )
        assertNull(bestSessionMatch(entryStart, 30, sessions))
    }
}
