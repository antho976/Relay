package com.forge.app.data.prefs

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.domain.schedule.WeeklySchedule
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * M-21: a weekday assignment is one DataStore edit over the current seven slots, so edits to
 * different weekdays compose however they interleave, and a cancelled edit is whole or absent.
 *
 * The concurrent case runs genuinely parallel callers under `runBlocking`, as
 * [com.forge.app.data.db.SessionWritesConcurrencyTest] does: the old read-modify-write in the
 * ViewModel lost one of two quick picks nearly every time the writes overlapped.
 */
@RunWith(RobolectricTestRunner::class)
class WeeklyScheduleDayWriteTest {

    private lateinit var repo: SettingsRepository

    private val rest: List<String> = List(WeeklySchedule.SLOTS) { "" }

    @Before
    fun setUp() {
        val context: Context = ApplicationProvider.getApplicationContext()
        repo = SettingsRepository(context, Clock { 0L })
    }

    @Test
    fun oneWeekdayChangesAndTheOthersStay() = runBlocking {
        repo.setWeeklySchedule(listOf("push", "", "pull", "", "legs", "", ""))

        repo.setWeeklyScheduleDay(1, "upper")

        assertEquals(listOf("push", "upper", "pull", "", "legs", "", ""), repo.weeklySchedule.first())
    }

    @Test
    fun assigningRestClearsOnlyThatSlot() = runBlocking {
        repo.setWeeklySchedule(listOf("push", "pull", "legs", "", "", "", ""))

        repo.setWeeklyScheduleDay(0, "")

        assertEquals(listOf("", "pull", "legs", "", "", "", ""), repo.weeklySchedule.first())
    }

    @Test
    fun concurrentEditsToDifferentWeekdaysAllLand() = runBlocking {
        repo.setWeeklySchedule(rest)
        val keys = (0 until WeeklySchedule.SLOTS).map { "day$it" }

        withContext(Dispatchers.Default) {
            keys.mapIndexed { weekday, key -> async { repo.setWeeklyScheduleDay(weekday, key) } }.awaitAll()
        }

        assertEquals("no weekday's change may be overwritten by another's", keys, repo.weeklySchedule.first())
    }

    @Test
    fun twoQuickPicksBothSurvive() = runBlocking {
        // The audit's reproduction: Monday then Tuesday before the first write completes.
        repo.setWeeklySchedule(rest)

        withContext(Dispatchers.Default) {
            listOf(
                async { repo.setWeeklyScheduleDay(0, "push") },
                async { repo.setWeeklyScheduleDay(1, "pull") }
            ).awaitAll()
        }

        val schedule = repo.weeklySchedule.first()
        assertEquals("push", schedule[0])
        assertEquals("pull", schedule[1])
    }

    @Test
    fun aCancelledEditIsWholeOrAbsentNeverPartial() = runBlocking {
        repo.setWeeklySchedule(rest)
        val expectedIfLanded = rest.mapIndexed { index, slot -> if (index == 2) "legs" else slot }

        val job = launch(Dispatchers.Default) { repo.setWeeklyScheduleDay(2, "legs") }
        job.cancel()
        job.join()

        val after = repo.weeklySchedule.first()
        assertTrue("got $after", after == rest || after == expectedIfLanded)
    }

    @Test
    fun anIndexOutsideTheWeekIsIgnored() = runBlocking {
        repo.setWeeklySchedule(rest)

        repo.setWeeklyScheduleDay(WeeklySchedule.SLOTS, "push")
        repo.setWeeklyScheduleDay(-1, "push")

        assertEquals(rest, repo.weeklySchedule.first())
    }
}
