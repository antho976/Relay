package com.margin.app.data

import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.db.GoalEntity
import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.RecurringEntity
import com.margin.app.data.db.TransactionEntity
import com.margin.app.data.repo.PlanRepository
import com.margin.core.Frequency
import com.margin.core.TxType
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

@RunWith(RobolectricTestRunner::class)
class PlanRepositoryTest {

    private val today = LocalDate.of(2026, 5, 15)
    private val clock = FixedClock(today)
    private lateinit var db: MarginDatabase
    private lateinit var plan: PlanRepository
    private var chequing = 0L

    @Before fun open() {
        db = RoomTestDb.create()
        plan = db.planRepository(clock)
    }

    @After fun close() { db.close() }

    /** A bill as the editor hands it over: nextDate is whatever the draft held. */
    private fun draft(
        anchor: LocalDate,
        frequency: Frequency = Frequency.MONTHLY,
        interval: Int = 1,
        name: String = "Rent",
    ) = RecurringEntity(
        name = name, type = TxType.EXPENSE, amount = 1_350_00, accountId = chequing,
        frequency = frequency, interval = interval, anchorDate = anchor, nextDate = anchor,
    )

    // ── Recurring ────────────────────────────────────────────────────────────

    @Test fun aNewBillStartsAtItsFirstOccurrenceFromTodayAndNeverBackFills() = runTest {
        chequing = db.addAccount("Chequing")

        val id = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 1, 31), name = "  Rent "))

        val saved = plan.recurringItem(id)!!
        assertEquals(LocalDate.of(2026, 5, 31), saved.nextDate)
        assertEquals("The anchor is kept, so the 31st survives short months", LocalDate.of(2026, 1, 31), saved.anchorDate)
        assertEquals("Rent", saved.name)
        assertEquals(0, db.transactions().count())
        assertEquals("Nothing is owed until the 31st", 0, db.recurringPoster(clock).postDue())
    }

    @Test fun aNewBillDueTodayStartsToday() = runTest {
        chequing = db.addAccount("Chequing")

        val id = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 3, 15)))

        assertEquals(today, plan.recurringItem(id)?.nextDate)
    }

    @Test fun aNewBillAnchoredInTheFutureStartsOnItsAnchor() = runTest {
        chequing = db.addAccount("Chequing")
        val anchor = LocalDate.of(2026, 7, 2)

        val id = plan.saveRecurring(draft(anchor = anchor, frequency = Frequency.WEEKLY, interval = 2))

        assertEquals(anchor, plan.recurringItem(id)?.nextDate)
    }

    @Test fun editingABillKeepsItsScheduleUnlessTheRuleChanges() = runTest {
        chequing = db.addAccount("Chequing")
        val id = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 1, 31)))
        val saved = plan.recurringItem(id)!!

        // An amount change keeps the next date as it stands.
        plan.saveRecurring(saved.copy(amount = 1_400_00, nextDate = LocalDate.of(2026, 6, 30)))
        assertEquals(LocalDate.of(2026, 6, 30), plan.recurringItem(id)?.nextDate)
        assertEquals(1_400_00L, plan.recurringItem(id)?.amount)

        // A new anchor re-anchors from today.
        plan.saveRecurring(plan.recurringItem(id)!!.copy(anchorDate = LocalDate.of(2026, 2, 1)))
        assertEquals(LocalDate.of(2026, 6, 1), plan.recurringItem(id)?.nextDate)

        // So does a new frequency.
        plan.saveRecurring(plan.recurringItem(id)!!.copy(frequency = Frequency.WEEKLY))
        assertEquals(LocalDate.of(2026, 5, 17), plan.recurringItem(id)?.nextDate)
        assertEquals(1, db.recurring().all().size)
    }

    @Test fun resumingAPausedBillStartsFromTodayAndOwesNothingForThePause() = runTest {
        chequing = db.addAccount("Chequing")
        val id = db.recurring().insert(
            draft(anchor = LocalDate.of(2026, 1, 31)).copy(nextDate = LocalDate.of(2026, 2, 28), active = false)
        )

        plan.setRecurringActive(id, true)

        val resumed = plan.recurringItem(id)!!
        assertTrue(resumed.active)
        assertEquals(LocalDate.of(2026, 5, 31), resumed.nextDate)
        assertEquals(0, db.recurringPoster(clock).postDue())
        assertEquals(0, db.transactions().count())
    }

    @Test fun pausingKeepsTheNextDate() = runTest {
        chequing = db.addAccount("Chequing")
        val id = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 1, 31)))

        plan.setRecurringActive(id, false)

        val paused = plan.recurringItem(id)!!
        assertEquals(false, paused.active)
        assertEquals(LocalDate.of(2026, 5, 31), paused.nextDate)
    }

    @Test fun deleteRecurringHandsTheBillBackAndRestorePutsItBackExactly() = runTest {
        chequing = db.addAccount("Chequing")
        val id = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 1, 31)))
        val original = plan.recurringItem(id)!!
        val posted = db.transactions().insert(
            TransactionEntity(type = TxType.EXPENSE, amount = 1_350_00, date = today, accountId = chequing, note = "Rent", recurringId = id)
        )

        val deleted = plan.deleteRecurring(id)
        assertEquals(original, deleted)
        assertNull(plan.recurringItem(id))
        assertEquals("Entries it posted stay", 1, db.transactions().count())
        assertNull("and are unlinked", db.transactions().get(posted)?.recurringId)

        plan.restoreRecurring(deleted!!)
        assertEquals(original, plan.recurringItem(id))
        assertNull(plan.deleteRecurring(404))
    }

    @Test fun recurringRowsCarryTheirNamesActiveFirst() = runTest {
        chequing = db.addAccount("Chequing")
        val housing = db.addCategory("Housing", icon = "home", color = 9)
        val rent = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 1, 31)).copy(categoryId = housing))
        val gym = plan.saveRecurring(draft(anchor = LocalDate.of(2026, 3, 20), name = "Gym"))
        plan.setRecurringActive(rent, false)

        val rows = plan.recurring().first()
        assertEquals(listOf(gym, rent), rows.map { it.recurring.id })
        assertEquals("Chequing", rows.first().accountName)
        assertEquals("Housing", rows.last().categoryName)
        assertEquals("home", rows.last().categoryIcon)
        assertEquals(9, rows.last().categoryColor)
    }

    // ── Goals ────────────────────────────────────────────────────────────────

    @Test fun deleteGoalAndRestoreGoalBringBackItsContributions() = runTest {
        val goal = plan.saveGoal(GoalEntity(name = " Lisbon trip ", target = 2_400_00, targetDate = LocalDate.of(2026, 11, 1), color = 6))
        plan.contribute(goal, 400_00, note = " First ")
        plan.contribute(goal, 300_00)
        plan.contribute(goal, -50_00, note = "Took some back")
        val before = plan.goals().first().single()
        val contributionsBefore = plan.contributions(goal).first()
        assertEquals("Lisbon trip", before.goal.name)
        assertEquals(650_00L, before.saved)

        val deleted = plan.deleteGoal(goal)!!
        assertEquals(3, deleted.contributions.size)
        assertTrue(plan.goals().first().isEmpty())
        assertTrue("Contributions go with their goal", db.goals().allContributions().isEmpty())

        plan.restoreGoal(deleted)
        assertEquals(before, plan.goals().first().single())
        assertEquals(contributionsBefore, plan.contributions(goal).first())
        assertNull(plan.deleteGoal(404))
    }

    @Test fun contributeIsDatedTodayAndOneCanBeUndone() = runTest {
        val goal = plan.saveGoal(GoalEntity(name = "Emergency fund", target = 10_000_00))
        val id = plan.contribute(goal, 250_00, note = "  Payday ")

        val row = db.goals().contribution(id)!!
        assertEquals(today, row.date)
        assertEquals("Payday", row.note)

        val deleted = plan.deleteContribution(id)
        assertEquals(row, deleted)
        assertEquals(0L, plan.goals().first().single().saved)

        plan.restoreContribution(deleted!!)
        assertEquals(listOf(row), plan.contributions(goal).first())
        assertEquals(250_00L, plan.goals().first().single().saved)
    }

    // ── Budgets ──────────────────────────────────────────────────────────────

    @Test fun setBudgetUpdatesInPlaceAndZeroRemovesIt() = runTest {
        val food = db.addCategory("Groceries")

        plan.setBudget(food, 500_00)
        val first = plan.budgetFor(food)!!
        assertEquals(500_00L, first.amount)

        plan.setBudget(food, 450_00)
        assertEquals(450_00L, plan.budgetFor(food)?.amount)
        assertEquals("Same row, changed in place", first.id, plan.budgetFor(food)?.id)
        assertEquals(1, db.budgets().all().size)

        plan.setBudget(food, 0)
        assertNull(plan.budgetFor(food))
        assertTrue(db.budgets().all().isEmpty())
    }

    @Test fun theOverallBudgetIsCategoryZeroAndANegativeAmountRemovesIt() = runTest {
        val food = db.addCategory("Groceries")
        plan.setBudget(BudgetEntity.OVERALL, 2_900_00)
        plan.setBudget(food, 500_00)

        assertEquals(listOf(BudgetEntity.OVERALL, food), plan.budgets().first().map { it.categoryId })

        plan.setBudget(BudgetEntity.OVERALL, -1)
        assertNull(plan.budgetFor(BudgetEntity.OVERALL))
        assertEquals(500_00L, plan.budgetFor(food)?.amount)
    }
}
