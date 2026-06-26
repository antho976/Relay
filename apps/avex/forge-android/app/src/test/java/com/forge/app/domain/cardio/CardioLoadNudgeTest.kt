package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CardioLoadNudgeTest {

    private val now = 100_000_000_000L
    private val hourMs = 60L * 60 * 1000

    private fun entry(type: String, durationMin: Int, agoHours: Long) =
        CardioEntry(date = now - agoHours * hourMs, type = type, durationMin = durationMin)

    @Test fun `fires for a big recent block`() {
        assertTrue(CardioLoadNudge.recentlyHeavy(listOf(entry("run", 50, 5)), now))
    }

    @Test fun `quiet for a short recent session`() {
        assertFalse(CardioLoadNudge.recentlyHeavy(listOf(entry("walk", 20, 5)), now))
    }

    @Test fun `ignores cardio outside the window`() {
        assertFalse(CardioLoadNudge.recentlyHeavy(listOf(entry("run", 60, 48)), now))
    }

    @Test fun `ignores rest entries`() {
        assertFalse(CardioLoadNudge.recentlyHeavy(listOf(entry("rest", 60, 5)), now))
    }

    @Test fun `sums multiple sessions in the window`() {
        val entries = listOf(entry("walk", 25, 6), entry("cycle", 25, 10))
        assertTrue(CardioLoadNudge.recentlyHeavy(entries, now))
    }

    @Test fun `load names a single activity and totals its minutes`() {
        val load = CardioLoadNudge.recentLoad(listOf(entry("run", 50, 5)), now)!!
        assertEquals(50, load.totalMinutes)
        assertEquals("Run", load.activityLabel)
        assertEquals(now - 5 * hourMs, load.lastSessionMs)
    }

    @Test fun `load labels a mixed block as Cardio and uses the latest session time`() {
        val load = CardioLoadNudge.recentLoad(listOf(entry("walk", 25, 10), entry("cycle", 25, 6)), now)!!
        assertEquals(50, load.totalMinutes)
        assertEquals("Cardio", load.activityLabel)
        assertEquals(now - 6 * hourMs, load.lastSessionMs)
    }

    @Test fun `load is null when under the threshold`() {
        assertNull(CardioLoadNudge.recentLoad(listOf(entry("walk", 20, 5)), now))
    }
}
