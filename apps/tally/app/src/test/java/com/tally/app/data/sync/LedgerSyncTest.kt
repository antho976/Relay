package com.tally.app.data.sync

import com.tally.app.data.FixedClock
import com.tally.app.data.RoomTestDb
import com.tally.app.data.addAccount
import com.tally.app.data.addCategory
import com.tally.app.data.addExpense
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.ledgerRepository
import com.tally.app.data.planRepository
import com.tally.core.AccountType
import com.tally.core.ActivityType
import com.tally.core.Registration
import com.tally.core.SecurityKind
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

/**
 * The ledger's half of sync: what the triggers keep, what goes to the PC, and how the PC's
 * changes are merged (newest wins, tombstones delete, uids become ids, budgets by category).
 */
@RunWith(RobolectricTestRunner::class)
class LedgerSyncTest {

    private val day = LocalDate.of(2026, 10, 5)
    private val clock = FixedClock(day)
    private lateinit var db: TallyDatabase
    private lateinit var sync: LedgerSync

    /** Far ahead of any stamp a test run makes: the PC's newer edit. */
    private val future = System.currentTimeMillis() + 3_600_000L

    @Before fun open() {
        db = RoomTestDb.create()
        sync = LedgerSync(db)
    }

    @After fun close() { db.close() }

    private fun accountRow(name: String) = buildJsonObject {
        put("name", name)
        put("type", "CHEQUING")
        put("openingBalance", 1_000_00)
        put("archived", false)
        put("sortOrder", 0)
    }

    private fun expenseRow(account: String, amount: Long, category: String? = null) = buildJsonObject {
        put("type", "EXPENSE")
        put("amount", amount)
        put("date", "2026-10-05")
        put("account", account)
        put("toAccount", JsonNull)
        put("category", category)
        put("note", "Metro")
        put("recurring", JsonNull)
        put("createdAt", 100)
    }

    // ── What the database keeps by itself ────────────────────────────────────

    @Test fun everyWriteIsStampedAndAnEditorCannotChangeAUid() = runTest {
        val id = db.accounts().insert(AccountEntity(name = "Chequing", type = AccountType.CHEQUING, updatedAt = 5L))
        val stored = db.accounts().get(id)!!
        assertTrue("An insert is stamped now, whatever the row said", stored.updatedAt > 5L)

        // An editor rebuilds the row from its fields: a fresh default uid, a stale change time.
        db.accounts().update(AccountEntity(id = id, name = "Everyday", type = AccountType.CHEQUING, updatedAt = 5L))
        val edited = db.accounts().get(id)!!
        assertEquals("Everyday", edited.name)
        assertEquals("The uid never changes", stored.uid, edited.uid)
        assertTrue("An edit is newer than what it replaced", edited.updatedAt > stored.updatedAt)
    }

    @Test fun deletesLeaveTombstonesCascadesIncludedAndUndoTakesThemBack() = runTest {
        val ledger = db.ledgerRepository(clock)
        val account = db.addAccount("Visa", AccountType.CREDIT)
        val food = db.addCategory("Food")
        val tx = db.addExpense(12_00, day, account, food)
        val row = db.transactions().get(tx)!!

        val deleted = ledger.delete(tx)!!
        assertEquals(row.uid, db.sync().tombstone("transactions", row.uid)?.uid)

        ledger.restore(deleted)
        assertNull("Undo brings the row back, so it is not deleted on the PC", db.sync().tombstone("transactions", row.uid))
        val back = db.transactions().get(tx)!!
        assertTrue("and it counts as changed again", back.updatedAt >= db.sync().tombstones().maxOfOrNull { it.deletedAt } ?: 0L)

        val accountUid = db.accounts().get(account)!!.uid
        ledger.deleteAccount(account)
        val gone = db.sync().tombstones().map { it.tableName to it.uid }.toSet()
        assertTrue(("accounts" to accountUid) in gone)
        assertTrue("The account's entry went with it, and says so", ("transactions" to row.uid) in gone)
    }

    @Test fun aBudgetsTombstoneNamesItsCategory() = runTest {
        val ledger = db.ledgerRepository(clock)
        val plan = db.planRepository(clock)
        val food = db.addCategory("Food")
        val foodUid = db.categories().get(food)!!.uid
        plan.setBudget(food, 400_00)
        plan.setBudget(BudgetEntity.OVERALL, 2_000_00)
        val overallUid = db.budgets().getFor(0)!!.uid

        ledger.deleteCategory(food, moveTo = null)
        plan.setBudget(BudgetEntity.OVERALL, 0)

        val budgets = sync.collect(since = 0L).filter { it.table == "budgets" }
        assertEquals(2, budgets.size)
        val byCategory = budgets.associate { it.row["category"] to it }
        assertTrue(byCategory.getValue(JsonPrimitive(foodUid)).deleted)
        assertEquals(overallUid, byCategory.getValue(JsonNull).uid)
    }

    // ── What goes to the PC ──────────────────────────────────────────────────

    @Test fun aFirstSyncSendsEverythingWithReferencesAsUids() = runTest {
        val account = db.addAccount("Chequing")
        val food = db.addCategory("Food")
        db.addExpense(42_50, day, account, food, note = "Metro")
        db.planRepository(clock).setBudget(BudgetEntity.OVERALL, 310_00)
        db.addExpense(1_00, day, account).also { db.ledgerRepository(clock).delete(it) }

        val changes = sync.collect(since = null)

        assertEquals(listOf("accounts", "categories", "transactions", "budgets"), changes.map { it.table })
        assertTrue("A first sync replaces the PC: no tombstones", changes.none { it.deleted })
        val tx = changes.single { it.table == "transactions" }
        assertEquals(JsonPrimitive(db.accounts().get(account)!!.uid), tx.row["account"])
        assertEquals(JsonPrimitive(db.categories().get(food)!!.uid), tx.row["category"])
        assertEquals(JsonPrimitive(42_50L), tx.row["amount"])
        assertEquals(JsonPrimitive("2026-10-05"), tx.row["date"])
        assertEquals(JsonNull, changes.single { it.table == "budgets" }.row["category"])
        assertEquals("updated_at travels snake case", true, tx.toJson().containsKey("updated_at"))
    }

    @Test fun laterSyncsSendOnlyWhatChangedSinceTheWatermark() = runTest {
        val account = db.addAccount("Chequing")
        db.addExpense(5_00, day, account)
        val watermark = System.currentTimeMillis() + 1
        Thread.sleep(5)
        val coffee = db.addExpense(4_00, day, account, note = "Coffee")
        db.ledgerRepository(clock).delete(db.addExpense(9_00, day, account))

        val changes = sync.collect(since = watermark)

        assertEquals(2, changes.size)
        assertEquals(db.transactions().get(coffee)!!.uid, changes.first { !it.deleted }.uid)
        assertTrue(changes.any { it.deleted && it.table == "transactions" })
    }

    // ── What comes back from the PC ──────────────────────────────────────────

    @Test fun thePcsRowsArriveWithTheirUidsMappedToLocalIds() = runTest {
        val applied = sync.apply(
            listOf(
                // Out of order on purpose: the entry needs its account first.
                Change("transactions", "t1", future, row = expenseRow("a1", 42_50)),
                Change("accounts", "a1", future, row = accountRow("Chequing")),
            )
        )

        assertEquals(Applied(2, 0), applied)
        val account = db.accounts().all().single()
        assertEquals("a1", account.uid)
        assertEquals("The PC's change time is kept", future, account.updatedAt)
        val tx = db.transactions().all().single()
        assertEquals(account.id, tx.accountId)
        assertEquals("t1", tx.uid)
        assertTrue("Applying is not a local change: nothing to tell the PC", db.sync().tombstones().isEmpty())
    }

    @Test fun theNewestEditWinsAndAnOlderOneIsIgnored() = runTest {
        sync.apply(listOf(Change("accounts", "a1", future, row = accountRow("Chequing"))))

        val older = sync.apply(listOf(Change("accounts", "a1", future - 10, row = accountRow("Older"))))
        val equal = sync.apply(listOf(Change("accounts", "a1", future, row = accountRow("Same time"))))
        assertEquals(Applied(0, 2), Applied(older.applied + equal.applied, older.skipped + equal.skipped))
        assertEquals("Chequing", db.accounts().all().single().name)

        sync.apply(listOf(Change("accounts", "a1", future + 10, row = accountRow("Everyday"))))
        assertEquals("Everyday", db.accounts().all().single().name)

        // An edit here is newer than anything the PC sent before it.
        val local = db.accounts().all().single()
        db.accounts().update(local.copy(name = "Mine"))
        sync.apply(listOf(Change("accounts", "a1", future + 5, row = accountRow("Stale"))))
        assertEquals("Mine", db.accounts().all().single().name)
    }

    @Test fun tombstonesDeleteAndALocalDeleteNewerThanTheEditStands() = runTest {
        sync.apply(
            listOf(
                Change("accounts", "a1", future, row = accountRow("Chequing")),
                Change("transactions", "t1", future, row = expenseRow("a1", 10_00)),
                Change("transactions", "t2", future, row = expenseRow("a1", 20_00)),
            )
        )

        sync.apply(listOf(Change("transactions", "t1", future + 1, deleted = true)))
        assertEquals(listOf("t2"), db.transactions().all().map { it.uid })
        assertNull("The PC sent that delete; it is not sent back", db.sync().tombstone("transactions", "t1"))

        // Deleted here after the PC's last edit of it.
        db.ledgerRepository(clock).delete(db.transactions().all().single().id)
        val deletedAt = db.sync().tombstone("transactions", "t2")!!.deletedAt
        val stale = sync.apply(listOf(Change("transactions", "t2", deletedAt - 1, row = expenseRow("a1", 25_00))))
        assertEquals(Applied(0, 1), stale)
        assertTrue(db.transactions().all().isEmpty())

        // Edited on the PC after the delete here: the edit is newer and brings it back.
        sync.apply(listOf(Change("transactions", "t2", deletedAt + 1, row = expenseRow("a1", 25_00))))
        assertEquals(25_00L, db.transactions().all().single().amount)
        assertNull(db.sync().tombstone("transactions", "t2"))
    }

    @Test fun aRowThatPointsAtSomethingMissingIsSkippedNotFatal() = runTest {
        val applied = sync.apply(
            listOf(
                Change("transactions", "t1", future, row = expenseRow("nowhere", 10_00)),
                Change("accounts", "a1", future, row = accountRow("Chequing")),
                Change("transactions", "t2", future, row = expenseRow("a1", 10_00, category = "unknown")),
            )
        )
        assertEquals(Applied(2, 1), applied)
        assertNull("A category this phone does not have reads as uncategorized", db.transactions().all().single().categoryId)
    }

    @Test fun budgetsMatchByCategoryWhateverTheirUid() = runTest {
        val plan = db.planRepository(clock)
        val food = db.addCategory("Food")
        val foodUid = db.categories().get(food)!!.uid
        plan.setBudget(food, 400_00)
        plan.setBudget(BudgetEntity.OVERALL, 2_000_00)

        sync.apply(
            listOf(
                Change("budgets", "pc-food", future, row = buildJsonObject { put("category", foodUid); put("amount", 450_00) }),
                Change("budgets", "pc-overall", future, row = buildJsonObject { put("category", JsonNull); put("amount", 2_500_00) }),
            )
        )

        val budgets = db.budgets().all().associateBy { it.categoryId }
        assertEquals(2, budgets.size)
        assertEquals(450_00L, budgets.getValue(food).amount)
        assertEquals(2_500_00L, budgets.getValue(0).amount)
        assertEquals("The PC's uid is taken, so both sides name it the same", "pc-food", budgets.getValue(food).uid)

        sync.apply(listOf(Change("budgets", "pc-overall", future + 1, deleted = true, row = buildJsonObject { put("category", JsonNull) })))
        assertNull(db.budgets().getFor(0))
        assertNotNull(db.budgets().getFor(food))
    }

    @Test fun aDeletedAccountOnThePcTakesItsEntriesHereToo() = runTest {
        sync.apply(
            listOf(
                Change("accounts", "a1", future, row = accountRow("Chequing")),
                Change("transactions", "t1", future, row = expenseRow("a1", 10_00)),
            )
        )
        sync.apply(listOf(Change("accounts", "a1", future + 1, deleted = true, row = JsonObject(emptyMap()))))
        assertTrue(db.accounts().all().isEmpty())
        assertTrue(db.transactions().all().isEmpty())
        assertFalse("The cascade's entry is told to the PC, in case it kept it", db.sync().tombstones().none { it.uid == "t1" })
    }

    @Test fun changesReadBackFromTheWire() {
        val c = Change("transactions", "t1", 1234, deleted = false, row = expenseRow("a1", 5))
        assertEquals(c, Change.fromJson(c.toJson()))
        assertNull(Change.fromJson(JsonPrimitive("nonsense")))
    }

    @Test fun aDeleteOutranksTheRowEvenWhenItsStampIsAhead() = runTest {
        sync.apply(listOf(Change("accounts", "a1", future, row = accountRow("Chequing"))))
        db.ledgerRepository(clock).deleteAccount(db.accounts().all().single().id)
        assertEquals("The tombstone is stamped past the row it removes", future + 1, db.sync().tombstone("accounts", "a1")!!.deletedAt)
    }

    @Test fun aRowThatComesBackIsStampedPastItsTombstone() = runTest {
        val ledger = db.ledgerRepository(clock)
        sync.apply(
            listOf(
                Change("accounts", "a1", future, row = accountRow("Chequing")),
                Change("transactions", "t1", future, row = expenseRow("a1", 10_00)),
            )
        )
        val deleted = ledger.delete(db.transactions().all().single().id)!!
        val buried = db.sync().tombstone("transactions", "t1")!!.deletedAt
        ledger.restore(deleted)
        assertTrue(db.transactions().all().single().updatedAt > buried)
        assertNull(db.sync().tombstone("transactions", "t1"))
    }

    @Test fun aBudgetTombstoneWithAnotherUidIsFoundByItsCategory() = runTest {
        val plan = db.planRepository(clock)
        val food = db.addCategory("Food")
        val foodUid = db.categories().get(food)!!.uid
        plan.setBudget(food, 400_00)
        plan.setBudget(BudgetEntity.OVERALL, 2_000_00)

        sync.apply(listOf(Change("budgets", "pc-food", future, deleted = true, row = buildJsonObject { put("category", foodUid) })))
        sync.apply(listOf(Change("budgets", "pc-overall", future, deleted = true, row = buildJsonObject { put("category", JsonNull) })))

        assertTrue(db.budgets().all().isEmpty())
    }

    @Test fun takingThePcsLedgerWholeLeavesNothingOfThisOneBehind() = runTest {
        val account = db.addAccount("Mine")
        db.addExpense(5_00, day, account)
        db.ledgerRepository(clock).delete(db.addExpense(6_00, day, account))

        val applied = sync.replaceWith(
            listOf(
                Change("accounts", "a1", 100, row = accountRow("Theirs")),
                Change("transactions", "t1", 100, row = expenseRow("a1", 7_00)),
                Change("transactions", "t-gone", 100, deleted = true),
            )
        )

        assertEquals(Applied(2, 0), applied)
        assertEquals(listOf("Theirs"), db.accounts().all().map { it.name })
        assertEquals(listOf(100L), db.transactions().all().map { it.updatedAt })
        assertTrue(db.sync().tombstones().isEmpty())
    }

    // ── Investments (docs/INVESTMENTS.md) ────────────────────────────────────

    /** The PC's investment rows, field for field as sync.rs names them, references as uids. */
    private val investmentRows = listOf(
        Change("activities", "imp:1", future, row = buildJsonObject {
            put("account", "ws:A1"); put("security", "sec:XEQT"); put("type", "BUY"); put("date", "2026-08-18")
            put("quantity", 1_000_000_000L); put("amount", 381_20L); put("fee", 0L); put("currency", "CAD")
            put("toAmount", JsonNull); put("toCurrency", JsonNull); put("note", ""); put("source", "WEALTHSIMPLE"); put("createdAt", 100L)
        }),
        Change("activities", "fx-1", future, row = buildJsonObject {
            put("account", "ws:A1"); put("security", JsonNull); put("type", "FX"); put("date", "2026-09-02")
            put("quantity", 0L); put("amount", 32_93L); put("fee", 0L); put("currency", "CAD")
            put("toAmount", 24_33L); put("toCurrency", "USD"); put("note", "At the bank"); put("source", "MANUAL"); put("createdAt", 100L)
        }),
        Change("holdings", "hold:ws:A1:sec:XEQT:2026-05-08", future, row = buildJsonObject {
            put("account", "ws:A1"); put("security", "sec:XEQT"); put("date", "2026-05-08")
            put("quantity", 1_000_000_000L); put("book", 250_00L); put("bookMarket", 250_00L)
        }),
        Change("securities", "sec:XEQT", future, row = buildJsonObject {
            put("symbol", "XEQT"); put("name", "iShares Core Equity ETF Portfolio"); put("currency", "CAD"); put("kind", "ETF"); put("exchange", "TSX")
        }),
        Change("accounts", "ws:A1", future, row = buildJsonObject {
            put("name", "TFSA"); put("type", "INVESTMENT"); put("openingBalance", 0L); put("archived", false); put("sortOrder", 0L)
            put("registration", "TFSA"); put("institution", "Wealthsimple"); put("externalRef", "A1")
        }),
        Change("prices", "px:sec:XEQT:2026-10-01", future, row = buildJsonObject {
            put("security", "sec:XEQT"); put("date", "2026-10-01"); put("price", 4_000_000_000L); put("source", "MANUAL")
        }),
        Change("fx_rates", "fx:USD:CAD:2026-10-01", future, row = buildJsonObject {
            put("base", "USD"); put("quote", "CAD"); put("date", "2026-10-01"); put("rate", 137_125_000L); put("source", "BANK_OF_CANADA")
        }),
        Change("room_facts", "room:TFSA:2026", future, row = buildJsonObject {
            put("registration", "TFSA"); put("year", 2026L); put("amount", 7_000_00L)
        }),
    )

    @Test fun investmentRowsRoundTripFieldForField() = runTest {
        // Out of order on purpose: each row waits for what it refers to.
        assertEquals(Applied(8, 0), sync.apply(investmentRows))

        val account = db.accounts().all().single()
        assertEquals(listOf(Registration.TFSA, "Wealthsimple", "A1"), listOf(account.registration, account.institution, account.externalRef))
        val security = db.invest().security("sec:XEQT")!!
        assertEquals(SecurityKind.ETF, security.kind)
        val holding = db.invest().holdings().single()
        assertEquals(listOf(account.id, security.id), listOf(holding.accountId, holding.securityId))
        assertEquals(LocalDate.of(2026, 5, 8), holding.date)
        val buy = db.invest().activity("imp:1")!!
        assertEquals(listOf(ActivityType.BUY, security.id, 381_20L), listOf(buy.type, buy.securityId, buy.amount))
        assertNull(db.invest().activity("fx-1")!!.securityId)
        assertEquals(LocalDate.of(2026, 10, 1), db.invest().prices().single().date)
        assertEquals(Registration.TFSA, db.invest().roomFacts().single().registration)

        // What goes back is what came: the same fields, ISO dates, references as uids.
        val sent = sync.collect(since = null).associateBy { it.table to it.uid }
        investmentRows.forEach { c ->
            val back = sent.getValue(c.table to c.uid)
            assertEquals("${c.table} ${c.uid}", c.row, back.row)
            assertEquals(c.updatedAt, back.updatedAt)
        }
        assertEquals(
            "In the PC's order, after the values",
            listOf("accounts", "securities", "holdings", "activities", "activities", "prices", "fx_rates", "room_facts"),
            sync.collect(since = null).map { it.table },
        )
    }

    @Test fun anAccountFromBeforeInvestmentsHasNoRegistration() = runTest {
        sync.apply(listOf(Change("accounts", "a1", future, row = accountRow("Chequing"))))
        val account = db.accounts().all().single()
        assertEquals(listOf(null, "", ""), listOf(account.registration, account.institution, account.externalRef))
    }

    @Test fun aSecurityDeletedOnThePcTakesItsHoldingsAndPricesButNotItsActivities() = runTest {
        sync.apply(investmentRows)
        sync.apply(listOf(Change("securities", "sec:XEQT", future + 1, deleted = true)))

        assertTrue(db.invest().securities().isEmpty())
        assertTrue(db.invest().holdings().isEmpty())
        assertTrue(db.invest().prices().isEmpty())
        assertEquals(2, db.invest().activities().size)
        assertNull(db.invest().activity("imp:1")!!.securityId)
        assertEquals(
            "A derived row stays deleted, at the PC's stamp: no news to send back",
            future + 1,
            db.sync().tombstone("securities", "sec:XEQT")?.deletedAt,
        )
    }

    /**
     * A derived row the PC deleted stays deleted here, whether this phone had it or not, as the PC
     * keeps the ones this phone deletes: the next import of the same file must not bring it back.
     * Any other row's delete leaves nothing behind, as before.
     */
    @Test fun aDerivedRowThePcDeletedStaysDeletedHere() = runTest {
        sync.apply(investmentRows)
        sync.apply(
            listOf(
                Change("activities", "imp:1", future + 1, deleted = true),
                Change("activities", "imp:never-here", future + 1, deleted = true),
                Change("activities", "fx-1", future + 1, deleted = true),
            )
        )

        assertTrue(db.invest().activities().isEmpty())
        val kept = listOf("imp:1" to future + 1, "imp:never-here" to future + 1)
        assertEquals(kept, db.sync().tombstones().map { it.uid to it.deletedAt }.sortedBy { it.first })
        sync.pruneTombstones(Long.MAX_VALUE)
        assertEquals("Pruning keeps them", kept, db.sync().tombstones().map { it.uid to it.deletedAt }.sortedBy { it.first })
        assertEquals("An older delete does not move them back", Applied(1, 0), sync.apply(listOf(Change("activities", "imp:1", 5, deleted = true))))
        assertEquals(future + 1, db.sync().tombstone("activities", "imp:1")?.deletedAt)
    }

    /**
     * A deleted line an import wrote keeps its tombstone after the PC has heard of it, so the next
     * import of the same file does not bring it back; any other row's goes once the PC has it.
     */
    @Test fun aDeletedImportedLineKeepsItsTombstone() = runTest {
        sync.apply(investmentRows)
        db.invest().deleteActivity(db.invest().activity("imp:1")!!.id)
        db.invest().deleteActivity(db.invest().activity("fx-1")!!.id)

        sync.pruneTombstones(Long.MAX_VALUE)

        assertEquals(listOf("imp:1"), db.sync().tombstones().map { it.uid })
    }

    @Test fun takingThePcsLedgerWholeKeepsItsDeletedImportedLines() = runTest {
        sync.replaceWith(
            listOf(
                Change("accounts", "a1", 100, row = accountRow("Theirs")),
                Change("activities", "imp:gone", 100, deleted = true),
                Change("transactions", "t-gone", 100, deleted = true),
            )
        )
        assertEquals(listOf("activities" to "imp:gone"), db.sync().tombstones().map { it.tableName to it.uid })
    }
}
