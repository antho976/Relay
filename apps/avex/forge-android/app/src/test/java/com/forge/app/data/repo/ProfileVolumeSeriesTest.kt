package com.forge.app.data.repo

import com.forge.app.data.db.entities.Session
import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneId

/** Covers [cumulativeSessionVolumeLb] — the All-Time profile graph's per-workout data series. */
class ProfileVolumeSeriesTest {

    private val zone = ZoneId.of("UTC")

    private fun ms(y: Int, m: Int, d: Int): Long =
        LocalDate.of(y, m, d).atStartOfDay(zone).toInstant().toEpochMilli()

    private fun session(y: Int, m: Int, d: Int, vol: Double?): Session =
        Session(dayKey = "push", startedAt = ms(y, m, d), totalVolumeLb = vol)

    @Test
    fun emptyWhenNoSessions() {
        assertEquals(emptyList<Double>(), cumulativeSessionVolumeLb(emptyList()))
    }

    @Test
    fun cumulativePerSessionSortedChronologically() {
        // Provided out of order; sorted by startedAt → running total: 100, 150, 190.
        val sessions = listOf(
            session(2026, 3, 5, 40.0),
            session(2026, 1, 10, 100.0),
            session(2026, 2, 2, 50.0)
        )
        assertEquals(listOf(100.0, 150.0, 190.0), cumulativeSessionVolumeLb(sessions))
    }

    @Test
    fun nullVolumeSessionCarriesTheTotalFlat() {
        val sessions = listOf(
            session(2026, 1, 1, 100.0),
            session(2026, 1, 2, null),   // pre-feature / bodyweight session → adds nothing
            session(2026, 1, 3, 25.0)
        )
        assertEquals(listOf(100.0, 100.0, 125.0), cumulativeSessionVolumeLb(sessions))
    }

    @Test
    fun singleSessionIsOnePoint() {
        assertEquals(listOf(80.0), cumulativeSessionVolumeLb(listOf(session(2026, 6, 1, 80.0))))
    }
}
