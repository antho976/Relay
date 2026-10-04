package com.margin.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.room.withTransaction
import com.margin.app.data.Clock
import com.margin.app.data.db.AccountEntity
import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.db.CategoryEntity
import com.margin.app.data.db.ContributionEntity
import com.margin.app.data.db.GoalEntity
import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.RecurringEntity
import com.margin.app.data.db.TransactionEntity
import com.margin.app.data.prefs.SettingsRepository
import com.margin.core.AccountDto
import com.margin.core.AccountType
import com.margin.core.BackupCodec
import com.margin.core.BackupFile
import com.margin.core.BackupReadResult
import com.margin.core.BudgetDto
import com.margin.core.CategoryDto
import com.margin.core.CategoryKind
import com.margin.core.ContributionDto
import com.margin.core.CsvCodec
import com.margin.core.CsvRow
import com.margin.core.GoalDto
import com.margin.core.MoneyFormatter
import com.margin.core.RecurringDto
import com.margin.core.SampleData
import com.margin.core.TransactionDto
import com.margin.core.TxType
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject
import javax.inject.Singleton

/** The outcome of a file operation, worded for the owner. */
sealed interface DataResult {
    data class Done(val message: String) : DataResult
    data class Failed(val message: String) : DataResult
}

/**
 * Everything that moves the whole ledger at once: JSON backup and restore, CSV export and import,
 * the sample set, and erase. Files go through the Storage Access Framework, so the app needs no
 * storage permission and the owner picks where every file lives.
 */
@Singleton
class DataRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: MarginDatabase,
    private val settings: SettingsRepository,
    private val ledger: LedgerRepository,
    private val clock: Clock,
) {

    suspend fun snapshot(): BackupFile = withContext(Dispatchers.IO) {
        val s = settings.current()
        BackupFile(
            exportedAt = clock.today().toString(),
            currency = s.currency,
            accounts = db.accounts().all().map { AccountDto(it.id, it.name, it.type, it.openingBalance, it.archived, it.sortOrder) },
            categories = db.categories().all().map { CategoryDto(it.id, it.name, it.kind, it.color, it.icon, it.archived, it.sortOrder) },
            transactions = db.transactions().all().map {
                TransactionDto(it.id, it.type, it.amount, it.date.toString(), it.accountId, it.toAccountId, it.categoryId, it.note, it.recurringId)
            },
            budgets = db.budgets().all().map { BudgetDto(it.id, it.categoryId.takeIf { c -> c != BudgetEntity.OVERALL }, it.amount) },
            recurring = db.recurring().all().map {
                RecurringDto(
                    it.id, it.name, it.type, it.amount, it.accountId, it.toAccountId, it.categoryId, it.frequency,
                    it.interval, it.anchorDate.toString(), it.nextDate.toString(), it.endDate?.toString(), it.autoPost, it.active,
                )
            },
            goals = db.goals().all().map { GoalDto(it.id, it.name, it.target, it.targetDate?.toString(), it.color, it.archived) },
            contributions = db.goals().allContributions().map { ContributionDto(it.id, it.goalId, it.amount, it.date.toString(), it.note) },
        )
    }

    suspend fun writeBackup(uri: Uri): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            val text = BackupCodec.encode(snapshot())
            context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) }
                ?: error("no stream")
        }.fold(
            onSuccess = { DataResult.Done("Backup saved") },
            onFailure = { DataResult.Failed("The backup could not be written to that location.") },
        )
    }

    suspend fun readBackup(uri: Uri): BackupReadResult = withContext(Dispatchers.IO) {
        val text = runCatching {
            context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) }
        }.getOrNull() ?: return@withContext BackupReadResult.Invalid("That file could not be opened.")
        BackupCodec.decode(text)
    }

    /** Replaces everything with [file]. One transaction: a failed restore leaves the old data. */
    suspend fun restore(file: BackupFile): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            db.withTransaction {
                wipeTables()
                insertFile(file)
            }
            settings.setCurrency(file.currency)
            settings.setOnboarded(true)
            settings.setSampleLoaded(false)
            file.accounts.firstOrNull { !it.archived }?.let { settings.setDefaultAccount(it.id) }
        }.fold(
            onSuccess = { DataResult.Done("Restored ${file.transactions.size} entries") },
            onFailure = { DataResult.Failed("The restore did not finish. Your data is unchanged.") },
        )
    }

    private suspend fun insertFile(file: BackupFile) {
        db.accounts().insertAll(file.accounts.map { AccountEntity(it.id, it.name, it.type, it.openingBalance, it.archived, it.sortOrder) })
        db.categories().insertAll(file.categories.map { CategoryEntity(it.id, it.name, it.kind, it.color, it.icon, it.archived, it.sortOrder) })
        db.recurring().insertAll(file.recurring.map {
            RecurringEntity(
                it.id, it.name, it.type, it.amount, it.accountId, it.toAccountId, it.categoryId, it.frequency, it.interval,
                LocalDate.parse(it.anchorDate), LocalDate.parse(it.nextDate), it.endDate?.let(LocalDate::parse), it.autoPost, it.active,
            )
        })
        db.transactions().insertAll(file.transactions.map {
            TransactionEntity(it.id, it.type, it.amount, LocalDate.parse(it.date), it.accountId, it.toAccountId, it.categoryId, it.note, it.recurringId)
        })
        db.budgets().insertAll(file.budgets.map { BudgetEntity(it.id, it.categoryId ?: BudgetEntity.OVERALL, it.amount) })
        db.goals().insertAll(file.goals.map { GoalEntity(it.id, it.name, it.target, it.targetDate?.let(LocalDate::parse), it.color, it.archived) })
        db.goals().insertContributions(file.contributions.map { ContributionEntity(it.id, it.goalId, it.amount, LocalDate.parse(it.date), it.note) })
    }

    private suspend fun wipeTables() {
        db.goals().deleteAllContributions()
        db.goals().deleteAll()
        db.budgets().deleteAll()
        db.transactions().deleteAll()
        db.recurring().deleteAll()
        db.categories().deleteAll()
        db.accounts().deleteAll()
    }

    suspend fun writeCsv(uri: Uri): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            val s = settings.current()
            val fmt = MoneyFormatter(s.currency, Locale.getDefault())
            val rows = db.transactions().allRows().map {
                CsvRow(it.date, it.type, it.amount, it.categoryName, it.accountName, it.toAccountName, it.note)
            }
            val text = CsvCodec.encode(rows, fmt.currency.currencyCode, fmt.fractionDigits)
            context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) } ?: error("no stream")
            rows.size
        }.fold(
            onSuccess = { DataResult.Done("Exported ${it} entries") },
            onFailure = { DataResult.Failed("The CSV could not be written to that location.") },
        )
    }

    /**
     * Adds the rows of a CSV to the ledger. Accounts and categories are matched by name and created
     * when missing. Nothing is replaced, and the whole import is one transaction.
     */
    suspend fun importCsv(uri: Uri): DataResult = withContext(Dispatchers.IO) {
        val text = runCatching {
            context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) }
        }.getOrNull() ?: return@withContext DataResult.Failed("That file could not be opened.")
        val s = settings.current()
        val fmt = MoneyFormatter(s.currency, Locale.getDefault())
        val defaultAccount = db.accounts().get(s.defaultAccountId)?.name ?: db.accounts().all().firstOrNull()?.name ?: "Imported"
        val parsed = CsvCodec.decode(text, fmt.fractionDigits, defaultAccount)
        if (parsed.rows.isEmpty()) {
            return@withContext DataResult.Failed(parsed.errors.firstOrNull() ?: "No entries found in that file.")
        }
        runCatching {
            db.withTransaction {
                val accounts = db.accounts().all().associateBy { it.name.lowercase() }.toMutableMap()
                suspend fun accountId(name: String): Long = accounts[name.lowercase()]?.id ?: run {
                    val id = ledger.saveAccount(AccountEntity(name = name, type = AccountType.CHEQUING))
                    accounts[name.lowercase()] = AccountEntity(id = id, name = name, type = AccountType.CHEQUING)
                    id
                }
                val catCache = HashMap<String, Long>()
                suspend fun categoryId(name: String?, type: TxType): Long? {
                    if (name.isNullOrBlank() || type == TxType.TRANSFER) return null
                    val kind = if (type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
                    val key = "${kind.name}:${name.lowercase()}"
                    return catCache.getOrPut(key) {
                        db.categories().findByName(name, kind)?.id
                            ?: ledger.saveCategory(CategoryEntity(name = name, kind = kind, color = (catCache.size + 3) % 12, icon = "dots"))
                    }
                }
                val now = clock.nowMillis()
                val entities = parsed.rows.map { r ->
                    TransactionEntity(
                        type = r.type,
                        amount = r.amount,
                        date = r.date,
                        accountId = accountId(r.account),
                        toAccountId = r.toAccount?.let { accountId(it) },
                        categoryId = categoryId(r.category, r.type),
                        note = r.note,
                        createdAt = now,
                    )
                }.filter { it.type != TxType.TRANSFER || it.toAccountId != it.accountId }
                db.transactions().insertAll(entities)
                entities.size
            }
        }.fold(
            onSuccess = { n ->
                val skipped = parsed.errors.size
                DataResult.Done(if (skipped == 0) "Imported $n entries" else "Imported $n entries, skipped $skipped")
            },
            onFailure = { DataResult.Failed("The import did not finish. Nothing was added.") },
        )
    }

    /** Fills an EMPTY ledger with the labeled sample set. Refuses to mix it into real entries. */
    suspend fun loadSample(): DataResult = withContext(Dispatchers.IO) {
        if (db.transactions().count() > 0) {
            return@withContext DataResult.Failed("Sample data loads only into an empty ledger. Erase first to try it.")
        }
        val s = settings.current()
        val fmt = MoneyFormatter(s.currency, Locale.getDefault())
        val file = SampleData.build(clock.today(), fmt.fractionDigits, fmt.currency.currencyCode)
        runCatching {
            db.withTransaction {
                wipeTables()
                insertFile(file)
            }
            settings.setOnboarded(true)
            settings.setSampleLoaded(true)
            settings.setDefaultAccount(file.accounts.first().id)
        }.fold(
            onSuccess = { DataResult.Done("Sample data loaded") },
            onFailure = { DataResult.Failed("The sample could not be loaded.") },
        )
    }

    /** Removes every entry, account, category, budget, bill and goal. Keeps the app's look. */
    suspend fun eraseAll(): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            db.withTransaction { wipeTables() }
            settings.resetData()
            ledger.seedCategoriesIfEmpty()
        }.fold(
            onSuccess = { DataResult.Done("Everything erased") },
            onFailure = { DataResult.Failed("Erase did not finish.") },
        )
    }
}
