package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CardioPaceTrendTest {

    private fun entry(
        id: Long,
        date: Long,
        type: String = CardioType.RUN.code,
        durationMin: Int = 30,
        distanceKm: Double? = 5.0
    ) = CardioEntry(id = id, date = date, type = type, durationMin = durationMin, distanceKm = distanceKm)

    @Test
    fun `a type needs at least two paced sessions to form a series`() {
        val single = entry(1, 100)
        assertEquals(emptyList<CardioPaceSeries>(), cardioPaceSeries(listOf(single)))
    }

    @Test
    fun `points run oldest to newest regardless of input order`() {
        val newer = entry(1, 300)
        val older = entry(2, 100)
        val middle = entry(3, 200)
        val series = cardioPaceSeries(listOf(newer, older, middle)).single()
        assertEquals(listOf(100L, 200L, 300L), series.points.map { it.dateMs })
    }

    @Test
    fun `rest and distance-less sessions are excluded from the series`() {
        val run1 = entry(1, 100)
        val run2 = entry(2, 200)
        val noDistance = entry(3, 150, distanceKm = null)
        val rest = entry(4, 175, type = CardioType.REST.code, durationMin = 0, distanceKm = null)
        val series = cardioPaceSeries(listOf(run1, run2, noDistance, rest)).single()
        assertEquals(2, series.points.size)
    }

    @Test
    fun `series are ordered by session count, then type code`() {
        val entries = listOf(
            entry(1, 100, type = CardioType.RUN.code),
            entry(2, 110, type = CardioType.RUN.code),
            entry(3, 120, type = CardioType.RUN.code),
            entry(4, 130, type = CardioType.CYCLE.code, distanceKm = 20.0),
            entry(5, 140, type = CardioType.CYCLE.code, distanceKm = 22.0),
            entry(6, 150, type = CardioType.SWIM.code, distanceKm = 1.0),
            entry(7, 160, type = CardioType.SWIM.code, distanceKm = 1.2)
        )
        val codes = cardioPaceSeries(entries).map { it.typeCode }
        // Run (3 sessions) first; Cycle and Swim tie at 2, broken alphabetically ("cycle" < "swim").
        assertEquals(listOf(CardioType.RUN.code, CardioType.CYCLE.code, CardioType.SWIM.code), codes)
    }

    @Test
    fun `each point carries the raw duration and distance for a single-rounding pace read`() {
        val a = entry(1, 100, durationMin = 30, distanceKm = 5.0)
        val b = entry(2, 200, durationMin = 25, distanceKm = 5.0)
        val pts = cardioPaceSeries(listOf(a, b)).single().points
        assertTrue(pts.all { it.durationMin > 0 && it.distanceKm > 0.0 })
        assertEquals(30, pts.first().durationMin)
        assertEquals(25, pts.last().durationMin)
    }
}
