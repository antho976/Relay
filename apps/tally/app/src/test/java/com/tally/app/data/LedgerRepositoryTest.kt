package com.tally.app.data

import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.repo.CategoryUse
import com.tally.app.data.repo.LedgerFilter
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.OtherAccountChange
import com.tally.app.ui.activity.summary
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.Defaults
import com.tally.core.Frequency
import com.tally.core.Registration
import com.tally.core.TxType
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

@RunWith(RobolectricTestRunner::class)
class LedgerRepositoryTest {

    private val today = LocalDate.of(2026, 10, 4)
    private val clock = FixedClock(today)
    private lateinit var db: TallyDatabase
    private lateinit var ledger: LedgerRepository

    @Before fun open() {
        db = RoomTestDb.create()
        ledger = db.ledgerRepository(clock)
    }

    @After fun close() { db.close() }

    private fun expense(accountId: Long, amount: Long = 12_50, categoryId: Long? = null, note: String = "") =
        TransactionEntity(type = TxType.EXPENSE, amount = amount, date = today, accountId = accountId, categoryId = categoryId, note = note)

    // ── save ─────────────────────────────────────────────────────────────────

    @Test fun saveRejectsZeroAndNegativeAmounts() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)

        val zero = runCatching { ledger.save(expense(visa, amount = 0)) }.exceptionOrNull()
        val negative = runCatching { ledger.save(expense(visa, amount = -5_00)) }.exceptionOrNull()

        assertTrue("zero: $zero", zero is IllegalArgumentException)
        assertTrue("negative: $negative", negative is IllegalArgumentException)
        assertEquals(0, db.transactions().count())
    }

    @Test fun saveRejectsATransferWithoutTwoDifferentAccounts() = runTest {
        val chequing = db.addAccount("Chequing")
        val same = TransactionEntity(type = TxType.TRANSFER, amount = 50_00, date = today, accountId = chequing, toAccountId = chequing)
        val nowhere = same.copy(toAccountId = null)

        assertTrue(runCatching { ledger.save(same) }.exceptionOrNull() is IllegalArgumentException)
        assertTrue(runCatching { ledger.save(nowhere) }.exceptionOrNull() is IllegalArgumentException)
        assertEquals(0, db.transactions().count())
    }

    @Test fun aTransferNeverKeepsACategory() = runTest {
        val chequing = db.addAccount("Chequing")
        val savings = db.addAccount("Savings", AccountType.SAVINGS)
        val food = db.addCategory("Groceries")

        val id = ledger.save(
            TransactionEntity(type = TxType.TRANSFER, amount = 250_00, date = today, accountId = chequing, toAccountId = savings, categoryId = food)
        )

        val saved = db.transactions().get(id)!!
        assertNull(saved.categoryId)
        assertEquals(savings, saved.toAccountId)
    }

    @Test fun anExpenseOrIncomeNeverKeepsAReceivingAccount() = runTest {
        val chequing = db.addAccount("Chequing")
        val savings = db.addAccount("Savings", AccountType.SAVINGS)
        val food = db.addCategory("Groceries")

        val spent = ledger.save(expense(chequing, categoryId = food).copy(toAccountId = savings))
        val earned = ledger.save(
            TransactionEntity(type = TxType.INCOME, amount = 100_00, date = today, accountId = chequing, toAccountId = savings)
        )

        assertNull(db.transactions().get(spent)?.toAccountId)
        assertEquals(food, db.transactions().get(spent)?.categoryId)
        assertNull(db.transactions().get(earned)?.toAccountId)
    }

    @Test fun saveInsertsWithATrimmedNoteAndStampThenUpdatesInPlace() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)

        val id = ledger.save(expense(visa, note = "  Metro  "))
        val inserted = db.transactions().get(id)!!
        assertEquals("Metro", inserted.note)
        assertEquals(clock.nowMillis(), inserted.createdAt)

        val again = ledger.save(inserted.copy(amount = 70_00, note = "Metro, the big shop "))
        assertEquals(id, again)
        assertEquals(1, db.transactions().count())
        assertEquals(70_00L, db.transactions().get(id)?.amount)
        assertEquals("Metro, the big shop", db.transactions().get(id)?.note)
    }

    // ── delete and Undo ──────────────────────────────────────────────────────

    @Test fun deleteHandsBackTheRowAndRestorePutsItBackWithTheSameId() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Groceries")
        ledger.save(expense(visa, amount = 3_00, note = "Before"))
        val id = ledger.save(expense(visa, amount = 68_42, categoryId = food, note = "Metro"))
        val original = db.transactions().get(id)!!

        val deleted = ledger.delete(id)
        assertEquals(original, deleted)
        assertNull(db.transactions().get(id))
        assertEquals(1, db.transactions().count())

        ledger.restore(deleted!!)
        // The same row, uid and all; only its change time moves on, so the paired PC hears of the undo.
        assertEquals(original, db.transactions().get(id)!!.copy(updatedAt = original.updatedAt))
        assertEquals(2, db.transactions().count())
    }

    @Test fun deletingAMissingEntryReturnsNull() = runTest {
        assertNull(ledger.delete(404))
    }

    // ── accounts and categories ──────────────────────────────────────────────

    @Test fun newAccountsAndCategoriesGetTrimmedNamesAndTheNextSortOrder() = runTest {
        val first = ledger.saveAccount(AccountEntity(name = " Chequing ", type = AccountType.CHEQUING))
        val second = ledger.saveAccount(AccountEntity(name = "Visa", type = AccountType.CREDIT))
        assertEquals("Chequing", ledger.account(first)?.name)
        assertEquals(0, ledger.account(first)?.sortOrder)
        assertEquals(1, ledger.account(second)?.sortOrder)

        val food = ledger.saveCategory(CategoryEntity(name = "Groceries ", kind = CategoryKind.EXPENSE, color = 0, icon = "cart"))
        val dining = ledger.saveCategory(CategoryEntity(name = "Dining", kind = CategoryKind.EXPENSE, color = 6, icon = "dining"))
        val salary = ledger.saveCategory(CategoryEntity(name = "Salary", kind = CategoryKind.INCOME, color = 0, icon = "work"))
        assertEquals("Groceries", ledger.category(food)?.name)
        assertEquals(listOf(0, 1), listOf(ledger.category(food)?.sortOrder, ledger.category(dining)?.sortOrder))
        assertEquals("Sort order counts per kind", 0, ledger.category(salary)?.sortOrder)

        val renamed = ledger.saveAccount(ledger.account(second)!!.copy(name = "Visa Infinite "))
        assertEquals(second, renamed)
        assertEquals("Visa Infinite", ledger.account(second)?.name)
    }

    /** The account editor rebuilds a row from its own fields, which do not include what an import set. */
    @Test fun anEditorThatKnowsNothingOfInvestmentsLeavesThemAsTheyWere() = runTest {
        val id = db.accounts().insert(
            AccountEntity(name = "TFSA", type = AccountType.INVESTMENT, registration = Registration.TFSA, institution = "Wealthsimple", externalRef = "HQ7XFMC41CAD")
        )

        ledger.saveAccount(AccountEntity(id = id, name = " My TFSA ", type = AccountType.INVESTMENT))
        val renamed = db.accounts().get(id)!!
        assertEquals("My TFSA", renamed.name)
        assertEquals(listOf(Registration.TFSA, "Wealthsimple", "HQ7XFMC41CAD"), listOf(renamed.registration, renamed.institution, renamed.externalRef))

        ledger.saveAccount(AccountEntity(id = id, name = "Savings", type = AccountType.SAVINGS))
        assertNull("Only an investment account has a registration", db.accounts().get(id)!!.registration)
    }

    @Test fun deleteAccountTakesItsEntriesWithIt() = runTest {
        val chequing = db.addAccount("Chequing")
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        db.addExpense(30_00, today, visa)
        db.addTransfer(200_00, today, fromAccountId = chequing, toAccountId = visa)
        val kept = db.addExpense(5_00, today, chequing)
        assertEquals(2, ledger.entriesForAccount(visa))

        ledger.deleteAccount(visa)

        assertNull(ledger.account(visa))
        assertEquals(listOf(kept), db.transactions().all().map { it.id })
    }

    private suspend fun addBill(name: String, accountId: Long, toAccountId: Long? = null, categoryId: Long? = null): Long =
        db.recurring().insert(
            RecurringEntity(
                name = name,
                type = if (toAccountId != null) TxType.TRANSFER else TxType.EXPENSE,
                amount = 20_00,
                accountId = accountId,
                toAccountId = toAccountId,
                categoryId = categoryId,
                frequency = Frequency.MONTHLY,
                anchorDate = today,
                nextDate = today.plusDays(10),
            )
        )

    /**
     * The sample's case: deleting the card takes its bills (paid from it, and the payment into it)
     * and the card payments from Chequing, which then reads higher. All of it is counted first.
     */
    @Test fun accountUseCountsEntriesBillsAndWhatEachOtherAccountLosesOrGains() = runTest {
        val chequing = db.addAccount("Chequing", sortOrder = 0)
        val visa = db.addAccount("Visa", AccountType.CREDIT, sortOrder = 1)
        val savings = db.addAccount("Savings", AccountType.SAVINGS, sortOrder = 2)
        db.addExpense(30_00, today, visa)
        db.addTransfer(900_00, today.minusDays(40), fromAccountId = chequing, toAccountId = visa)
        db.addTransfer(900_00, today.minusDays(9), fromAccountId = chequing, toAccountId = visa)
        db.addTransfer(50_00, today, fromAccountId = visa, toAccountId = chequing)
        db.addTransfer(10_00, today, fromAccountId = visa, toAccountId = savings)
        db.addTransfer(70_00, today, fromAccountId = chequing, toAccountId = savings)
        addBill("Phone plan", visa)
        addBill("Card payment", chequing, toAccountId = visa)
        addBill("Rent", chequing)

        val use = ledger.accountUse(visa)

        assertEquals(5, use.entries)
        assertEquals(2, use.bills)
        assertEquals(
            listOf(OtherAccountChange("Chequing", 3, 1_750_00), OtherAccountChange("Savings", 1, -10_00)),
            use.others,
        )
        // And it is exactly what the delete does to the accounts that stay.
        val before = ledger.balances().first().associate { it.id to it.balance }
        ledger.deleteAccount(visa)
        val after = ledger.balances().first().associate { it.id to it.balance }
        assertEquals(before.getValue(chequing) + 1_750_00, after.getValue(chequing))
        assertEquals(before.getValue(savings) - 10_00, after.getValue(savings))
        assertEquals(listOf("Rent"), db.recurring().all().map { it.name })
    }

    @Test fun anAccountWithNothingOnItHasAnEmptyUse() = runTest {
        val cash = db.addAccount("Cash", AccountType.CASH)
        db.addAccount("Chequing")

        val use = ledger.accountUse(cash)

        assertTrue(use.isEmpty)
        assertTrue(use.others.isEmpty())
    }

    /** A category with no entries yet but a bill and a budget: both are counted, so the bill is not un-filed silently. */
    @Test fun categoryUseCountsBillsAndTheBudgetNotOnlyEntries() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val streaming = db.addCategory("Streaming")
        addBill("Netflix", visa, categoryId = streaming)
        db.planRepository(clock).setBudget(streaming, 20_00)

        val use = ledger.categoryUse(streaming)

        assertEquals(CategoryUse(entries = 0, bills = 1, budget = 20_00), use)
        assertTrue(use.hasMovable)
    }

    @Test fun deleteCategoryWithMoveToMovesItsBillsToo() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val streaming = db.addCategory("Streaming")
        val subscriptions = db.addCategory("Subscriptions")
        val netflix = addBill("Netflix", visa, categoryId = streaming)

        ledger.deleteCategory(streaming, moveTo = subscriptions)

        assertEquals(subscriptions, db.recurring().get(netflix)?.categoryId)
    }

    @Test fun restoreCategoryPutsTheRowAndItsBudgetBack() = runTest {
        val food = db.addCategory("Groceries", icon = "cart", color = 3)
        val original = ledger.category(food)!!
        db.planRepository(clock).setBudget(food, 500_00)

        ledger.deleteCategory(food, moveTo = null)
        assertNull(ledger.category(food))
        ledger.restoreCategory(original, 500_00)

        assertEquals(original, ledger.category(food)!!.copy(updatedAt = original.updatedAt))
        assertEquals(500_00L, db.planRepository(clock).budgetFor(food)?.amount)
        ledger.deleteCategory(food, moveTo = null)
        ledger.restoreCategory(original, null)
        assertNull("No budget comes back when it had none", db.planRepository(clock).budgetFor(food))
    }

    @Test fun deleteCategoryWithMoveToMovesEntriesAndRemovesItsBudget() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val dining = db.addCategory("Dining")
        val food = db.addCategory("Groceries")
        val a = db.addExpense(12_00, today, visa, dining)
        val b = db.addExpense(8_00, today, visa, dining)
        val c = db.addExpense(50_00, today, visa, food)
        val plan = db.planRepository(clock)
        plan.setBudget(dining, 320_00)
        plan.setBudget(food, 500_00)

        ledger.deleteCategory(dining, moveTo = food)

        assertNull(ledger.category(dining))
        assertEquals(listOf(food, food, food), listOf(a, b, c).map { db.transactions().get(it)?.categoryId })
        assertNull(plan.budgetFor(dining))
        assertEquals("The receiving category's budget stays", 500_00L, plan.budgetFor(food)?.amount)
        assertEquals(3, ledger.entriesForCategory(food))
    }

    @Test fun deleteCategoryWithoutMoveToLeavesEntriesUncategorized() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val dining = db.addCategory("Dining")
        val a = db.addExpense(12_00, today, visa, dining)
        db.planRepository(clock).setBudget(dining, 320_00)

        ledger.deleteCategory(dining, moveTo = null)

        assertEquals(1, db.transactions().count())
        assertNull(db.transactions().get(a)?.categoryId)
        assertTrue(db.budgets().all().isEmpty())
    }

    @Test fun deleteCategoryMovingIntoItselfStillDeletesIt() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val dining = db.addCategory("Dining")
        val a = db.addExpense(12_00, today, visa, dining)

        ledger.deleteCategory(dining, moveTo = dining)

        assertNull(ledger.category(dining))
        assertNotNull(db.transactions().get(a))
        assertNull(db.transactions().get(a)?.categoryId)
    }

    // ── first run ────────────────────────────────────────────────────────────

    @Test fun seedCategoriesIfEmptySeedsTheDefaultsOnce() = runTest {
        ledger.seedCategoriesIfEmpty()
        ledger.seedCategoriesIfEmpty()

        val all = db.categories().all()
        assertEquals(Defaults.categories.size, all.size)
        // all() sorts by kind then sortOrder, which is the seed order (expenses are seeded first).
        assertEquals(Defaults.categories.map { it.name }, all.map { it.name })
        assertEquals(Defaults.categories.map { it.kind }, all.map { it.kind })
        assertEquals(Defaults.categories.map { it.icon }, all.map { it.icon })
        assertEquals(Defaults.categories.map { it.color }, all.map { it.color })
        assertEquals(Defaults.categories.indices.toList(), all.map { it.sortOrder })
    }

    @Test fun seedingLeavesAnOwnersCategoriesAlone() = runTest {
        db.addCategory("My only category")

        ledger.seedCategoriesIfEmpty()

        assertEquals(listOf("My only category"), db.categories().all().map { it.name })
    }

    // ── reads ────────────────────────────────────────────────────────────────

    @Test fun searchTrimsTheQueryAndPassesEveryFilter() = runTest {
        val chequing = db.addAccount("Chequing")
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Groceries")
        val metro = db.addExpense(68_42, today, visa, food, note = "Metro")
        db.addExpense(5_00, today.minusDays(1), chequing, food, note = "Corner store")
        db.addTransfer(900_00, today.minusDays(2), fromAccountId = chequing, toAccountId = visa)

        assertEquals(listOf(metro), ledger.search(LedgerFilter(query = "  metro ")).first().map { it.id })
        assertEquals(
            listOf(metro),
            ledger.search(LedgerFilter(type = TxType.EXPENSE, categoryId = food, accountId = visa, start = today, end = today.plusDays(1)))
                .first().map { it.id },
        )
        assertEquals(1, ledger.search(LedgerFilter(), limit = 1).first().size)
        assertEquals(3, ledger.search(LedgerFilter()).first().size)
    }

    @Test fun theSummaryCountsEveryMatchWhileTheListStopsAtTheCap() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        val n = LedgerRepository.SEARCH_LIMIT + 100
        db.transactions().insertAll(
            (0 until n).map { i ->
                TransactionEntity(type = TxType.EXPENSE, amount = 10_00, date = today.minusDays(i.toLong()), accountId = visa, note = "Coffee")
            }
        )
        db.addExpense(2_400_00, today.minusDays(n.toLong()), visa, note = "Old laptop")
        val filter = LedgerFilter(query = "VISA")

        assertEquals("the list is capped", LedgerRepository.SEARCH_LIMIT, ledger.search(filter).first().size)
        val s = ledger.summary(filter).first()
        assertEquals(n + 1, s.count)
        assertEquals(n * 10_00L + 2_400_00L, s.spent)
        assertEquals(n + 1, s.basisCount)
        assertEquals((n * 10_00L + 2_400_00L) / (n + 1), s.average)
        assertEquals("the oldest entry, far past the cap, is still the largest", "Old laptop", s.largest?.note)
        assertEquals(n + 1, s.activeDays)
    }

    @Test fun noteSuggestionsNeedAPrefix() = runTest {
        val visa = db.addAccount("Visa", AccountType.CREDIT)
        db.addExpense(68_42, today, visa, note = "Metro")

        assertEquals(emptyList<String>(), ledger.noteSuggestions("  "))
        assertEquals(listOf("Metro"), ledger.noteSuggestions(" Me"))
    }
}
