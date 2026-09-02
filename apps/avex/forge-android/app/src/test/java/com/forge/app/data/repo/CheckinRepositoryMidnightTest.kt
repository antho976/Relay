package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.inMemoryForgeDb
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId

/**
 * M-04: a check-in saved across midnight must correct ONE day's row and touch no other.
 *
 * `save` used to read the clock three times: once to find the existing row's id, once for the
 * entity's date key and once for its recorded-at stamp. The clock here hands out the audit's
 * reproduction sequence (23:59:59.999, then 00:00:00.000, then later) so the first read lands on
 * yesterday and every later read on today. With the old code the entity carried yesterday's id
 * under today's date key; REPLACE then deleted BOTH conflicting rows and inserted one.
 */
@RunWith(RobolectricTestRunner::class)
class CheckinRepositoryMidnightTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private val zone: ZoneId = ZoneId.systemDefault()

    private val yesterday: LocalDate = LocalDate.of(2026, 3, 9)
    private val today: LocalDate = yesterday.plusDays(1)

    private val beforeMidnightMs = yesterday.atTime(LocalTime.of(23, 59, 59, 999_000_000)).atZone(zone).toInstant().toEpochMilli()
    private val midnightMs = today.atStartOfDay(zone).toInstant().toEpochMilli()
    private val afterMidnightMs = midnightMs + 250L

    /** The audit's clock: the first sample is just before midnight, every later one is after it. */
    private val samples = ArrayDeque(listOf(beforeMidnightMs, midnightMs, afterMidnightMs))
    private val clock = Clock { samples.removeFirstOrNull() ?: afterMidnightMs }

    private val repo = CheckinRepository(db.checkinDao(), db.injuryRestrictionDao(), clock)

    @After
    fun tearDown() = db.close()

    private suspend fun seed(date: LocalDate, sleep: Int): Long = db.checkinDao().upsert(
        CheckinEntry(
            dateKey = date.toString(),
            sleepQuality = sleep,
            recordedAt = date.atTime(8, 0).atZone(zone).toInstant().toEpochMilli()
        )
    )

    @Test
    fun aSaveStraddlingMidnightKeepsBothDaysRows() = runTest {
        val yesterdayId = seed(yesterday, sleep = 2)
        val todayId = seed(today, sleep = 4)

        repo.save(sleepQuality = 5, soreness = 1)

        val rows = db.checkinDao().all()
        assertEquals("neither day's row may be deleted or merged", 2, rows.size)

        val yesterdayRow = db.checkinDao().forDate(yesterday.toString())
        assertNotNull("yesterday's row survives", yesterdayRow)
        assertEquals(yesterdayId, yesterdayRow!!.id)
        assertEquals("the sampled instant was still yesterday, so yesterday's row is the one corrected", 5, yesterdayRow.sleepQuality)
        assertEquals("and it is stamped with that same instant", beforeMidnightMs, yesterdayRow.recordedAt)

        val todayRow = db.checkinDao().forDate(today.toString())
        assertNotNull("today's row survives untouched", todayRow)
        assertEquals(todayId, todayRow!!.id)
        assertEquals(4, todayRow.sleepQuality)
    }

    @Test
    fun aSaveAfterMidnightNeverMovesYesterdaysRow() = runTest {
        // Burn the pre-midnight sample so the save's single read is the first post-midnight one.
        samples.removeFirst()
        val yesterdayId = seed(yesterday, sleep = 2)

        repo.save(sleepQuality = 3)

        val rows = db.checkinDao().all()
        assertEquals("a new day gets a new row", 2, rows.size)
        val yesterdayRow = db.checkinDao().forDate(yesterday.toString())!!
        assertEquals("yesterday keeps its id", yesterdayId, yesterdayRow.id)
        assertEquals("and its answers", 2, yesterdayRow.sleepQuality)
        val todayRow = db.checkinDao().forDate(today.toString())!!
        assertEquals(3, todayRow.sleepQuality)
        assertEquals(midnightMs, todayRow.recordedAt)
    }

    @Test
    fun savingTwiceOnOneDayCorrectsTheSameRow() = runTest {
        samples.clear() // every sample is after midnight: one day, two saves
        repo.save(sleepQuality = 1)
        repo.save(sleepQuality = 4, stress = 2)

        val rows = db.checkinDao().all()
        assertEquals(1, rows.size)
        assertEquals(today.toString(), rows.single().dateKey)
        assertEquals(4, rows.single().sleepQuality)
        assertEquals(2, rows.single().stress)
    }
}
