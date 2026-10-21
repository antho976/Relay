package com.forge.app.ui.gym.history

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneOffset
import java.util.Locale

/** The History redesign's month grouping, title line and type filter. */
class HistoryMonthsTest {

    private val utc = ZoneOffset.UTC
    private fun at(y: Int, m: Int, d: Int) = LocalDate.of(y, m, d).atTime(12, 0).toInstant(utc).toEpochMilli()

    private fun workout(id: Long, ms: Long, prs: Int = 0) =
        HistoryItem.Workout(Session(id = id, dayKey = "push-a", startedAt = ms, finishedAt = ms + 1, prCount = prs))

    private fun cardio(id: Long, ms: Long) = HistoryItem.Cardio(CardioEntry(id = id, date = ms, type = "run", durationMin = 30))

    private val items = listOf(
        workout(1, at(2026, 10, 20), prs = 1),
        cardio(2, at(2026, 10, 16)),
        workout(3, at(2026, 9, 29), prs = 2),
        workout(4, at(2025, 12, 1)),
    )

    @Test
    fun monthsGroupInOrderWithTheirCounts() {
        val months = groupByMonth(items, today = LocalDate.of(2026, 10, 21), zone = utc, locale = Locale.UK)
        assertEquals(listOf("October", "September", "December 2025"), months.map { it.label })
        assertEquals("2 sessions · 1 PR", months[0].meta)
        assertEquals("1 session · 2 PRs", months[1].meta)
        assertEquals("1 session", months[2].meta)
    }

    @Test
    fun metaReadsWhatIsLeftAndSinceWhen() {
        assertEquals("3 workouts and 1 cardio session since Dec 2025", historyMeta(items, utc, Locale.UK))
        assertEquals("0 workouts and 0 cardio sessions", historyMeta(emptyList(), utc, Locale.UK))
    }

    @Test
    fun theTypeFilterNarrowsTheList() {
        val workouts = listOf(
            Session(id = 1, dayKey = "push-a", startedAt = 3, finishedAt = 4, prCount = 1),
            Session(id = 2, dayKey = "pull-a", startedAt = 2, finishedAt = 3)
        )
        val runs = listOf(CardioEntry(id = 9, date = 1, type = "run", durationMin = 20))
        fun ids(kind: HistoryKind) = buildFilteredHistory(workouts, runs, emptyMap(), HistoryFilters(kind = kind)).map { it.key }
        assertEquals(listOf("w1", "w2", "c9"), ids(HistoryKind.ALL))
        assertEquals(listOf("w1", "w2"), ids(HistoryKind.GYM))
        assertEquals(listOf("c9"), ids(HistoryKind.CARDIO))
        assertEquals(listOf("w1"), ids(HistoryKind.PRS))
    }

    @Test
    fun aTinyMoveReadsAsMatched() {
        assertEquals(listOf(0f, 3f, -2f, null), deltaBars(listOf(0.2f, 3f, -2f, null)))
        assertNull(deltaBars(listOf(null)).single())
    }
}
