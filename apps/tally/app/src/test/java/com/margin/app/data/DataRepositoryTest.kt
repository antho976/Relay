package com.margin.app.data

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.TransactionRow
import com.margin.app.data.prefs.Accent
import com.margin.app.data.prefs.SettingsRepository
import com.margin.app.data.repo.DataRepository
import com.margin.app.data.repo.DataResult
import com.margin.core.AccountDto
import com.margin.core.AccountType
import com.margin.core.BackupCodec
import com.margin.core.BackupFile
import com.margin.core.BackupReadResult
import com.margin.core.CategoryKind
import com.margin.core.Defaults
import com.margin.core.SampleData
import com.margin.core.TransactionDto
import com.margin.core.TxType
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
    private lateinit var db: MarginDatabase
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
        assertEquals("The restore wrote exactly what the file says", sample.byId(), first.byId())

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
        val uri = uriFor("margin-backup.json")

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

        assertEquals(BackupReadResult.Invalid("This file is not a Margin backup."), garbage)
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

        assertEquals(SampleData.build(today, 2, "CAD").byId(), data.snapshot().byId())
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
        val uri = uriFor("margin.csv")

        assertEquals(DataResult.Done("Exported ${exported.size} entries"), data.writeCsv(uri))
        assertTrue(data.eraseAll() is DataResult.Done)
        assertEquals(DataResult.Done("Imported ${exported.size} entries"), data.importCsv(uri))

        assertEquals(exported, db.transactions().allRows().map { it.line() }.sorted())
        assertNull("Imported accounts are plain chequing until the owner says otherwise",
            db.accounts().all().firstOrNull { it.type != AccountType.CHEQUING })
    }
}
