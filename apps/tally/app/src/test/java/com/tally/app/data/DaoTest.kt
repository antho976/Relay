package com.tally.app.data

import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.ImportRefused
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.Invest
import com.tally.core.Registration
import com.tally.core.TextMatch
import com.tally.core.TxType
import com.tally.core.WsKind
import kotlinx.coroutines.flow.first
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

/** The SQL itself: balances, breakdowns, the search, uniqueness and what a delete takes with it. */
@RunWith(RobolectricTestRunner::class)
class DaoTest {

    private lateinit var db: TallyDatabase
    private val day = LocalDate.of(2026, 10, 4)

    @Before fun open() { db = RoomTestDb.create() }

    @After fun close() { db.close() }

    // ── Account balances ─────────────────────────────────────────────────────

    @Test fun balanceIsOpeningPlusIncomeMinusExpensesAndTransfersOutPlusTransfersIn() = runTest {
        val chequing = db.addAccount("Chequing", opening = 100_00, sortOrder = 0)
        val visa = db.addAccount("Visa", AccountType.CREDIT, sortOrder = 1)
        val savings = db.addAccount("Savings", AccountType.SAVINGS, opening = 50_00, sortOrder = 2)
        val untouched = db.addAccount("Cash", AccountType.CASH, opening = 12_00, sortOrder = 3)

        db.addIncome(500_00, day, chequing)
        db.addExpense(30_00, day, chequing)
        db.addExpense(45_00, day, visa)
        db.addTransfer(200_00, day, fromAccountId = chequing, toAccountId = visa)
        db.addTransfer(20_00, day, fromAccountId = savings, toAccountId = chequing)

        val balances = db.accounts().observeBalances().first().associateBy { it.id }
        assertEquals(100_00L + 500_00 - 30_00 - 200_00 + 20_00, balances.getValue(chequing).balance)
        assertEquals(-45_00L + 200_00, balances.getValue(visa).balance)
        assertEquals(50_00L - 20_00, balances.getValue(savings).balance)
        assertEquals("An account with no entries holds its opening balance", 12_00L, balances.getValue(untouched).balance)

        // Entry counts include both sides of a transfer.
        assertEquals(4, balances.getValue(chequing).entryCount)
        assertEquals(2, balances.getValue(visa).entryCount)
        assertEquals(1, balances.getValue(savings).entryCount)
        assertEquals(0, balances.getValue(untouched).entryCount)
    }

    @Test fun aRecordedValueReplacesTheOpeningAndEverythingBeforeIt() = runTest {
        val tfsa = db.addAccount("TFSA", AccountType.INVESTMENT, opening = 1_000_00)
        db.addIncome(100_00, day.minusDays(10), tfsa)
        db.values().insert(AccountValueEntity(accountId = tfsa, date = day.minusDays(5), value = 1_250_00))
        db.addIncome(40_00, day, tfsa)

        val balance = db.accounts().observeBalances().first().single()
        assertEquals("The value, then only what came after it", 1_250_00L + 40_00, balance.balance)
        assertEquals(day.minusDays(5), balance.valuedOn)

        // A newer value wins; one on the same day as an entry already holds it.
        db.values().insert(AccountValueEntity(accountId = tfsa, date = day, value = 1_300_00))
        assertEquals(1_300_00L, db.accounts().observeBalances().first().single().balance)
    }

    @Test fun balancesListActiveAccountsBySortOrderAndArchivedLast() = runTest {
        val archived = db.addAccount("Old card", AccountType.CREDIT, archived = true, sortOrder = 0)
        val second = db.addAccount("Savings", AccountType.SAVINGS, sortOrder = 2)
        val first = db.addAccount("Chequing", sortOrder = 1)

        assertEquals(listOf(first, second, archived), db.accounts().observeBalances().first().map { it.id })
        assertEquals(listOf(first, second, archived), db.accounts().all().map { it.id })
    }

    // ── Breakdowns ───────────────────────────────────────────────────────────

    @Test fun byCategoryGroupsOneTypeAndLeavesTransfersOut() = runTest {
        val chequing = db.addAccount("Chequing")
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Groceries")
        val transport = db.addCategory("Transport")
        val salary = db.addCategory("Salary", CategoryKind.INCOME)

        db.addExpense(10_00, day, visa, food)
        db.addExpense(15_00, day.minusDays(3), visa, food)
        db.addExpense(5_00, day, chequing, transport)
        db.addExpense(7_00, day, chequing, categoryId = null)
        db.addIncome(1_000_00, day, chequing, salary)
        db.addTransfer(99_00, day, fromAccountId = chequing, toAccountId = visa)
        // Outside the window: the end is exclusive.
        db.addExpense(40_00, day.plusDays(1), visa, food)

        val expenses = db.transactions().observeByCategory(TxType.EXPENSE, day.minusDays(7), day.plusDays(1)).first()
            .associateBy { it.categoryId }
        assertEquals(setOf(food, transport, null), expenses.keys)
        assertEquals(25_00L, expenses.getValue(food).total)
        assertEquals(2, expenses.getValue(food).count)
        assertEquals(5_00L, expenses.getValue(transport).total)
        assertEquals("The transfer's null category must not join the uncategorized group", 7_00L, expenses.getValue(null).total)
        assertEquals(1, expenses.getValue(null).count)

        val income = db.transactions().observeByCategory(TxType.INCOME, day.minusDays(7), day.plusDays(1)).first()
        assertEquals(1, income.size)
        assertEquals(salary, income.single().categoryId)
        assertEquals(1_000_00L, income.single().total)
    }

    @Test fun typeTotalsSumIncomeAndExpenseAndNeverTransfers() = runTest {
        val chequing = db.addAccount("Chequing")
        val savings = db.addAccount("Savings", AccountType.SAVINGS)
        db.addExpense(12_00, day, chequing)
        db.addExpense(8_00, day.minusDays(1), chequing)
        db.addIncome(300_00, day, chequing)
        db.addTransfer(250_00, day, fromAccountId = chequing, toAccountId = savings)
        db.addExpense(1_00, day.minusDays(30), chequing)

        val totals = db.transactions().observeTypeTotals(day.minusDays(7), day.plusDays(1)).first().associate { it.type to it.total }
        assertEquals(mapOf(TxType.EXPENSE to 20_00L, TxType.INCOME to 300_00L), totals)
        assertFalse(TxType.TRANSFER in totals)
    }

    @Test fun dailyExpenseSumsEachDayInOrderAndSkipsOtherTypes() = runTest {
        val chequing = db.addAccount("Chequing")
        val savings = db.addAccount("Savings", AccountType.SAVINGS)
        db.addExpense(4_00, day, chequing)
        db.addExpense(6_00, day, chequing)
        db.addExpense(3_00, day.minusDays(2), chequing)
        db.addIncome(500_00, day.minusDays(1), chequing)
        db.addTransfer(50_00, day.minusDays(1), fromAccountId = chequing, toAccountId = savings)
        db.addExpense(9_00, day.plusDays(1), chequing)

        val days = db.transactions().observeDailyExpense(day.minusDays(6), day.plusDays(1)).first()
        assertEquals(listOf(day.minusDays(2), day), days.map { it.date })
        assertEquals(listOf(3_00L, 10_00L), days.map { it.total })
    }

    // ── Search ───────────────────────────────────────────────────────────────

    private data class SearchWorld(
        val chequing: Long,
        val visa: Long,
        val savings: Long,
        val groceries: Long,
        val dining: Long,
        val metro: Long,
        val pho: Long,
        val salary: Long,
        val cardPayment: Long,
        val toSavings: Long,
        val old: Long,
    )

    private suspend fun searchWorld(): SearchWorld {
        val chequing = db.addAccount("Chequing")
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val savings = db.addAccount("Savings", AccountType.SAVINGS)
        val groceries = db.addCategory("Groceries")
        val dining = db.addCategory("Dining")
        val salaryCat = db.addCategory("Salary", CategoryKind.INCOME)
        return SearchWorld(
            chequing = chequing,
            visa = visa,
            savings = savings,
            groceries = groceries,
            dining = dining,
            metro = db.addExpense(68_42, day, visa, groceries, note = "Metro"),
            pho = db.addExpense(23_10, day.minusDays(1), visa, dining, note = "Pho Lien"),
            salary = db.addIncome(2_150_00, day.minusDays(2), chequing, salaryCat, note = "Pay"),
            cardPayment = db.addTransfer(900_00, day.minusDays(3), fromAccountId = chequing, toAccountId = visa, note = "Card payment"),
            toSavings = db.addTransfer(250_00, day.minusDays(4), fromAccountId = chequing, toAccountId = savings, note = "Rainy day"),
            old = db.addExpense(5_00, day.minusDays(40), chequing, groceries, note = "Corner store"),
        )
    }

    private suspend fun search(
        query: String = "",
        type: TxType? = null,
        categoryId: Long? = null,
        accountId: Long? = null,
        start: LocalDate? = null,
        end: LocalDate? = null,
        limit: Int = 400,
    ): List<Long> = db.transactions().search(TextMatch.containsPattern(query), type, categoryId, accountId, start, end, limit).first().map { it.id }

    private suspend fun totals(query: String = "", type: TxType? = null, accountId: Long? = null) =
        db.transactions().searchTotals(TextMatch.containsPattern(query), type, null, accountId, null, null).first()

    private suspend fun largest(query: String = "", type: TxType? = null, accountId: Long? = null) =
        db.transactions().searchLargest(TextMatch.containsPattern(query), type, null, accountId, null, null).first()

    @Test fun searchWithNoFiltersListsEverythingNewestFirst() = runTest {
        val w = searchWorld()
        assertEquals(listOf(w.metro, w.pho, w.salary, w.cardPayment, w.toSavings, w.old), search())
    }

    @Test fun searchQueryMatchesNoteCategoryAndAccountNames() = runTest {
        val w = searchWorld()
        assertEquals("note, any case", listOf(w.metro), search(query = "metro"))
        assertEquals("category name", listOf(w.metro, w.old), search(query = "groc"))
        assertEquals("account name", listOf(w.metro, w.pho), search(query = "visa"))
        assertEquals(emptyList<Long>(), search(query = "nothing like this"))
    }

    @Test fun searchFiltersByTypeAndCategory() = runTest {
        val w = searchWorld()
        assertEquals(listOf(w.cardPayment, w.toSavings), search(type = TxType.TRANSFER))
        assertEquals(listOf(w.salary), search(type = TxType.INCOME))
        assertEquals(listOf(w.pho), search(categoryId = w.dining))
        assertEquals(listOf(w.old), search(type = TxType.EXPENSE, categoryId = w.groceries, accountId = w.chequing))
    }

    @Test fun searchByAccountMatchesBothSidesOfATransfer() = runTest {
        val w = searchWorld()
        assertEquals("Visa receives the card payment", listOf(w.metro, w.pho, w.cardPayment), search(accountId = w.visa))
        assertEquals(listOf(w.toSavings), search(accountId = w.savings))
        assertEquals(listOf(w.salary, w.cardPayment, w.toSavings, w.old), search(accountId = w.chequing))
    }

    @Test fun searchDateRangeIsStartInclusiveEndExclusive() = runTest {
        val w = searchWorld()
        assertEquals(listOf(w.pho, w.salary), search(start = day.minusDays(2), end = day))
        assertEquals(listOf(w.metro, w.pho), search(start = day.minusDays(1)))
        assertEquals(listOf(w.old), search(end = day.minusDays(4)))
    }

    @Test fun searchHonoursItsLimit() = runTest {
        val w = searchWorld()
        assertEquals(listOf(w.metro, w.pho), search(limit = 2))
    }

    @Test fun searchFoldsCaseForAccentedLettersToo() = runTest {
        val chequing = db.addAccount("Compte chèque")
        val school = db.addCategory("École")
        val grocery = db.addExpense(42_10, day, chequing, note = "Épicerie Metro")
        val fees = db.addExpense(300_00, day.minusDays(1), chequing, school, note = "Frais")
        assertEquals("note, lower case finds the capital É", listOf(grocery), search(query = "épicerie"))
        assertEquals(listOf(grocery), search(query = "ÉPICERIE"))
        assertEquals("category name", listOf(fees), search(query = "école"))
        assertEquals("account name", listOf(grocery, fees), search(query = "CHÈQUE"))
    }

    @Test fun searchMatchesWildcardCharactersAsTyped() = runTest {
        val cash = db.addAccount("Cash")
        val sale = db.addExpense(10_00, day, cash, note = "50% off")
        db.addExpense(10_00, day, cash, note = "500 off")
        val star = db.addExpense(10_00, day, cash, note = "a*b")
        db.addExpense(10_00, day, cash, note = "ab")
        assertEquals(listOf(sale), search(query = "50%"))
        assertEquals(listOf(star), search(query = "a*b"))
        assertEquals(emptyList<Long>(), search(query = "a?"))
    }

    @Test fun searchTotalsCountEveryMatchNotTheCappedList() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val other = db.addAccount("Chequing")
        val big = db.addExpense(2_400_00, day.minusDays(500), visa, note = "Old laptop")
        repeat(30) { db.addExpense(10_00, day.minusDays(it.toLong()), visa, note = "Coffee") }
        db.addIncome(50_00, day, visa, note = "Refund")
        db.addExpense(99_00, day, other, note = "Elsewhere")

        assertEquals("the list is capped", 5, search(query = "visa", limit = 5).size)
        val t = totals(query = "visa")
        assertEquals(32, t.count)
        assertEquals(2_400_00L + 30 * 10_00L, t.spent)
        assertEquals(50_00L, t.income)
        assertEquals(31, t.expenses)
        assertEquals(1, t.incomes)
        assertEquals(0, t.transfers)
        assertEquals("30 days of coffee, the refund on one of them, and the laptop", 31, t.activeDays)
        assertEquals("the old purchase past the cap is still the largest", big, largest(query = "visa")?.id)
    }

    @Test fun searchLargestIsOfTheBasisTypeAndNullWhenNothingMatches() = runTest {
        val w = searchWorld()
        assertEquals("spending first, so the salary never reads as the largest spend", w.metro, largest()?.id)
        assertEquals(w.salary, largest(type = TxType.INCOME)?.id)
        assertEquals("only transfers touch Savings", w.toSavings, largest(accountId = w.savings)?.id)
        assertNull(largest(query = "nothing like this"))
        assertEquals(0, totals(query = "nothing like this").count)
        assertEquals(0L, totals(query = "nothing like this").spent)
    }

    // ── Budgets ──────────────────────────────────────────────────────────────

    @Test fun budgetsAreOnePerCategoryWithTheOverallAtZero() = runTest {
        val food = db.addCategory("Groceries", sortOrder = 1)
        db.budgets().upsert(BudgetEntity(categoryId = BudgetEntity.OVERALL, amount = 2_000_00))
        db.budgets().upsert(BudgetEntity(categoryId = food, amount = 500_00))
        val overall = db.budgets().getFor(BudgetEntity.OVERALL)!!

        // Upserting the existing row by its id changes it in place.
        db.budgets().upsert(overall.copy(amount = 2_500_00))
        assertEquals(2_500_00L, db.budgets().getFor(BudgetEntity.OVERALL)?.amount)
        assertEquals(overall.id, db.budgets().getFor(BudgetEntity.OVERALL)?.id)

        // A second row for the same category can never exist: the index is unique. An upsert
        // without the existing id cannot add one (whether Room swallows or throws the conflict),
        // which is why PlanRepository.setBudget looks the id up first.
        runCatching { db.budgets().upsert(BudgetEntity(categoryId = food, amount = 999_00)) }
        db.budgets().insertAll(listOf(BudgetEntity(categoryId = BudgetEntity.OVERALL, amount = 3_000_00)))
        val all = db.budgets().all()
        assertEquals(2, all.size)
        assertEquals(1, all.count { it.categoryId == BudgetEntity.OVERALL })
        assertEquals(1, all.count { it.categoryId == food })

        // The overall budget sorts first and has no category to join.
        val rows = db.budgets().observeAll().first()
        assertEquals(listOf(BudgetEntity.OVERALL, food), rows.map { it.categoryId })
        assertNull(rows.first().categoryName)
        assertEquals("Groceries", rows.last().categoryName)
    }

    // ── Goals ────────────────────────────────────────────────────────────────

    @Test fun goalSavedIsTheSumOfContributionsWithdrawalsIncluded() = runTest {
        val trip = db.goals().insert(GoalEntity(name = "Lisbon trip", target = 2_400_00, targetDate = day.plusMonths(6)))
        val fund = db.goals().insert(GoalEntity(name = "Emergency fund", target = 10_000_00))
        db.goals().insertContribution(ContributionEntity(goalId = trip, amount = 300_00, date = day.minusDays(30)))
        db.goals().insertContribution(ContributionEntity(goalId = trip, amount = 200_00, date = day.minusDays(10)))
        db.goals().insertContribution(ContributionEntity(goalId = trip, amount = -50_00, date = day, note = "Took some back"))

        val goals = db.goals().observeAll().first()
        assertEquals("Dated goals sort before undated ones", listOf(trip, fund), goals.map { it.goal.id })
        assertEquals(450_00L, goals.first { it.goal.id == trip }.saved)
        assertEquals("No contributions reads as zero, not null", 0L, goals.first { it.goal.id == fund }.saved)
        assertEquals("The first contribution starts the pace", day.minusDays(30), goals.first { it.goal.id == trip }.firstDate)
        assertNull("No contributions, no first date", goals.first { it.goal.id == fund }.firstDate)

        val history = db.goals().observeContributions(trip).first()
        assertEquals("Newest first", listOf(day, day.minusDays(10), day.minusDays(30)), history.map { it.date })
    }

    // ── What deletes take with them ──────────────────────────────────────────

    @Test fun deletingAnAccountCascadesEveryEntryThatTouchesIt() = runTest {
        val chequing = db.addAccount("Chequing")
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val cash = db.addAccount("Cash", AccountType.CASH)
        db.addExpense(30_00, day, visa)
        db.addTransfer(200_00, day, fromAccountId = chequing, toAccountId = visa)
        db.addTransfer(40_00, day, fromAccountId = visa, toAccountId = cash)
        val kept = db.addExpense(5_00, day, chequing)
        db.recurring().insert(
            RecurringEntity(
                name = "Card payment", type = TxType.TRANSFER, amount = 900_00, accountId = chequing, toAccountId = visa,
                frequency = Frequency.MONTHLY, anchorDate = day, nextDate = day.plusMonths(1),
            )
        )

        db.accounts().delete(db.accounts().get(visa)!!)

        assertEquals(listOf(kept), db.transactions().all().map { it.id })
        assertTrue("A bill into the deleted account goes too", db.recurring().all().isEmpty())
        assertEquals(listOf(chequing, cash), db.accounts().all().map { it.id })
    }

    @Test fun deletingACategoryKeepsItsEntriesUncategorized() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val dining = db.addCategory("Dining")
        val food = db.addCategory("Groceries")
        val a = db.addExpense(12_00, day, visa, dining)
        val b = db.addExpense(8_00, day, visa, dining)
        val c = db.addExpense(50_00, day, visa, food)
        val bill = db.recurring().insert(
            RecurringEntity(
                name = "Lunch club", type = TxType.EXPENSE, amount = 20_00, accountId = visa, categoryId = dining,
                frequency = Frequency.WEEKLY, anchorDate = day, nextDate = day.plusWeeks(1),
            )
        )

        db.categories().delete(dining)

        assertEquals(3, db.transactions().count())
        assertNull(db.transactions().get(a)?.categoryId)
        assertNull(db.transactions().get(b)?.categoryId)
        assertEquals(food, db.transactions().get(c)?.categoryId)
        assertNull(db.recurring().get(bill)?.categoryId)
    }

    @Test fun deletingABillUnlinksTheEntriesItPosted() = runTest {
        val chequing = db.addAccount("Chequing")
        val bill = db.recurring().insert(
            RecurringEntity(
                name = "Rent", type = TxType.EXPENSE, amount = 1_350_00, accountId = chequing,
                frequency = Frequency.MONTHLY, anchorDate = day, nextDate = day.plusMonths(1),
            )
        )
        val posted = db.transactions().insert(
            TransactionEntity(type = TxType.EXPENSE, amount = 1_350_00, date = day, accountId = chequing, note = "Rent", recurringId = bill)
        )

        db.recurring().delete(bill)

        assertNull(db.transactions().get(posted)?.recurringId)
        assertEquals(1, db.transactions().count())
    }

    // ── Categories ───────────────────────────────────────────────────────────

    @Test fun categoriesWithCountsCountEachCategorysEntries() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Groceries", sortOrder = 0)
        val dining = db.addCategory("Dining", sortOrder = 1)
        val unused = db.addCategory("Gifts", sortOrder = 2)
        val salary = db.addCategory("Salary", CategoryKind.INCOME, sortOrder = 0)
        repeat(3) { db.addExpense(10_00L + it, day.minusDays(it.toLong()), visa, food) }
        db.addExpense(25_00, day, visa, dining)
        db.addIncome(2_000_00, day, visa, salary)
        db.addExpense(1_00, day, visa, categoryId = null)

        val counts = db.categories().observeWithCounts().first()
        assertEquals("Expense kinds first, then by sort order", listOf(food, dining, unused, salary), counts.map { it.category.id })
        assertEquals(listOf(3, 1, 0, 1), counts.map { it.entryCount })
    }

    @Test fun findByNameIgnoresCaseAndRespectsKind() = runTest {
        val food = db.addCategory("Groceries")
        db.addCategory("Refunds", CategoryKind.INCOME)
        assertEquals(food, db.categories().findByName("groceries", CategoryKind.EXPENSE)?.id)
        assertNull(db.categories().findByName("Groceries", CategoryKind.INCOME))
        assertNull(db.categories().findByName("Refunds", CategoryKind.EXPENSE))
    }

    // ── Bills due ────────────────────────────────────────────────────────────

    @Test fun dueListsOnlyActiveAutoPostingBillsOnOrBeforeToday() = runTest {
        val chequing = db.addAccount("Chequing")
        fun bill(name: String, next: LocalDate, active: Boolean = true, autoPost: Boolean = true) = RecurringEntity(
            name = name, type = TxType.EXPENSE, amount = 10_00, accountId = chequing, frequency = Frequency.MONTHLY,
            anchorDate = next, nextDate = next, active = active, autoPost = autoPost,
        )
        val today = db.recurring().insert(bill("Today", day))
        val late = db.recurring().insert(bill("Late", day.minusDays(9)))
        db.recurring().insert(bill("Tomorrow", day.plusDays(1)))
        db.recurring().insert(bill("Paused", day.minusDays(1), active = false))
        db.recurring().insert(bill("Reminder", day.minusDays(1), autoPost = false))

        assertEquals(setOf(today, late), db.recurring().due(day).map { it.id }.toSet())
    }

    // ── Editor helpers ───────────────────────────────────────────────────────

    @Test fun noteSuggestionsAreMostUsedFirstAndRememberTheirCategory() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Groceries")
        val dining = db.addCategory("Dining")
        db.addExpense(60_00, day.minusDays(9), visa, dining, note = "Metro")
        db.addExpense(60_00, day.minusDays(2), visa, food, note = "Metro")
        db.addExpense(55_00, day.minusDays(1), visa, food, note = "Metro")
        db.addExpense(12_00, day, visa, dining, note = "Mezze bar")
        db.addExpense(3_00, day, visa, dining, note = "Bagels")

        assertEquals(listOf("Metro", "Mezze bar"), db.transactions().noteSuggestions(TextMatch.prefixPattern("Me")))
        assertEquals("any case", listOf("Metro", "Mezze bar"), db.transactions().noteSuggestions(TextMatch.prefixPattern("me")))
        db.addExpense(9_00, day, visa, food, note = "Épicerie du coin")
        assertEquals("accented capitals too", listOf("Épicerie du coin"), db.transactions().noteSuggestions(TextMatch.prefixPattern("épi")))
        assertEquals("The most recent use wins", food, db.transactions().lastCategoryForNote("Metro"))
        assertNull(db.transactions().lastCategoryForNote("Never typed"))
    }

    // ── Investments (docs/INVESTMENTS.md) ────────────────────────────────────

    /** Wealthsimple's files are read in Canadian dollars only; settings outlive a test, so it puts them back. */
    private fun withCad(block: suspend (SettingsRepository) -> Unit) = runTest {
        val settings = SettingsRepository(ApplicationProvider.getApplicationContext())
        val was = settings.current().currency
        settings.setCurrency("CAD")
        try {
            block(settings)
        } finally {
            settings.setCurrency(was)
        }
    }

    /**
     * An import's lines go in under uids derived from the file, so the same file twice adds them
     * once, and a line deleted since is not brought back by the next one.
     */
    @Test fun anImportTwiceAddsOnce() = withCad { settings ->
        val invest = db.investRepository(FixedClock(day), settings)

        val preview = invest.preview(WsFiles.HOLDINGS_REPORT)
        assertEquals(WsKind.HOLDINGS, preview.kind)
        assertEquals(LocalDate.of(2026, 5, 8), preview.asOf)
        assertEquals(listOf(3, 3, 0), listOf(preview.holdings, preview.new, preview.duplicates))
        assertNull("No account holds its number yet", preview.accounts.single().accountId)
        assertEquals(3, preview.accounts.single().rows)

        val first = invest.import(WsFiles.HOLDINGS_REPORT, emptyMap(), null)
        assertEquals(listOf(1, 3, 3, 0, 3, 1), listOf(first.accountsCreated, first.securities, first.holdings, first.duplicates, first.prices, first.values))
        val account = db.accounts().all().single()
        assertEquals("ws:DEMO0001CAD", account.uid)
        assertEquals(listOf(AccountType.INVESTMENT, Registration.TFSA, "Wealthsimple", "DEMO0001CAD"), listOf(account.type, account.registration, account.institution, account.externalRef))
        val seen = invest.preview(WsFiles.HOLDINGS_REPORT)
        assertEquals("The account holds the number now", account.id, seen.accounts.single().accountId)
        assertEquals(listOf(0, 3), listOf(seen.new, seen.duplicates))

        val again = invest.import(WsFiles.HOLDINGS_REPORT, emptyMap(), null)
        assertEquals(listOf(0, 0, 0, 3, 0, 0), listOf(again.accountsCreated, again.securities, again.holdings, again.duplicates, again.prices, again.values))
        assertEquals(3, db.invest().holdings().size)
        assertEquals(3, db.invest().prices().size)
        assertEquals(1, db.values().all().size)

        // The activities export names another account; its lines go in once, and a deleted one stays out.
        val lines = invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null).activities
        assertTrue(lines > 0)
        assertEquals(2, db.accounts().count())
        val twice = invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        assertEquals(listOf(0, lines), listOf(twice.activities, twice.duplicates))
        db.invest().deleteActivity(db.invest().activities().first().id)
        val after = invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        assertEquals(listOf(0, lines), listOf(after.activities, after.duplicates))
        assertEquals(lines - 1, db.invest().activities().size)
    }

    /** The report's numbers come back out of the portfolio, and its value is the account's balance on Home. */
    @Test fun anImportedHoldingsReportReadsAsTheReportSays() = withCad { settings ->
        val invest = db.investRepository(FixedClock(day), settings)
        invest.import(WsFiles.HOLDINGS_REPORT, emptyMap(), null)

        val p = invest.portfolio().first()
        assertEquals("2026-05-08", p.asOf)
        assertEquals("AAPL's 1,000 USD at its own book ratio, XEQT, ARKK", 1_633_33L, p.value)
        assertEquals(listOf("AAPL" to 10 * Invest.QTY_SCALE, "XEQT" to 10 * Invest.QTY_SCALE, "ARKK" to Invest.QTY_SCALE), p.holdings.map { it.symbol to it.quantity })
        assertEquals(1_633_33L, db.accounts().observeBalances().first().single().balance)

        // Room is the CRA figure by kind and year; zero takes it away.
        invest.setRoom(Registration.TFSA, 2026, 7_000_00)
        assertEquals(7_000_00L, invest.portfolio().first().room.single { it.registration == Registration.TFSA }.room)
        invest.setRoom(Registration.TFSA, 2026, 0)
        assertNull(invest.portfolio().first().room.single { it.registration == Registration.TFSA }.room)
        assertTrue(db.invest().roomFacts().isEmpty())
    }

    /** A file mapped to an account by hand goes there, and the account keeps the number for the next one. */
    @Test fun aFileGoesIntoTheAccountItIsMappedTo() = withCad { settings ->
        val invest = db.investRepository(FixedClock(day), settings)
        val mine = db.addAccount("My TFSA", AccountType.INVESTMENT)
        val chequing = db.addAccount("Chequing")

        val done = invest.import(WsFiles.ACTIVITIES_EXPORT, mapOf("HQ7XFMC41CAD" to mine), null)
        assertEquals(0, done.accountsCreated)
        assertTrue(db.invest().activities().all { it.accountId == mine })
        val kept = db.accounts().get(mine)!!
        assertEquals(listOf("HQ7XFMC41CAD", "Wealthsimple"), listOf(kept.externalRef, kept.institution))
        assertEquals("It takes the file's registration when it had none", Registration.TFSA, kept.registration)

        val refused = runCatching { invest.import(WsFiles.ACTIVITIES_EXPORT, mapOf("HQ7XFMC41CAD" to chequing), null) }.exceptionOrNull()
        assertEquals("Pick an investment account", refused?.message)
        assertTrue(runCatching { invest.preview("Date,Amount\n2026-01-01,4\n") }.exceptionOrNull() is ImportRefused)
    }
}
