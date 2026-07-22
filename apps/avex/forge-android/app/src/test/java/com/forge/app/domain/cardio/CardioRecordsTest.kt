package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CardioRecordsTest {

    private fun entry(
        id: Long,
        date: Long,
        type: String = CardioType.RUN.code,
        durationMin: Int = 30,
        distanceKm: Double? = null
    ) = CardioEntry(id = id, date = date, type = type, durationMin = durationMin, distanceKm = distanceKm)

    @Test
    fun `picks longest distance and fastest pace per type, possibly different sessions`() {
        val slow = entry(1, 100, durationMin = 30, distanceKm = 5.0)   // 6:00 /km
        val fast = entry(2, 200, durationMin = 25, distanceKm = 5.0)   // 5:00 /km — fastest
        val long = entry(3, 300, durationMin = 60, distanceKm = 10.0)  // longest
        val records = cardioActivityRecords(listOf(slow, fast, long))
        assertEquals(1, records.size)
        val run = records.single()
        assertEquals(CardioType.RUN.code, run.typeCode)
        assertEquals(3, run.sessions)
        assertEquals(3L, run.longestEntry.id)
        assertEquals(2L, run.fastestEntry.id)
    }

    @Test
    fun `rest and distance-less sessions never produce or inflate a record`() {
        val run = entry(1, 100, distanceKm = 5.0)
        val noDistance = entry(2, 200, distanceKm = null)             // dropped
        val zeroDistance = entry(3, 300, distanceKm = 0.0)           // dropped
        val rest = entry(4, 400, type = CardioType.REST.code, durationMin = 0)
        val records = cardioActivityRecords(listOf(run, noDistance, zeroDistance, rest))
        assertEquals(1, records.size)
        assertEquals(1, records.single().sessions)                   // only the real distance run counts
    }

    @Test
    fun `most-logged type sorts first, ties break by type code`() {
        val entries = listOf(
            entry(1, 100, type = CardioType.RUN.code, distanceKm = 5.0),
            entry(2, 110, type = CardioType.RUN.code, distanceKm = 6.0),
            entry(3, 120, type = CardioType.SWIM.code, distanceKm = 1.0),
            entry(4, 130, type = CardioType.CYCLE.code, distanceKm = 20.0)
        )
        val records = cardioActivityRecords(entries)
        assertEquals(listOf(CardioType.RUN.code, CardioType.CYCLE.code, CardioType.SWIM.code), records.map { it.typeCode })
        // Cycle before Swim: same session count (1), tie broken alphabetically by code ("cycle" < "swim").
        assertTrue(records[1].typeCode < records[2].typeCode)
    }

    @Test
    fun `a custom activity code produces its own record, unresolved`() {
        val custom = entry(1, 100, type = "custom_ruck", distanceKm = 8.0)
        val records = cardioActivityRecords(listOf(custom))
        assertEquals("custom_ruck", records.single().typeCode)
    }

    @Test
    fun `no entries yields no records`() {
        assertEquals(emptyList<CardioActivityRecord>(), cardioActivityRecords(emptyList()))
    }
}
