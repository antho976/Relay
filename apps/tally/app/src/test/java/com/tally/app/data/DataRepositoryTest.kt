package com.tally.app.data

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.FxRateEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.prefs.Accent
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.CurrencyPlan
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.core.AccountDto
import com.tally.core.AccountType
import com.tally.core.BackupCodec
import com.tally.core.BackupFile
import com.tally.core.BackupReadResult
import com.tally.core.CategoryKind
import com.tally.core.Defaults
import com.tally.core.Frequency
import com.tally.core.Registration
import com.tally.core.SampleData
import com.tally.core.SecurityDto
import com.tally.core.SecurityKind
import com.tally.core.TransactionDto
import com.tally.core.TxType
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.time.LocalDate

/**
 * Whole-ledger moves: backup, restore, sample, erase and CSV. Files go through the real
 * ContentResolver with file:// uris, which Robolectric serves from disk.
 */
@RunWith(RobolectricTestRunner::class)
class DataRepositoryTest {

    @get:Rule val folder = TemporaryFolder()

    private val today = LocalDate.of(2026, 10, 4)
    private val clock = FixedClock(today)
    private lateinit var context: Context
    private lateinit var db: TallyDatabase
    private lateinit var settings: SettingsRepository
    private lateinit var data: DataRepository

    @Before fun open() {
        context = ApplicationProvider.getApplicationContext()
        db = RoomTestDb.create()
        settings = SettingsRepository(context)
        data = DataRepository(context, db, settings, db.ledgerRepository(clock), clock)
        // DataStore keeps one instance per file for the whole process, so settings outlive a test.
        // Every test starts from the same known values instead of trusting a fresh file.
        runBlocking {
            settings.resetData()
            settings.setCurrency("CAD")
            settings.setAccent(Accent.DEFAULT)
            settings.setMonthStartDay(1)
            settings.setWeekStartsMonday(true)
        }
    }

    @After fun close() { db.close() }

    private fun uriFor(name: String, text: String? = null): Uri {
        val file = File(folder.root, name)
        if (text != null) file.writeText(text)
        return Uri.fromFile(file)
    }

    /** The file with every list in id order, so a comparison is about content, not query order. */
    private fun BackupFile.byId(): BackupFile = copy(
        accounts = accounts.sortedBy { it.id },
        categories = categories.sortedBy { it.id },
        transactions = transactions.sortedBy { it.id },
        budgets = budgets.sortedBy { it.id },
        recurring = recurring.sortedBy { it.id },
        goals = goals.sortedBy { it.id },
        contributions = contributions.sortedBy { it.id },
        values = values.sortedBy { it.id },
        securities = securities.sortedBy { it.id },
        holdings = holdings.sortedBy { it.id },
        activities = activities.sortedBy { it.id },
        prices = prices.sortedBy { it.id },
        fxRates = fxRates.sortedBy { it.id },
        roomFacts = roomFacts.sortedBy { it.id },
    )

    /**
     * [byId] without the month and week start, nor the uids: a snapshot always writes the phone's
     * own, while the sample file leaves them out (a restore makes the uids), so a ledger comparison
     * must not count them.
     */
    private fun BackupFile.ledgerOnly(): BackupFile = byId().copy(
        monthStartDay = null,
        weekStartsMonday = null,
        accounts = accounts.sortedBy { it.id }.map { it.copy(uid = null) },
        securities = securities.sortedBy { it.id }.map { it.copy(uid = null) },
        holdings = holdings.sortedBy { it.id }.map { it.copy(uid = null) },
        activities = activities.sortedBy { it.id }.map { it.copy(uid = null) },
        prices = prices.sortedBy { it.id }.map { it.copy(uid = null) },
        fxRates = fxRates.sortedBy { it.id }.map { it.copy(uid = null) },
        roomFacts = roomFacts.sortedBy { it.id }.map { it.copy(uid = null) },
    )

    /** A small ledger the owner made by hand, to prove a failed move leaves it alone. */
    private suspend fun ownLedger() {
        val chequing = db.addAccount("Chequing", opening = 1_000_00)
        val food = db.addCategory("Groceries", icon = "cart")
        db.addExpense(68_42, today, chequing, food, note = "Metro")
        db.addIncome(2_150_00, today.minusDays(3), chequing, note = "Pay")
        db.planRepository(clock).setBudget(food, 500_00)
    }

    private fun TransactionRow.line(): String =
        listOf(date, type, amount, accountName, toAccountName, categoryName, note).joinToString("|")

    // ── Backup and restore ───────────────────────────────────────────────────

    @Test fun snapshotRestoreSnapshotKeepsEveryRowAndId() = runTest {
        val sample = SampleData.build(today, 2, "EUR")

        val restored = data.restore(sample)
        assertEquals(DataResult.Done("Restored ${sample.transactions.size} entries"), restored)
        val first = data.snapshot()
        assertEquals("The restore wrote exactly what the file says", sample.ledgerOnly(), first.ledgerOnly())
        assertEquals("A file that does not say leaves the month start alone", 1, first.monthStartDay)

        assertTrue(data.restore(first) is DataResult.Done)
        val second = data.snapshot()
        assertEquals(first, second)

        val s = settings.current()
        assertEquals("The backup's currency comes with it", "EUR", s.currency)
        assertTrue(s.onboarded)
        assertFalse(s.sampleLoaded)
        assertEquals(sample.accounts.first().id, s.defaultAccountId)
    }

    @Test fun writeBackupThenReadBackupRoundTripsThroughAFile() = runTest {
        ownLedger()
        val uri = uriFor("tally-backup.json")

        assertEquals(DataResult.Done("Backup saved"), data.writeBackup(uri))
        val read = data.readBackup(uri)

        assertTrue("read: $read", read is BackupReadResult.Ok)
        assertEquals(data.snapshot(), (read as BackupReadResult.Ok).file)
    }

    @Test fun anUnreadableBackupIsRefusedBeforeAnythingIsTouched() = runTest {
        ownLedger()
        val before = data.snapshot()

        val garbage = data.readBackup(uriFor("notes.json", "{ this is not a backup"))
        val missing = data.readBackup(uriFor("gone.json"))

        assertEquals(BackupReadResult.Invalid("This file is not a Tally backup."), garbage)
        assertEquals(BackupReadResult.Invalid("That file could not be opened."), missing)
        assertEquals(before, data.snapshot())
    }

    @Test fun aBackupWithBrokenReferencesFailsValidationAndChangesNothing() = runTest {
        ownLedger()
        val before = data.snapshot()
        val orphan = SampleData.build(today, 2, "CAD").let { file ->
            file.copy(transactions = file.transactions + TransactionDto(9_999, TxType.EXPENSE, 1_00, today.toString(), accountId = 77))
        }

        val read = data.readBackup(uriFor("orphan.json", BackupCodec.encode(orphan)))

        assertEquals(BackupReadResult.Invalid("A transaction points at an account that is not in the file."), read)
        assertEquals(before, data.snapshot())
        assertEquals("CAD", settings.current().currency)
    }

    @Test fun aRestoreThatFailsHalfwayRollsBackToTheOldData() = runTest {
        ownLedger()
        val before = data.snapshot()
        val sample = SampleData.build(today, 2, "EUR")
        // Validation cannot see a repeated id; the database's primary key can.
        val repeated = sample.copy(transactions = sample.transactions + sample.transactions.first())
        assertTrue(BackupCodec.validate(repeated) is BackupReadResult.Ok)

        val result = data.restore(repeated)

        assertEquals(DataResult.Failed("The restore did not finish. Your data is unchanged."), result)
        assertEquals(before, data.snapshot())
        assertEquals("Settings are untouched too", "CAD", settings.current().currency)
    }

    @Test fun aRestoreThatSkippedValidationStillCannotWriteOrphans() = runTest {
        ownLedger()
        val before = data.snapshot()
        val orphan = BackupFile(
            exportedAt = today.toString(),
            currency = "EUR",
            accounts = listOf(AccountDto(1, "Chequing", AccountType.CHEQUING, 0)),
            transactions = listOf(TransactionDto(1, TxType.EXPENSE, 5_00, today.toString(), accountId = 2)),
        )

        assertTrue(data.restore(orphan) is DataResult.Failed)
        assertEquals(before, data.snapshot())
    }

    // ── Sample data ──────────────────────────────────────────────────────────

    @Test fun loadSampleRefusesALedgerThatHasEntries() = runTest {
        ownLedger()
        val before = data.snapshot()

        val result = data.loadSample()

        assertEquals(DataResult.Failed("Sample data loads only into an empty app. Erase everything first to try it."), result)
        assertEquals(before, data.snapshot())
        assertFalse(settings.current().sampleLoaded)
    }

    @Test fun loadSampleFillsAnEmptyLedgerAndSaysSo() = runTest {
        db.ledgerRepository(clock).seedCategoriesIfEmpty()

        assertEquals(DataResult.Done("Sample data loaded"), data.loadSample())

        assertEquals(SampleData.build(today, 2, "CAD").ledgerOnly(), data.snapshot().ledgerOnly())
        assertTrue(db.accounts().all().all { it.name.startsWith(SampleData.ACCOUNT_PREFIX) })
        val s = settings.current()
        assertTrue(s.sampleLoaded)
        assertTrue(s.onboarded)
        assertEquals(1L, s.defaultAccountId)
    }

    // ── Erase ────────────────────────────────────────────────────────────────

    @Test fun eraseAllWipesEverythingAndReseedsTheCategories() = runTest {
        assertTrue(data.loadSample() is DataResult.Done)
        settings.setAccent(Accent.SAGE)

        assertEquals(DataResult.Done("Everything erased"), data.eraseAll())

        assertEquals(0, db.transactions().count())
        assertEquals(0, db.accounts().count())
        assertTrue(db.budgets().all().isEmpty())
        assertTrue(db.recurring().all().isEmpty())
        assertTrue(db.goals().all().isEmpty())
        assertTrue(db.goals().allContributions().isEmpty())
        assertEquals(Defaults.categories.map { it.name }, db.categories().all().map { it.name })

        val s = settings.current()
        assertFalse(s.onboarded)
        assertFalse(s.sampleLoaded)
        assertEquals(0L, s.defaultAccountId)
        assertEquals("Erase keeps the currency", "CAD", s.currency)
        assertEquals("and the look", Accent.SAGE, s.accent)
    }

    // ── CSV ──────────────────────────────────────────────────────────────────

    @Test fun csvImportMatchesOrCreatesAccountsAndCategoriesByNameAndSkipsBadLines() = runTest {
        val chequing = db.addAccount("Chequing")
        val groceries = db.addCategory("Groceries", icon = "cart")
        val csv = listOf(
            "date,type,amount,currency,category,account,to_account,note",
            "2026-10-01,expense,12.50,CAD,groceries,chequing,,Metro",
            "2026-10-02,expense,8.00,CAD,Coffee,Visa,,Cafe",
            "2026-10-03,income,100.00,CAD,Salary,Chequing,,Pay",
            "2026-10-03,transfer,50.00,CAD,,Chequing,Savings,To savings",
            "03/10/2026,expense,1.00,CAD,,Chequing,,Bad date",
            "2026-10-04,expense,abc,CAD,,Chequing,,Bad amount",
            "2026-10-04,transfer,5.00,CAD,,Chequing,,No receiving account",
        ).joinToString("\r\n")

        val result = data.importCsv(uriFor("bank.csv", csv))

        assertEquals(DataResult.Done("Imported 4 entries, skipped 3"), result)

        val accounts = db.accounts().all().associateBy { it.name }
        assertEquals(setOf("Chequing", "Visa", "Savings"), accounts.keys)
        assertEquals("Matched without regard to case", chequing, accounts.getValue("Chequing").id)

        val categories = db.categories().all()
        assertEquals(listOf("Groceries", "Coffee", "Salary"), categories.map { it.name })
        assertEquals(groceries, categories.first { it.name == "Groceries" }.id)
        assertEquals(CategoryKind.EXPENSE, categories.first { it.name == "Coffee" }.kind)
        assertEquals(CategoryKind.INCOME, categories.first { it.name == "Salary" }.kind)

        val rows = db.transactions().allRows()
        assertEquals(
            listOf(
                "2026-10-01|EXPENSE|1250|Chequing|null|Groceries|Metro",
                "2026-10-02|EXPENSE|800|Visa|null|Coffee|Cafe",
                "2026-10-03|INCOME|10000|Chequing|null|Salary|Pay",
                "2026-10-03|TRANSFER|5000|Chequing|Savings|null|To savings",
            ),
            rows.map { it.line() },
        )
        assertTrue(db.transactions().all().all { it.createdAt == clock.nowMillis() })
    }

    @Test fun csvWithNothingReadableFailsAndAddsNothing() = runTest {
        ownLedger()
        val before = data.snapshot()

        val result = data.importCsv(uriFor("bad.csv", "date,amount\r\nyesterday,1.00\r\n"))

        assertEquals(DataResult.Failed("Line 2: the date \"yesterday\" is not in YYYY-MM-DD form."), result)
        assertEquals(before, data.snapshot())
        assertEquals(DataResult.Failed("That file could not be opened."), data.importCsv(uriFor("absent.csv")))
    }

    @Test fun csvExportThenImportGivesBackEveryEntry() = runTest {
        assertTrue(data.loadSample() is DataResult.Done)
        val exported = db.transactions().allRows().map { it.line() }.sorted()
        val uri = uriFor("tally.csv")

        assertEquals(DataResult.Done("Exported ${exported.size} entries"), data.writeCsv(uri))
        assertTrue(data.eraseAll() is DataResult.Done)
        assertEquals(DataResult.Done("Imported ${exported.size} entries"), data.importCsv(uri))

        assertEquals(exported, db.transactions().allRows().map { it.line() }.sorted())
        assertNull("Imported accounts are plain chequing until the owner says otherwise",
            db.accounts().all().firstOrNull { it.type != AccountType.CHEQUING })
    }

    @Test fun csvExportStartsWithAByteOrderMarkSoExcelReadsTheAccents() = runTest {
        val chequing = db.addAccount("Compte chèque")
        db.addExpense(12_50, today, chequing, db.addCategory("Épicerie"), note = "Café dépanneur")
        val uri = uriFor("accents.csv")

        assertTrue(data.writeCsv(uri) is DataResult.Done)

        val bytes = File(folder.root, "accents.csv").readBytes()
        assertEquals(listOf(0xEF, 0xBB, 0xBF), bytes.take(3).map { it.toInt() and 0xFF })
        assertTrue(bytes.toString(Charsets.UTF_8).contains("Café dépanneur"))
    }

    @Test fun csvImportMatchesAnAccentedCategoryInAnyCaseInsteadOfMakingASecondOne() = runTest {
        db.addAccount("Chequing")
        val groceries = db.addCategory("Épicerie")
        val csv = "date,type,amount,category,account,note\r\n2026-10-01,expense,12.50,épicerie,Chequing,Metro\r\n"

        assertTrue(data.importCsv(uriFor("fr.csv", csv)) is DataResult.Done)

        assertEquals(listOf("Épicerie"), db.categories().all().map { it.name })
        assertEquals(groceries, db.transactions().all().single().categoryId)
    }

    @Test fun csvRoundTripKeepsNamesThatLookLikeFormulas() = runTest {
        val plus = db.addAccount("+Savings")
        db.addExpense(5_00, today, plus, note = "@Costco")
        val uri = uriFor("formulas.csv")
        assertTrue(data.writeCsv(uri) is DataResult.Done)
        db.transactions().deleteAll()

        assertTrue(data.importCsv(uri) is DataResult.Done)

        assertEquals("No second account named '+Savings", listOf("+Savings"), db.accounts().all().map { it.name })
        assertEquals("@Costco", db.transactions().all().single().note)
    }

    // ── Settings that ride in the backup ─────────────────────────────────────

    @Test fun aPaydayMonthSurvivesEraseAndRestore() = runTest {
        ownLedger()
        settings.setMonthStartDay(15)
        settings.setWeekStartsMonday(false)
        val backup = data.snapshot()
        assertEquals(15, backup.monthStartDay)
        assertEquals(false, backup.weekStartsMonday)

        assertTrue(data.eraseAll() is DataResult.Done)
        settings.setWeekStartsMonday(true)
        assertEquals("Erase forgets the month start", 1, settings.current().monthStartDay)
        assertTrue(data.restore(backup) is DataResult.Done)

        val s = settings.current()
        assertEquals(15, s.monthStartDay)
        assertFalse(s.weekStartsMonday)
    }

    // ── Currency ─────────────────────────────────────────────────────────────

    /** One of every stored amount: an opening balance, entries, a budget, a bill, a goal and a withdrawal. */
    private suspend fun everyKindOfAmount(): Long {
        ownLedger()
        val chequing = db.accounts().all().first().id
        db.recurring().insert(
            RecurringEntity(
                name = "Phone", type = TxType.EXPENSE, amount = 15_99, accountId = chequing, frequency = Frequency.MONTHLY,
                anchorDate = today, nextDate = today.plusMonths(1),
            )
        )
        val goal = db.goals().insert(GoalEntity(name = "Trip", target = 10_000_00))
        db.goals().insertContribution(ContributionEntity(goalId = goal, amount = -25_50, date = today))
        return goal
    }

    private suspend fun amounts(): List<Long> =
        db.accounts().all().map { it.openingBalance } +
            db.transactions().all().sortedBy { it.id }.map { it.amount } +
            db.budgets().all().map { it.amount } +
            db.recurring().all().map { it.amount } +
            db.goals().all().map { it.target } +
            db.goals().allContributions().map { it.amount }

    @Test fun aSwitchToFewerDecimalsKeepsEachFigureAndSaysHowManyItRounds() = runTest {
        everyKindOfAmount()
        assertEquals(listOf(1_000_00L, 68_42L, 2_150_00L, 500_00L, 15_99L, 10_000_00L, -25_50L), amounts())

        val plan = data.planCurrencyChange("JPY")
        assertEquals(CurrencyPlan(code = "JPY", fromDigits = 2, toDigits = 0, rounded = 3), plan)

        assertEquals(DataResult.Done("Currency changed"), data.changeCurrency("JPY"))

        assertEquals("JPY", settings.current().currency)
        // $1,000.00 reads ¥1,000, not ¥100,000; cents round half-even, a withdrawal keeps its sign.
        assertEquals(listOf(1_000L, 68L, 2_150L, 500L, 16L, 10_000L, -26L), amounts())
    }

    @Test fun aSwitchToMoreDecimalsIsExactAndNeedsNoConfirm() = runTest {
        settings.setCurrency("JPY")
        val chequing = db.addAccount("Chequing", opening = 1_000)
        db.addExpense(1_250, today, chequing)

        assertEquals(0, data.planCurrencyChange("CAD")?.rounded)
        assertTrue(data.changeCurrency("CAD") is DataResult.Done)

        assertEquals("¥1,250 reads $1,250.00, not $12.50", listOf(1_000_00L, 1_250_00L), amounts())
        assertEquals("CAD", settings.current().currency)
    }

    @Test fun aSwitchBetweenCurrenciesWithTheSameDecimalsOnlyRelabels() = runTest {
        everyKindOfAmount()
        val before = amounts()

        assertEquals(CurrencyPlan(code = "USD", fromDigits = 2, toDigits = 2, rounded = 0), data.planCurrencyChange("USD"))
        assertNull("No plan for the currency already in use", data.planCurrencyChange("CAD"))
        assertTrue(data.changeCurrency("USD") is DataResult.Done)

        assertEquals(before, amounts())
        assertEquals("USD", settings.current().currency)
    }

    @Test fun aSwitchThatFailsHalfwayLeavesEveryAmountAndTheCurrency() = runTest {
        settings.setCurrency("JPY")
        val chequing = db.addAccount("Chequing", opening = 1_000)
        db.addExpense(1_250, today, chequing)
        // Goals are rewritten after entries and accounts; this target cannot gain two digits in a Long.
        db.goals().insert(GoalEntity(name = "Too big", target = Long.MAX_VALUE / 10))
        val before = amounts()

        assertEquals(DataResult.Failed("The currency did not change. Your amounts are as they were."), data.changeCurrency("CAD"))

        assertEquals(before, amounts())
        assertEquals("JPY", settings.current().currency)
    }

    // ── Investments (docs/INVESTMENTS.md) ────────────────────────────────────

    @Test fun investmentsAndRegistrationsRideThroughABackup() = runTest {
        val invest = db.investRepository(clock, settings)
        invest.import(WsFiles.HOLDINGS_REPORT, emptyMap(), null)
        invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        invest.setRoom(Registration.TFSA, 2026, 7_000_00)
        db.invest().insertFxRates(listOf(FxRateEntity(base = "USD", quote = "CAD", date = LocalDate.of(2026, 5, 1), rate = 137_125_000, source = "MANUAL")))
        val portfolio = invest.portfolio().first()

        val file = data.snapshot()
        assertEquals(listOf(3, 3, 3, 1, 1), listOf(file.securities.size, file.holdings.size, file.prices.size, file.fxRates.size, file.roomFacts.size))
        assertTrue(file.activities.isNotEmpty())
        val demo = file.accounts.single { it.externalRef == "DEMO0001CAD" }
        assertEquals(listOf(Registration.TFSA, "Wealthsimple"), listOf(demo.registration, demo.institution))

        val read = BackupCodec.decode(BackupCodec.encode(file))
        assertTrue("$read", read is BackupReadResult.Ok)
        assertTrue(data.restore((read as BackupReadResult.Ok).file) is DataResult.Done)
        assertEquals(file.byId(), data.snapshot().byId())
        assertEquals("The same rows read the same", portfolio, invest.portfolio().first())
    }

    /**
     * docs/INVESTMENTS.md, shared with relay-money: a backup keeps the uids an import derived (the
     * `ws:` account, its `imp:` lines), so the same file imported after a restore adds nothing. No
     * tombstone is what stops it: the restore takes back the ones its erase left for those uids.
     */
    @Test fun `a restore keeps investment uids so a re-import adds nothing`() = runTest {
        val invest = db.investRepository(clock, settings)
        val first = invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        assertEquals(1, first.accountsCreated)
        assertTrue(first.activities > 0)
        val uids = db.invest().activities().map { it.uid }.toSet()

        val file = BackupCodec.decode(BackupCodec.encode(data.snapshot())) as BackupReadResult.Ok
        assertEquals("ws:HQ7XFMC41CAD", file.file.accounts.single().uid)
        assertEquals(uids, file.file.activities.map { it.uid }.toSet())
        assertTrue(data.restore(file.file) is DataResult.Done)
        assertEquals(uids, db.invest().activities().map { it.uid }.toSet())
        assertTrue(db.sync().tombstones().none { it.tableName == "activities" || it.tableName == "accounts" })

        val again = invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        assertEquals(listOf(0, 0, first.activities), listOf(again.accountsCreated, again.activities, again.duplicates))
        assertEquals(uids, db.invest().activities().map { it.uid }.toSet())
    }

    @Test fun aRestoreGivesAFreshUidToABlankOrRepeatedOne() = runTest {
        val sample = SampleData.build(today, 2, "CAD")
        val repeated = sample.copy(
            accounts = sample.accounts.mapIndexed { i, a -> a.copy(uid = if (i == 0) " " else "same") },
            securities = listOf(SecurityDto(1, "XEQT", currency = "CAD", kind = SecurityKind.ETF, uid = "sec:XEQT")),
        )

        assertTrue(data.restore(repeated) is DataResult.Done)

        val accounts = db.accounts().all().sortedBy { it.id }.map { it.uid }
        assertEquals("same", accounts[1])
        assertEquals("Every account has its own uid", accounts.size, accounts.toSet().size)
        assertTrue(accounts.none { it.isBlank() })
        assertEquals("sec:XEQT", db.invest().securities().single().uid)
    }

    @Test fun aCurrencySwitchRescalesTheBookAndTheRoomButNotWhatIsInASecuritysOwnCurrency() = runTest {
        val invest = db.investRepository(clock, settings)
        invest.import(WsFiles.HOLDINGS_REPORT, emptyMap(), null)
        invest.import(WsFiles.ACTIVITIES_EXPORT, emptyMap(), null)
        invest.setRoom(Registration.TFSA, 2026, 7_000_50)
        val activities = db.invest().activities()
        val prices = db.invest().prices()

        assertEquals("The report's value (1,633.33) and the room's 50 cents round", 2, data.planCurrencyChange("JPY")?.rounded)
        assertTrue(data.changeCurrency("JPY") is DataResult.Done)

        val held = db.invest().holdings()
        assertEquals("The ledger's book rescales", listOf(1_000L, 250L, 50L), held.map { it.book })
        assertEquals("The market's book is the security's own", listOf(750_00L, 250_00L, 50_00L), held.map { it.bookMarket })
        assertEquals(7_000L, db.invest().roomFacts().single().amount)
        assertEquals(activities.map { it.amount }, db.invest().activities().map { it.amount })
        assertEquals(prices.map { it.price }, db.invest().prices().map { it.price })
    }
}
