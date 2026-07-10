package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CardioCompareTest {

    private fun entry(
        id: Long,
        date: Long,
        type: CardioType = CardioType.RUN,
        durationMin: Int = 30,
        distanceKm: Double? = null
    ) = CardioEntry(id = id, date = date, type = type.code, durationMin = durationMin, distanceKm = distanceKm)

    @Test
    fun `rest entries never compare`() {
        val rest = entry(1, 100, type = CardioType.REST, durationMin = 0)
        val run = entry(2, 50, distanceKm = 5.0)
        assertNull(compareCardioSession(rest, listOf(rest, run)))
    }

    @Test
    fun `first session of its type has nothing to compare`() {
        val run = entry(1, 100, distanceKm = 5.0)
        val walk = entry(2, 50, type = CardioType.WALK, distanceKm = 2.0)
        assertNull(compareCardioSession(run, listOf(run, walk)))
    }

    @Test
    fun `compares only within the same type`() {
        val run = entry(1, 300, durationMin = 30, distanceKm = 5.0)          // 6:00 /km
        val fasterWalk = entry(2, 200, type = CardioType.WALK, durationMin = 10, distanceKm = 5.0)
        val slowerRun = entry(3, 100, durationMin = 40, distanceKm = 5.0)    // 8:00 /km
        val cmp = compareCardioSession(run, listOf(run, fasterWalk, slowerRun))!!
        assertTrue(cmp.isPaceBest)
        assertEquals(slowerRun.id, cmp.bestPaceEntry?.id)
        assertEquals(slowerRun.id, cmp.previous?.id)
    }

    @Test
    fun `slower than the best is not a pace best`() {
        val run = entry(1, 300, durationMin = 40, distanceKm = 5.0)          // 8:00 /km
        val fastRun = entry(2, 100, durationMin = 25, distanceKm = 5.0)      // 5:00 /km
        val cmp = compareCardioSession(run, listOf(run, fastRun))!!
        assertFalse(cmp.isPaceBest)
        assertEquals(fastRun.id, cmp.bestPaceEntry?.id)
    }

    @Test
    fun `distance best tracks the longest same-type session`() {
        val run = entry(1, 300, distanceKm = 10.0)
        val shorter = entry(2, 100, distanceKm = 8.0)
        val cmp = compareCardioSession(run, listOf(run, shorter))!!
        assertTrue(cmp.isDistanceBest)
        assertEquals(8.0, cmp.bestOtherDistanceKm!!, 1e-9)
    }

    @Test
    fun `session without pace cannot be a pace best`() {
        val noDistance = entry(1, 300, distanceKm = null)
        val paced = entry(2, 100, durationMin = 30, distanceKm = 5.0)
        val cmp = compareCardioSession(noDistance, listOf(noDistance, paced))!!
        assertFalse(cmp.isPaceBest)
        assertFalse(cmp.isDistanceBest)
    }

    @Test
    fun `previous is the most recent earlier session, not a later one`() {
        val middle = entry(1, 200, distanceKm = 5.0)
        val oldest = entry(2, 100, distanceKm = 5.0)
        val newest = entry(3, 300, distanceKm = 5.0)
        val cmp = compareCardioSession(middle, listOf(middle, oldest, newest))!!
        assertEquals(oldest.id, cmp.previous?.id)
    }

    @Test
    fun `pace helpers convert and format`() {
        assertEquals(360, paceSecPerKm(30, 5.0))
        assertNull(paceSecPerKm(0, 5.0))
        assertNull(paceSecPerKm(30, null))
        assertEquals("6:00", formatPaceSec(360))
        assertEquals(579, paceSecPerUnit(30, 5.0, useMiles = true))
        assertEquals(360, paceSecPerUnit(30, 5.0, useMiles = false))
    }

    @Test
    fun `mile pace rounds once from the raw session, not twice through sec-per-km`() {
        // 31 min over 8.3 km: converting distance first and rounding once gives 6:01/mi; rounding
        // sec/km first (round(224) = 224) then scaling gave 6:00/mi. Both the pace chip and the
        // compare read go through paceSecPerUnit now, so they land on the same 361s.
        assertEquals(361, paceSecPerUnit(31, 8.3, useMiles = true))
    }
}
