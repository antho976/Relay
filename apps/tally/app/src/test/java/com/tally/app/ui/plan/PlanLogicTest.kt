package com.tally.app.ui.plan

import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.BudgetRow
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.GoalWithSaved
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RecurringRow
import com.tally.app.data.db.TypeTotal
import com.tally.core.AccountType
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.PaceReading
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class PlanLogicTest {

    private val today = LocalDate.of(2026, 10, 14)
    private val period = BudgetPeriod.containing(today)

    private fun expense(id: Long, name: String, archived: Boolean = false) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.EXPENSE, color = id.toInt() % 12, icon = "dots", archived = archived, sortOrder = id.toInt())

    private fun income(id: Long, name: String) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.INCOME, color = 0, icon = "work", sortOrder = id.toInt())

    private fun bill(
        id: Long,
        amount: Long,
        next: LocalDate,
        type: TxType = TxType.EXPENSE,
        frequency: Frequency = Frequency.MONTHLY,
        interval: Int = 1,
        anchor: LocalDate = next,
        autoPost: Boolean = true,
        active: Boolean = true,
    ) = RecurringRow(
        recurring = RecurringEntity(
            id = id,
            name = "Bill $id",
            type = type,
            amount = amount,
            accountId = 1,
            frequency = frequency,
            interval = interval,
            anchorDate = anchor,
            nextDate = next,
            autoPost = autoPost,
            active = active,
        ),
        accountName = "Chequing",
        toAccountName = null,
        categoryName = null,
        categoryColor = null,
        categoryIcon = null,
    )

    // ── Budgets ──────────────────────────────────────────────────────────────

    @Test fun envelopesLeadWithTheFurthestOverPaceForTheirSize() {
        val elapsed = period.elapsedDays(today)
        val small = BudgetLine(1, "Small", null, null, PaceReading(10_000, 9_000, period.days, elapsed))
        val big = BudgetLine(2, "Big", null, null, PaceReading(100_000, 60_000, period.days, elapsed))
        val under = BudgetLine(3, "Under", null, null, PaceReading(50_000, 1_000, period.days, elapsed))
        assertEquals(listOf(1L, 2L, 3L), sortEnvelopes(listOf(under, big, small)).map { it.categoryId })
    }

    @Test fun unbudgetedSkipsBudgetedIncomeUncategorizedAndZero() {
        val categories = listOf(expense(1, "Groceries"), expense(2, "Dining"), expense(3, "Health"), income(20, "Salary"))
        val spending = listOf(
            CategoryTotal(1, 50_000, 10),
            CategoryTotal(2, 8_000, 3),
            CategoryTotal(3, 12_000, 2),
            CategoryTotal(null, 4_000, 1),
            CategoryTotal(20, 9_000, 1),
            CategoryTotal(99, 1_000, 1),
        )
        val lines = unbudgetedLines(spending, budgeted = setOf(1L), categories = categories)
        assertEquals(listOf(3L, 2L), lines.map { it.categoryId })
        assertEquals(12_000L, lines.first().spent)
        assertEquals(2, lines.first().count)
    }

    @Test fun suggestionPrefersTheBiggestUnbudgetedSpend() {
        val categories = listOf(expense(1, "Groceries"), expense(2, "Dining"))
        val lines = listOf(UnbudgetedLine(2, "Dining", null, null, 8_000, 3))
        assertEquals(2L, suggestedBudgetCategory(lines, categories, emptySet()))
    }

    @Test fun suggestionFallsBackToTheFirstLiveExpenseCategoryWithoutABudget() {
        val categories = listOf(expense(1, "Archived", archived = true), income(20, "Salary"), expense(2, "Groceries"), expense(3, "Dining"))
        assertEquals(3L, suggestedBudgetCategory(emptyList(), categories, setOf(2L)))
        assertNull(suggestedBudgetCategory(emptyList(), listOf(income(20, "Salary")), emptySet()))
    }

    @Test fun budgetsReadingFoldsOverallEnvelopesAndUnbudgeted() {
        val categories = listOf(expense(1, "Groceries"), expense(2, "Dining"), expense(3, "Health"))
        val budgets = listOf(
            BudgetRow(10, 0, 290_000, null, null, null),
            BudgetRow(11, 1, 50_000, "Groceries", 0, "cart"),
            BudgetRow(12, 2, 30_000, "Dining", 6, "dining"),
        )
        val spending = listOf(CategoryTotal(1, 20_000, 5), CategoryTotal(2, 25_000, 7), CategoryTotal(3, 6_000, 1))
        val reading = budgetsReading(
            period, today, budgets, spending,
            totals = listOf(TypeTotal(TxType.EXPENSE, 51_000), TypeTotal(TxType.INCOME, 215_000)),
            lastTotals = listOf(TypeTotal(TxType.EXPENSE, 270_000)),
            categories = categories,
        )
        assertEquals(51_000L, reading.spent)
        assertEquals(270_000L, reading.lastPeriodSpent)
        assertEquals(290_000L, reading.overall?.budget)
        assertEquals(51_000L, reading.overall?.spent)
        assertEquals(listOf(2L, 1L), reading.envelopes.map { it.categoryId })
        assertEquals(80_000L, reading.budgeted)
        assertEquals(listOf(3L), reading.unbudgeted.map { it.categoryId })
        assertEquals(6_000L, reading.unbudgetedSpent)
        assertEquals(3L, reading.suggestedCategoryId)
    }

    @Test fun noOverallBudgetReadsNull() {
        val reading = budgetsReading(period, today, emptyList(), emptyList(), emptyList(), emptyList(), emptyList())
        assertNull(reading.overall)
        assertTrue(reading.envelopes.isEmpty())
        assertEquals(0L, reading.spent)
    }

    @Test fun averageCountsOnlyMonthsWithSpending() {
        assertNull(averageOnRecord(listOf(0L, 0L, 0L)))
        assertEquals(27_100L, averageOnRecord(listOf(0L, 0L, 27_100L)))
        assertEquals(20_000L, averageOnRecord(listOf(10_000L, 20_000L, 30_000L)))
    }

    @Test fun roundingToTheTen() {
        assertEquals(272_000L, roundUpTo(271_040, 1_000))
        assertEquals(271_000L, roundUpTo(271_000, 1_000))
        assertEquals(0L, roundUpTo(0, 1_000))
        assertEquals(30_000L, roundTo(30_410, 1_000))
        assertEquals(31_000L, roundTo(30_500, 1_000))
        assertEquals(500L, roundUpTo(500, 0))
    }

    @Test fun ordinalsReadAsWords() {
        assertEquals("1st", ordinal(1))
        assertEquals("2nd", ordinal(2))
        assertEquals("3rd", ordinal(3))
        assertEquals("4th", ordinal(4))
        assertEquals("11th", ordinal(11))
        assertEquals("12th", ordinal(12))
        assertEquals("13th", ordinal(13))
        assertEquals("21st", ordinal(21))
        assertEquals("22nd", ordinal(22))
        assertEquals("Resets on the 15th of every month", resetLine(15))
    }

    // ── Bills ────────────────────────────────────────────────────────────────

    @Test fun frequencyLabelsReadAsWords() {
        assertEquals("Weekly", frequencyLabel(Frequency.WEEKLY, 1))
        assertEquals("Every 2 weeks", frequencyLabel(Frequency.WEEKLY, 2))
        assertEquals("Monthly", frequencyLabel(Frequency.MONTHLY, 1))
        assertEquals("Every 3 months", frequencyLabel(Frequency.MONTHLY, 3))
        assertEquals("Yearly", frequencyLabel(Frequency.YEARLY, 1))
        assertEquals("Every 2 years", frequencyLabel(Frequency.YEARLY, 2))
        assertEquals("Every month", everyLabel(Frequency.MONTHLY, 1))
        assertEquals("Every 4 weeks", everyLabel(Frequency.WEEKLY, 4))
    }

    @Test fun perMonthSpreadsAnyRhythmOverAnAverageMonth() {
        assertEquals(145_000L, perMonth(145_000, Frequency.MONTHLY, 1))
        assertEquals(3_920L, perMonth(7_840, Frequency.MONTHLY, 2))
        assertEquals(1_000L, perMonth(12_000, Frequency.YEARLY, 1))
        // 30.44 / 14 weeks of a biweekly pay.
        assertEquals(Math.round(215_000 * 30.44 / 14.0), perMonth(215_000, Frequency.WEEKLY, 2))
        assertEquals(0L, perMonth(0, Frequency.WEEKLY, 1))
    }

    @Test fun billsReadingSplitsActiveFromPausedAndSumsExpensesOnly() {
        val rows = listOf(
            bill(1, 145_000, today.plusDays(17)),
            bill(2, 7_840, today.plusDays(4), interval = 2),
            bill(3, 215_000, today.plusDays(2), type = TxType.INCOME, frequency = Frequency.WEEKLY, interval = 2),
            bill(4, 4_500, today.plusDays(9), autoPost = false),
            bill(5, 5_499, today.minusDays(30), active = false),
            bill(6, 20_000, today.plusDays(3), type = TxType.TRANSFER),
        )
        val r = billsReading(rows, today)
        assertEquals(listOf(3L, 6L, 2L, 4L, 1L), r.active.map { it.id })
        assertEquals(listOf(5L), r.paused.map { it.id })
        assertEquals(145_000L + 3_920L + 4_500L, r.monthlyCost)
        assertEquals(perMonth(215_000, Frequency.WEEKLY, 2), r.monthlyIncome)
        assertEquals(listOf(1L, 4L, 2L).map { "Bill $it" }, r.costShares.map { it.name })
        assertEquals(1, r.reminders)
        // Next 7 days: bill 2 (in 4 days). Next 30: 1, 2, 4.
        assertEquals(DueSum(7_840, 1), r.week)
        assertEquals(DueSum(145_000L + 7_840L + 4_500L, 3), r.month)
        assertEquals(4L, r.active.first { it.id == 2L }.daysUntil)
    }

    @Test fun dueThroughCountsEveryWeeklyPaymentAndOverdueReminders() {
        val weekly = billLine(bill(1, 1_000, today.plusDays(1), frequency = Frequency.WEEKLY), today)
        assertEquals(DueSum(5_000, 5), dueThrough(listOf(weekly), today.plusDays(29)))
        val overdue = billLine(bill(2, 4_500, today.minusDays(3), autoPost = false), today)
        assertEquals(DueSum(4_500, 1), dueThrough(listOf(overdue), today.plusDays(6)))
        val later = billLine(bill(3, 9_999, today.plusDays(40)), today)
        assertEquals(DueSum(), dueThrough(listOf(later), today.plusDays(29)))
    }

    @Test fun billProblemsNameEachMissingField() {
        val blank = BillDraft(anchorDate = today)
        val p = billProblems(blank)
        assertNotNull(p.name)
        assertNotNull(p.amount)
        assertNotNull(p.account)
        assertTrue(p.any)

        val good = BillDraft(name = "Rent", amount = 145_000, accountId = 1, anchorDate = today)
        assertFalse(billProblems(good).any)

        val zero = good.copy(amount = 0)
        assertNotNull(billProblems(zero).amount)

        val transfer = good.copy(type = TxType.TRANSFER)
        assertEquals("Pick the account it goes to", billProblems(transfer).account)
        assertNotNull(billProblems(transfer.copy(toAccountId = 1)).account)
        assertNull(billProblems(transfer.copy(toAccountId = 2)).account)
    }

    @Test fun resolveBillFillsTheDefaultAccountAndDropsWrongKindCategories() {
        val accounts = listOf(
            AccountEntity(id = 1, name = "Old", type = AccountType.CASH, archived = true),
            AccountEntity(id = 2, name = "Chequing", type = AccountType.CHEQUING),
            AccountEntity(id = 3, name = "Visa", type = AccountType.CREDIT),
        )
        val categories = listOf(expense(5, "Utilities"), income(20, "Salary"))
        val fresh = resolveBill(BillDraft(anchorDate = today), accounts, categories, defaultAccountId = 3)
        assertEquals(3L, fresh.accountId)

        val noDefault = resolveBill(BillDraft(anchorDate = today), accounts, categories, defaultAccountId = 1)
        assertEquals(2L, noDefault.accountId)

        val wrongKind = resolveBill(BillDraft(type = TxType.INCOME, categoryId = 5, anchorDate = today), accounts, categories, 0)
        assertNull(wrongKind.categoryId)
        val rightKind = resolveBill(BillDraft(type = TxType.INCOME, categoryId = 20, anchorDate = today), accounts, categories, 0)
        assertEquals(20L, rightKind.categoryId)

        val transfer = resolveBill(BillDraft(type = TxType.TRANSFER, categoryId = 5, anchorDate = today), accounts, categories, 2)
        assertEquals(2L, transfer.accountId)
        assertEquals(3L, transfer.toAccountId)
        assertNull(transfer.categoryId)

        assertEquals(listOf(2L, 3L), billAccounts(accounts, fresh).map { it.id })
        assertEquals(listOf(20L), billCategories(categories, TxType.INCOME, null).map { it.id })
        assertTrue(billCategories(categories, TxType.TRANSFER, null).isEmpty())
    }

    @Test fun nextDatesStartFromTodayForANewRuleAndKeepTheStoredDateOtherwise() {
        val anchor = LocalDate.of(2026, 1, 31)
        val draft = BillDraft(name = "Rent", amount = 1, accountId = 1, anchorDate = anchor)
        assertEquals(
            listOf(LocalDate.of(2026, 10, 31), LocalDate.of(2026, 11, 30), LocalDate.of(2026, 12, 31)),
            nextDates(draft, stored = null, today = today),
        )
        val future = draft.copy(anchorDate = LocalDate.of(2026, 12, 1), frequency = Frequency.WEEKLY, interval = 2)
        assertEquals(
            listOf(LocalDate.of(2026, 12, 1), LocalDate.of(2026, 12, 15), LocalDate.of(2026, 12, 29)),
            nextDates(future, stored = null, today = today),
        )
        val stored = RecurringEntity(
            id = 1, name = "Rent", type = TxType.EXPENSE, amount = 1, accountId = 1,
            frequency = Frequency.MONTHLY, anchorDate = anchor, nextDate = LocalDate.of(2026, 9, 30),
        )
        assertEquals(LocalDate.of(2026, 9, 30), nextDates(draft.copy(id = 1), stored, today).first())
        // A changed rule starts again from today: every two months from Jan 31 next lands on Nov 30.
        assertEquals(LocalDate.of(2026, 11, 30), nextDates(draft.copy(id = 1, interval = 2), stored, today).first())
    }

    // ── Goals ────────────────────────────────────────────────────────────────

    @Test fun monthsUntilRoundsUpAndStopsAtZeroOncePassed() {
        assertEquals(1, monthsUntil(today, today))
        assertEquals(1, monthsUntil(today, LocalDate.of(2026, 11, 14)))
        assertEquals(2, monthsUntil(today, LocalDate.of(2026, 11, 15)))
        assertEquals(7, monthsUntil(today, LocalDate.of(2027, 5, 1)))
        assertEquals(0, monthsUntil(today, LocalDate.of(2026, 10, 13)))
    }

    @Test fun goalsReadingSumsAndFindsTheNextDate() {
        val rows = listOf(
            GoalWithSaved(GoalEntity(id = 1, name = "Lisbon", target = 320_000, targetDate = LocalDate.of(2027, 5, 1), color = 1), 186_000),
            GoalWithSaved(GoalEntity(id = 2, name = "Cushion", target = 500_000, color = 3), 120_000),
            GoalWithSaved(GoalEntity(id = 3, name = "Bike", target = 90_000, targetDate = LocalDate.of(2026, 12, 1), color = 7), 90_000),
        )
        val g = goalsReading(rows, today)
        assertEquals(396_000L, g.saved)
        assertEquals(910_000L, g.target)
        assertEquals(1, g.reached)
        assertEquals(LocalDate.of(2027, 5, 1), g.nextDate)
        assertEquals("Lisbon", g.nextDateName)
        assertEquals(7, g.goals.first().monthsLeft)
        assertNull(g.goals[1].monthsLeft)
        assertTrue(g.goals[2].reached)
        assertEquals(134_000L, g.goals.first().left)
    }

    @Test fun goalPaceAveragesSinceTheFirstContribution() {
        val contributions = listOf(
            ContributionEntity(id = 2, goalId = 1, amount = 40_000, date = today.minusDays(10)),
            ContributionEntity(id = 1, goalId = 1, amount = 80_000, date = LocalDate.of(2026, 7, 14)),
        )
        val pace = goalPace(contributions, saved = 120_000, target = 320_000, today = today)
        assertNotNull(pace)
        pace!!
        assertEquals(3, pace.months)
        assertEquals(40_000L, pace.perMonth)
        assertEquals(today.plusMonths(5), pace.reachBy)
        assertNull(goalPace(emptyList(), 0, 320_000, today))
        assertNull(goalPace(contributions, 400_000, 320_000, today)?.reachBy)
    }

    @Test fun goalProblemsNeedANameAndATarget() {
        assertTrue(goalProblems(GoalDraft()).any)
        assertNotNull(goalProblems(GoalDraft(name = "Trip")).target)
        assertNotNull(goalProblems(GoalDraft(target = 1_000)).name)
        assertFalse(goalProblems(GoalDraft(name = "Trip", target = 1_000)).any)
    }

    // ── Keypad ───────────────────────────────────────────────────────────────

    @Test fun keypadTypesAndDeletes() {
        var input = AmountInput()
        listOf(PadKey.Digit(3), PadKey.Digit(5), PadKey.Decimal, PadKey.Digit(5)).forEach { input = input.pressed(it) }
        assertEquals(3_550L, input.minor)
        input = input.pressed(PadKey.Backspace).pressed(PadKey.Backspace)
        assertEquals(3_500L, input.minor)
        assertEquals("35", input.text)
        assertEquals(null, padRows(showDecimal = false)[3][0])
        assertEquals(PadKey.Decimal, padRows(showDecimal = true)[3][0])
    }
}
