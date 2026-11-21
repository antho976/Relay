package com.forge.app.ui.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDateTime
import java.time.ZoneId

/** Unit tests for the hero's "cardio days this week" count — distinct active days, rest excluded. */
class CardioWeekStatsTest {

    private val zone = ZoneId.of("UTC")

    private fun ms(year: Int, month: Int, day: Int, hour: Int = 12): Long =
        LocalDateTime.of(year, month, day, hour, 0).atZone(zone).toInstant().toEpochMilli()

    private fun entry(dateMs: Long, type: String = CardioType.RUN.code, dur: Int = 30) =
        CardioEntry(date = dateMs, type = type, durationMin = dur)

    @Test fun `no entries is zero days`() {
        assertEquals(0, CardioViewModel.countActiveDays(emptyList(), zone))
    }

    @Test fun `two sessions on the same day count once`() {
        val day = ms(2026, 6, 23)
        val entries = listOf(entry(day, dur = 30), entry(day + 3_600_000, dur = 20))
        assertEquals(1, CardioViewModel.countActiveDays(entries, zone))
    }

    @Test fun `sessions on distinct days count separately`() {
        val entries = listOf(entry(ms(2026, 6, 22)), entry(ms(2026, 6, 23)), entry(ms(2026, 6, 24)))
        assertEquals(3, CardioViewModel.countActiveDays(entries, zone))
    }

    @Test fun `rest days do not count`() {
        val entries = listOf(
            entry(ms(2026, 6, 22), type = CardioType.RUN.code),
            entry(ms(2026, 6, 23), type = CardioType.REST.code, dur = 0)
        )
        assertEquals(1, CardioViewModel.countActiveDays(entries, zone))
    }

    @Test fun `a rest day alongside an active session on the same day still counts that day`() {
        val day = ms(2026, 6, 23)
        val entries = listOf(
            entry(day, type = CardioType.REST.code, dur = 0),
            entry(day + 7_200_000, type = CardioType.WALK.code, dur = 25)
        )
        assertEquals(1, CardioViewModel.countActiveDays(entries, zone))
    }
}
