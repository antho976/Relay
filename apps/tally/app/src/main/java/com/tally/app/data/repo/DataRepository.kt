package com.tally.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.ActivityEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.FxRateEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.HoldingEntity
import com.tally.app.data.db.PriceEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RoomFactEntity
import com.tally.app.data.db.SecurityEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.db.newUid
import com.tally.app.data.prefs.SettingsRepository
import com.tally.core.AccountDto
import com.tally.core.AccountType
import com.tally.core.AccountValueDto
import com.tally.core.ActivityDto
import com.tally.core.BackupCodec
import com.tally.core.BackupFile
import com.tally.core.BackupReadResult
import com.tally.core.BudgetDto
import com.tally.core.CategoryDto
import com.tally.core.CategoryKind
import com.tally.core.ContributionDto
import com.tally.core.CsvCodec
import com.tally.core.CsvRow
import com.tally.core.FxRateDto
import com.tally.core.GoalDto
import com.tally.core.HoldingDto
import com.tally.core.MoneyFormatter
import com.tally.core.PriceDto
import com.tally.core.RecurringDto
import com.tally.core.RoomFactDto
import com.tally.core.SampleData
import com.tally.core.SecurityDto
import com.tally.core.TransactionDto
import com.tally.core.TxType
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
 * A currency switch, worked out before it runs: from and to how many decimals, and how many
 * stored amounts would be rounded to fit (0 when the switch keeps every amount exactly).
 */
data class CurrencyPlan(val code: String, val fromDigits: Int, val toDigits: Int, val rounded: Int)

/**
 * Everything that moves the whole ledger at once: JSON backup and restore, CSV export and import,
 * the sample set, and erase. Files go through the Storage Access Framework, so the app needs no
 * storage permission and the owner picks where every file lives.
 */
@Singleton
class DataRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: TallyDatabase,
    private val settings: SettingsRepository,
    private val ledger: LedgerRepository,
    private val clock: Clock,
) {

    /**
     * The whole ledger as one file. The reads are one transaction, so a bill the daily poster
     * advances meanwhile is never captured past entries the file does not hold. The currency is
     * read inside it too, since a currency switch rescales the amounts in the same transaction.
     */
    suspend fun snapshot(): BackupFile = withContext(Dispatchers.IO) {
        db.withTransaction {
            val s = settings.current()
            BackupFile(
                exportedAt = clock.today().toString(),
                currency = s.currency,
                monthStartDay = s.monthStartDay,
                weekStartsMonday = s.weekStartsMonday,
                accounts = db.accounts().all().map {
                    AccountDto(
                        it.id, it.name, it.type, it.openingBalance, it.archived, it.sortOrder,
                        registration = it.registration, institution = it.institution, externalRef = it.externalRef, uid = it.uid,
                    )
                },
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
                goals = db.goals().all().map {
                    GoalDto(
                        it.id, it.name, it.target, it.targetDate?.toString(), it.color, it.archived,
                        kind = it.kind, accountId = it.accountId, percent = it.percent,
                        startDate = it.startDate?.toString(), startAmount = it.startAmount,
                    )
                },
                contributions = db.goals().allContributions().map { ContributionDto(it.id, it.goalId, it.amount, it.date.toString(), it.note) },
                values = db.values().all().map { AccountValueDto(it.id, it.accountId, it.date.toString(), it.value) },
                securities = db.invest().securities().map {
                    SecurityDto(id = it.id, symbol = it.symbol, name = it.name, currency = it.currency, kind = it.kind, exchange = it.exchange, uid = it.uid)
                },
                holdings = db.invest().holdings().map {
                    HoldingDto(
                        id = it.id, accountId = it.accountId, securityId = it.securityId, date = it.date.toString(),
                        quantity = it.quantity, book = it.book, bookMarket = it.bookMarket, uid = it.uid,
                    )
                },
                activities = db.invest().activities().map {
                    ActivityDto(
                        id = it.id, accountId = it.accountId, securityId = it.securityId, type = it.type, date = it.date.toString(),
                        quantity = it.quantity, amount = it.amount, fee = it.fee, currency = it.currency,
                        toAmount = it.toAmount, toCurrency = it.toCurrency, note = it.note, source = it.source, uid = it.uid,
                    )
                },
                prices = db.invest().prices().map {
                    PriceDto(id = it.id, securityId = it.securityId, date = it.date.toString(), price = it.price, source = it.source, uid = it.uid)
                },
                fxRates = db.invest().fxRates().map {
                    FxRateDto(id = it.id, base = it.base, quote = it.quote, date = it.date.toString(), rate = it.rate, source = it.source, uid = it.uid)
                },
                roomFacts = db.invest().roomFacts().map {
                    RoomFactDto(id = it.id, registration = it.registration, year = it.year, amount = it.amount, uid = it.uid)
                },
            )
        }
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
            // A file that does not say (the sample, or one saved before these were kept) leaves them.
            file.monthStartDay?.let { settings.setMonthStartDay(it) }
            file.weekStartsMonday?.let { settings.setWeekStartsMonday(it) }
            settings.setOnboarded(true)
            settings.setSampleLoaded(false)
            // Always reset: an id from the old data must not survive into the restored set.
            settings.setDefaultAccount(file.accounts.firstOrNull { !it.archived }?.id ?: 0)
        }.fold(
            onSuccess = { DataResult.Done("Restored ${file.transactions.size} entries") },
            onFailure = { DataResult.Failed("The restore did not finish. Your data is unchanged.") },
        )
    }

    /**
     * The uid a restored row takes: the file's, so an import's derived uids (`ws:`, `imp:`, `sec:`…)
     * outlive the restore and the same file imported again adds nothing (docs/INVESTMENTS.md). A
     * file without one (older, the sample, or hand-made), or one an earlier row of the same table
     * already took, gets a fresh uid instead, as every row did before.
     */
    private class Uids {
        private val taken = HashSet<String>()
        fun keep(uid: String?): String = uid?.takeIf { it.isNotBlank() && taken.add(it) } ?: newUid()
    }

    private suspend fun insertFile(file: BackupFile) {
        val accountUids = Uids()
        db.accounts().insertAll(file.accounts.map {
            AccountEntity(
                it.id, it.name, it.type, it.openingBalance, it.archived, it.sortOrder,
                registration = it.registration, institution = it.institution, externalRef = it.externalRef, uid = accountUids.keep(it.uid),
            )
        })
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
        db.goals().insertAll(file.goals.map {
            GoalEntity(
                it.id, it.name, it.target, it.targetDate?.let(LocalDate::parse), it.color, it.archived,
                kind = it.kind, accountId = it.accountId, percent = it.percent,
                startDate = it.startDate?.let(LocalDate::parse), startAmount = it.startAmount,
            )
        })
        db.goals().insertContributions(file.contributions.map { ContributionEntity(it.id, it.goalId, it.amount, LocalDate.parse(it.date), it.note) })
        db.values().insertAll(file.values.map { AccountValueEntity(it.id, it.accountId, LocalDate.parse(it.date), it.value) })
        val securityUids = Uids()
        db.invest().insertSecurities(file.securities.map {
            SecurityEntity(
                id = it.id, symbol = it.symbol, name = it.name, currency = it.currency, kind = it.kind, exchange = it.exchange,
                uid = securityUids.keep(it.uid),
            )
        })
        val holdingUids = Uids()
        db.invest().insertHoldings(file.holdings.map {
            HoldingEntity(
                id = it.id, accountId = it.accountId, securityId = it.securityId, date = LocalDate.parse(it.date),
                quantity = it.quantity, book = it.book, bookMarket = it.bookMarket, uid = holdingUids.keep(it.uid),
            )
        })
        val activityUids = Uids()
        db.invest().insertActivities(file.activities.map {
            ActivityEntity(
                id = it.id, accountId = it.accountId, securityId = it.securityId, type = it.type, date = LocalDate.parse(it.date),
                quantity = it.quantity, amount = it.amount, fee = it.fee, currency = it.currency,
                toAmount = it.toAmount, toCurrency = it.toCurrency, note = it.note, source = it.source, uid = activityUids.keep(it.uid),
            )
        })
        val priceUids = Uids()
        db.invest().insertPrices(file.prices.map {
            PriceEntity(
                id = it.id, securityId = it.securityId, date = LocalDate.parse(it.date), price = it.price, source = it.source,
                uid = priceUids.keep(it.uid),
            )
        })
        val rateUids = Uids()
        db.invest().insertFxRates(file.fxRates.map {
            FxRateEntity(
                id = it.id, base = it.base, quote = it.quote, date = LocalDate.parse(it.date), rate = it.rate, source = it.source,
                uid = rateUids.keep(it.uid),
            )
        })
        val roomUids = Uids()
        db.invest().insertRoomFacts(file.roomFacts.map {
            RoomFactEntity(id = it.id, registration = it.registration, year = it.year, amount = it.amount, uid = roomUids.keep(it.uid))
        })
    }

    private suspend fun wipeTables() {
        db.invest().deleteAll()
        db.values().deleteAll()
        db.goals().deleteAllContributions()
        db.goals().deleteAll()
        db.budgets().deleteAll()
        db.transactions().deleteAll()
        db.recurring().deleteAll()
        db.categories().deleteAll()
        db.accounts().deleteAll()
    }

    /** Every entry as CSV, or those from [start] (inclusive) to [end] (exclusive) when given. */
    suspend fun writeCsv(uri: Uri, start: LocalDate? = null, end: LocalDate? = null): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            val s = settings.current()
            val fmt = MoneyFormatter(s.currency, Locale.getDefault())
            val all = db.transactions().allRows()
            val picked = all.filter { (start == null || !it.date.isBefore(start)) && (end == null || it.date.isBefore(end)) }
            val rows = picked.map {
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
        importCsvText(text)
    }

    /** [importCsv] over text already read, for a file the bank import recognised as Tally's own. */
    suspend fun importCsvText(text: String): DataResult = withContext(Dispatchers.IO) {
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
                // Matched in Kotlin with the category editor's own rule (Unicode, any case), not
                // SQLite's NOCASE, which folds ASCII only and would make "épicerie" a second "Épicerie".
                val existing = db.categories().all()
                suspend fun categoryId(name: String?, type: TxType): Long? {
                    if (name.isNullOrBlank() || type == TxType.TRANSFER) return null
                    val kind = if (type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
                    val key = "${kind.name}:${name.lowercase()}"
                    return catCache.getOrPut(key) {
                        existing.firstOrNull { it.kind == kind && it.name.trim().equals(name.trim(), ignoreCase = true) }?.id
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
                }
                val valid = entities.filter { it.type != TxType.TRANSFER || it.toAccountId != it.accountId }
                db.transactions().insertAll(valid)
                valid.size to (entities.size - valid.size)
            }
        }.fold(
            onSuccess = { (n, sameAccountTransfers) ->
                val skipped = parsed.errors.size + sameAccountTransfers
                DataResult.Done(if (skipped == 0) "Imported $n entries" else "Imported $n entries, skipped $skipped")
            },
            onFailure = { DataResult.Failed("The import did not finish. Nothing was added.") },
        )
    }

    // ── Currency ─────────────────────────────────────────────────────────────

    /**
     * What switching to [code] does to the stored amounts, read before anything changes. Null
     * when [code] is already the currency. [rounded] counts the amounts that would lose digits
     * (entries, opening balances, budgets, bills, goals, contributions, account values, holdings'
     * book and room); zero means the switch is exact and needs no confirm.
     */
    suspend fun planCurrencyChange(code: String): CurrencyPlan? = withContext(Dispatchers.IO) {
        val current = settings.current().currency
        if (code == current) return@withContext null
        val from = MoneyFormatter(current, Locale.getDefault()).fractionDigits
        val to = MoneyFormatter(code, Locale.getDefault()).fractionDigits
        val rounded = if (to < from) {
            db.withTransaction { storedAmounts().count { MoneyFormatter.rescaleRounds(it, from, to) } }
        } else 0
        CurrencyPlan(code = code, fromDigits = from, toDigits = to, rounded = rounded)
    }

    /**
     * Switches the currency. Amounts are minor units of the CURRENT currency, so when [code] has a
     * different number of decimals every stored amount is rewritten to keep its figure ($12.50
     * reads 12.50 in any 2-decimal currency and ¥12 in yen, never ¥1,250), in the same transaction as
     * the setting: a failure leaves both the amounts and the currency as they were. The current
     * currency is read inside the transaction too, so two quick switches cannot rescale twice from
     * the same starting point.
     */
    suspend fun changeCurrency(code: String): DataResult = withContext(Dispatchers.IO) {
        runCatching {
            currencyLock.withLock {
                db.withTransaction {
                    val current = settings.current().currency
                    if (code != current) {
                        val from = MoneyFormatter(current, Locale.getDefault()).fractionDigits
                        val to = MoneyFormatter(code, Locale.getDefault()).fractionDigits
                        if (from != to) rescaleAmounts { MoneyFormatter.rescale(it, from, to) }
                        settings.setCurrency(code)
                    }
                }
            }
        }.fold(
            onSuccess = { DataResult.Done("Currency changed") },
            onFailure = { DataResult.Failed("The currency did not change. Your amounts are as they were.") },
        )
    }

    private val currencyLock = Mutex()

    /**
     * Every stored amount in the ledger currency, in no particular order. Call inside a transaction
     * for one consistent read. An investment's amounts in its own currency (an activity's, a book
     * in the market's currency, a price) are not the ledger's and do not move with it.
     */
    private suspend fun storedAmounts(): List<Long> =
        db.transactions().all().map { it.amount } +
            db.accounts().all().map { it.openingBalance } +
            db.budgets().all().map { it.amount } +
            db.recurring().all().map { it.amount } +
            db.goals().all().flatMap { listOf(it.target, it.startAmount) } +
            db.goals().allContributions().map { it.amount } +
            db.values().all().map { it.value } +
            db.invest().holdings().map { it.book } +
            db.invest().roomFacts().map { it.amount }

    /** Rewrites every stored amount through [rescale]. Runs inside the caller's transaction. */
    private suspend fun rescaleAmounts(rescale: (Long) -> Long) {
        db.transactions().updateAll(db.transactions().all().map { it.copy(amount = rescale(it.amount)) })
        db.accounts().updateAll(db.accounts().all().map { it.copy(openingBalance = rescale(it.openingBalance)) })
        db.budgets().updateAll(db.budgets().all().map { it.copy(amount = rescale(it.amount)) })
        db.recurring().updateAll(db.recurring().all().map { it.copy(amount = rescale(it.amount)) })
        db.goals().updateAll(db.goals().all().map { it.copy(target = rescale(it.target), startAmount = rescale(it.startAmount)) })
        db.goals().updateContributions(db.goals().allContributions().map { it.copy(amount = rescale(it.amount)) })
        db.values().updateAll(db.values().all().map { it.copy(value = rescale(it.value)) })
        db.invest().updateHoldings(db.invest().holdings().map { it.copy(book = rescale(it.book)) })
        db.invest().updateRoomFacts(db.invest().roomFacts().map { it.copy(amount = rescale(it.amount)) })
    }

    /**
     * True when nothing the owner made exists yet. The seeded default categories do not count;
     * an account, a budget, a bill or a goal does, because loading the sample replaces them all.
     */
    suspend fun isEmptyForSample(): Boolean = withContext(Dispatchers.IO) {
        db.transactions().count() == 0 && db.accounts().count() == 0 && db.budgets().count() == 0 &&
            db.recurring().count() == 0 && db.goals().count() == 0 &&
            db.invest().securities().isEmpty() && db.invest().roomFacts().isEmpty() && db.invest().fxRates().isEmpty()
    }

    /** Fills an EMPTY app with the labeled sample set. Refuses to replace anything the owner made. */
    suspend fun loadSample(): DataResult = withContext(Dispatchers.IO) {
        if (!isEmptyForSample()) {
            return@withContext DataResult.Failed("Sample data loads only into an empty app. Erase everything first to try it.")
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

    /** Removes every entry, account, category, budget, bill, goal and investment. Keeps the app's look. */
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
