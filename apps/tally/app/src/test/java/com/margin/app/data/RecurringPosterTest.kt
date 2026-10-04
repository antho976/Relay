package com.margin.app.data

import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.RecurringEntity
import com.margin.app.data.repo.RecurringPoster
import com.margin.core.AccountType
import com.margin.core.Frequency
import com.margin.core.TxType
import kotlinx.coroutines.async
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

@RunWith(RobolectricTestRunner::class)
class RecurringPosterTest {

    private val jan31 = LocalDate.of(2026, 1, 31)
    private val clock = FixedClock(LocalDate.of(2026, 5, 15))
    private lateinit var db: MarginDatabase
    private lateinit var poster: RecurringPoster
    private var chequing = 0L
    private var savings = 0L
    private var housing = 0L

    @Before fun open() {
        db = RoomTestDb.create()
        poster = db.recurringPoster(clock)
    }

    @After fun close() { db.close() }

    private suspend fun world() {
        chequing = db.addAccount("Chequing")
        savings = db.addAccount("Savings", AccountType.SAVINGS)
        housing = db.addCategory("Housing", icon = "home")
    }

    /** A bill as the database holds it: next due on [next], which a caller may set in the past. */
    private suspend fun bill(
        name: String = "Rent",
        type: TxType = TxType.EXPENSE,
        amount: Long = 1_350_00,
        anchor: LocalDate = jan31,
        next: LocalDate = anchor,
        frequency: Frequency = Frequency.MONTHLY,
        interval: Int = 1,
        endDate: LocalDate? = null,
        categoryId: Long? = housing,
        toAccountId: Long? = null,
        active: Boolean = true,
        autoPost: Boolean = true,
    ): Long = db.recurring().insert(
        RecurringEntity(
            name = name, type = type, amount = amount, accountId = chequing, toAccountId = toAccountId,
            categoryId = categoryId, frequency = frequency, interval = interval, anchorDate = anchor,
            nextDate = next, endDate = endDate, autoPost = autoPost, active = active,
        )
    )

    private suspend fun postedDates(): List<LocalDate> = db.transactions().all().map { it.date }

    @Test fun catchesUpEveryMissedMonthOnTheAnchorDayClampedToShortMonths() = runTest {
        world()
        val rent = bill()

        val posted = poster.postDue()

        assertEquals(4, posted)
        assertEquals(
            listOf(LocalDate.of(2026, 1, 31), LocalDate.of(2026, 2, 28), LocalDate.of(2026, 3, 31), LocalDate.of(2026, 4, 30)),
            postedDates(),
        )
        val entries = db.transactions().all()
        assertTrue(entries.all { it.type == TxType.EXPENSE && it.amount == 1_350_00L })
        assertTrue(entries.all { it.accountId == chequing && it.categoryId == housing && it.toAccountId == null })
        assertTrue(entries.all { it.note == "Rent" && it.recurringId == rent })
        assertTrue(entries.all { it.createdAt == clock.nowMillis() })
        assertEquals("Back on the 31st after April", LocalDate.of(2026, 5, 31), db.recurring().get(rent)?.nextDate)
        assertEquals(true, db.recurring().get(rent)?.active)
    }

    @Test fun postsOnTheDueDayItselfAndAdvancesAsTheDaysPass() = runTest {
        world()
        val rent = bill(anchor = LocalDate.of(2026, 5, 15))

        assertEquals(1, poster.postDue())
        assertEquals(LocalDate.of(2026, 6, 15), db.recurring().get(rent)?.nextDate)

        clock.date = LocalDate.of(2026, 6, 14)
        assertEquals("Not due yet", 0, poster.postDue())

        clock.date = LocalDate.of(2026, 7, 20)
        assertEquals(2, poster.postDue())
        assertEquals(
            listOf(LocalDate.of(2026, 5, 15), LocalDate.of(2026, 6, 15), LocalDate.of(2026, 7, 15)),
            postedDates(),
        )
        assertEquals(LocalDate.of(2026, 8, 15), db.recurring().get(rent)?.nextDate)
    }

    @Test fun runningTwiceNeverPostsADateTwice() = runTest {
        world()
        bill()

        assertEquals(4, poster.postDue())
        assertEquals(0, poster.postDue())

        val dates = postedDates()
        assertEquals(4, dates.size)
        assertEquals(dates.size, dates.toSet().size)
    }

    @Test fun overlappingRunsNeverPostADateTwice() = runTest {
        world()
        bill()
        bill(name = "Gym", amount = 39_00, anchor = LocalDate.of(2026, 3, 13))

        val first = async { poster.postDue() }
        val second = async { poster.postDue() }
        val total = first.await() + second.await()

        // Rent: Jan 31, Feb 28, Mar 31, Apr 30. Gym: Mar 13, Apr 13, May 13.
        assertEquals(7, total)
        val entries = db.transactions().all()
        assertEquals(7, entries.size)
        assertEquals(entries.size, entries.map { it.recurringId to it.date }.toSet().size)
    }

    @Test fun twoPostersOverOneDatabaseStillPostEachDateOnce() = runTest {
        // The per-bill transaction re-reads the bill, so even two instances (no shared mutex) agree.
        world()
        bill()
        val other = db.recurringPoster(clock)

        val first = async { poster.postDue() }
        val second = async { other.postDue() }

        assertEquals(4, first.await() + second.await())
        assertEquals(4, db.transactions().count())
    }

    @Test fun pausedBillsAndRemindersNeverPost() = runTest {
        world()
        val paused = bill(name = "Paused", active = false)
        val reminder = bill(name = "Reminder", autoPost = false)

        assertEquals(0, poster.postDue())

        assertEquals(0, db.transactions().count())
        assertEquals(jan31, db.recurring().get(paused)?.nextDate)
        assertEquals(jan31, db.recurring().get(reminder)?.nextDate)
        assertEquals(false, db.recurring().get(paused)?.active)
        assertEquals(true, db.recurring().get(reminder)?.active)
    }

    @Test fun billEndsOnceItsEndDateHasPassed() = runTest {
        world()
        val ended = bill(endDate = LocalDate.of(2026, 3, 15))
        val endsOnAPostingDay = bill(name = "Lease", endDate = LocalDate.of(2026, 3, 31))
        val running = bill(name = "Phone", amount = 45_00, endDate = LocalDate.of(2026, 12, 31))

        poster.postDue()

        val byBill = db.transactions().all().groupBy({ it.recurringId }, { it.date })
        assertEquals(listOf(LocalDate.of(2026, 1, 31), LocalDate.of(2026, 2, 28)), byBill[ended])
        assertEquals(listOf(LocalDate.of(2026, 1, 31), LocalDate.of(2026, 2, 28), LocalDate.of(2026, 3, 31)), byBill[endsOnAPostingDay])
        assertEquals(4, byBill[running]?.size)
        assertFalse(db.recurring().get(ended)!!.active)
        assertFalse(db.recurring().get(endsOnAPostingDay)!!.active)
        assertTrue(db.recurring().get(running)!!.active)

        clock.date = LocalDate.of(2026, 9, 1)
        poster.postDue()
        assertEquals("An ended bill stays ended", 2, db.transactions().all().count { it.recurringId == ended })
    }

    @Test fun transferBillsPostToTheReceivingAccountWithNoCategory() = runTest {
        world()
        // A category on a transfer bill is a data slip; the ledger entry must not carry it.
        val toSavings = bill(
            name = "To savings", type = TxType.TRANSFER, amount = 250_00, anchor = LocalDate.of(2026, 4, 6),
            categoryId = housing, toAccountId = savings,
        )

        assertEquals(2, poster.postDue())

        val entries = db.transactions().all()
        assertEquals(listOf(LocalDate.of(2026, 4, 6), LocalDate.of(2026, 5, 6)), entries.map { it.date })
        assertTrue(entries.all { it.type == TxType.TRANSFER && it.toAccountId == savings && it.accountId == chequing })
        entries.forEach { assertNull(it.categoryId) }
        assertTrue(entries.all { it.recurringId == toSavings })
    }

    @Test fun anExpenseBillNeverPostsAReceivingAccount() = runTest {
        world()
        bill(anchor = LocalDate.of(2026, 5, 1), toAccountId = savings)

        assertEquals(1, poster.postDue())

        assertNull(db.transactions().all().single().toAccountId)
    }

    @Test fun aLongBacklogIsPaidOverSeveralRunsAndNeverTwice() = runTest {
        world()
        val today = clock.today()
        val weekly = bill(name = "Lessons", amount = 20_00, anchor = today.minusWeeks(100), frequency = Frequency.WEEKLY)

        val firstRun = poster.postDue()
        val secondRun = poster.postDue()
        val thirdRun = poster.postDue()

        assertEquals(RecurringPoster.CATCH_UP_LIMIT, firstRun)
        assertEquals("Weeks 0 through 100 inclusive", 101, firstRun + secondRun)
        assertEquals(0, thirdRun)
        val dates = postedDates()
        assertEquals(101, dates.toSet().size)
        assertEquals(today, dates.last())
        assertEquals(today.plusWeeks(1), db.recurring().get(weekly)?.nextDate)
    }

    @Test fun anIntervalSkipsTheMonthsBetween() = runTest {
        world()
        val quarterly = bill(name = "Insurance", amount = 210_00, anchor = LocalDate.of(2025, 11, 30), interval = 3)

        assertEquals(2, poster.postDue())

        assertEquals(listOf(LocalDate.of(2025, 11, 30), LocalDate.of(2026, 2, 28)), postedDates())
        assertEquals(LocalDate.of(2026, 5, 30), db.recurring().get(quarterly)?.nextDate)
    }
}
