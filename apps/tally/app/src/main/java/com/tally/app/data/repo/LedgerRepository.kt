package com.tally.app.data.repo

import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountDao
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.BudgetDao
import com.tally.app.data.db.CategoryDao
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.DayTotal
import com.tally.app.data.db.LedgerTotals
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionDao
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.db.TypeTotal
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Defaults
import com.tally.core.TextMatch
import com.tally.core.TxType
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/** What the Activity search is narrowed by. Every field is optional. */
data class LedgerFilter(
    val query: String = "",
    val type: TxType? = null,
    val categoryId: Long? = null,
    val accountId: Long? = null,
    val start: LocalDate? = null,
    val end: LocalDate? = null,
) {
    val isNarrowed: Boolean get() = query.isNotBlank() || type != null || categoryId != null || accountId != null
}

@Singleton
class LedgerRepository @Inject constructor(
    private val db: TallyDatabase,
    private val transactions: TransactionDao,
    private val accounts: AccountDao,
    private val categories: CategoryDao,
    private val budgets: BudgetDao,
    private val clock: Clock,
) {
    // ── Reads ────────────────────────────────────────────────────────────────

    fun recent(limit: Int): Flow<List<TransactionRow>> = transactions.observeRecent(limit)

    fun between(period: BudgetPeriod): Flow<List<TransactionRow>> =
        transactions.observeBetween(period.start, period.endExclusive)

    /** The newest [limit] entries matching [filter]; the query matches in any case, any alphabet. */
    fun search(filter: LedgerFilter, limit: Int = SEARCH_LIMIT): Flow<List<TransactionRow>> = transactions.search(
        pattern = TextMatch.containsPattern(filter.query),
        type = filter.type,
        categoryId = filter.categoryId,
        accountId = filter.accountId,
        start = filter.start,
        end = filter.end,
        limit = limit,
    )

    /** What every entry matching [filter] adds up to, never capped like [search]'s list. */
    fun searchTotals(filter: LedgerFilter): Flow<LedgerTotals> = transactions.searchTotals(
        pattern = TextMatch.containsPattern(filter.query),
        type = filter.type,
        categoryId = filter.categoryId,
        accountId = filter.accountId,
        start = filter.start,
        end = filter.end,
    )

    /** The largest entry matching [filter], of the type the readings are about; null when none match. */
    fun searchLargest(filter: LedgerFilter): Flow<TransactionRow?> = transactions.searchLargest(
        pattern = TextMatch.containsPattern(filter.query),
        type = filter.type,
        categoryId = filter.categoryId,
        accountId = filter.accountId,
        start = filter.start,
        end = filter.end,
    )

    fun byCategory(type: TxType, start: LocalDate, endExclusive: LocalDate): Flow<List<CategoryTotal>> =
        transactions.observeByCategory(type, start, endExclusive)

    fun dailyExpense(start: LocalDate, endExclusive: LocalDate): Flow<List<DayTotal>> =
        transactions.observeDailyExpense(start, endExclusive)

    fun typeTotals(start: LocalDate, endExclusive: LocalDate): Flow<List<TypeTotal>> =
        transactions.observeTypeTotals(start, endExclusive)

    fun balances(): Flow<List<AccountBalance>> = accounts.observeBalances()
    fun accounts(): Flow<List<AccountEntity>> = accounts.observeAll()
    fun categories(): Flow<List<CategoryEntity>> = categories.observeAll()
    fun categoriesWithCounts(): Flow<List<com.tally.app.data.db.CategoryWithCount>> = categories.observeWithCounts()
    fun activeCategories(kind: CategoryKind): Flow<List<CategoryEntity>> = categories.observeActive(kind)
    fun count(): Flow<Int> = transactions.observeCount()

    suspend fun transaction(id: Long): TransactionEntity? = transactions.get(id)
    suspend fun account(id: Long): AccountEntity? = accounts.get(id)
    suspend fun category(id: Long): CategoryEntity? = categories.get(id)
    suspend fun noteSuggestions(prefix: String): List<String> =
        if (prefix.isBlank()) emptyList() else transactions.noteSuggestions(TextMatch.prefixPattern(prefix))
    suspend fun lastCategoryForNote(note: String): Long? = transactions.lastCategoryForNote(note.trim())
    suspend fun entriesForAccount(id: Long): Int = transactions.countForAccount(id)
    suspend fun entriesForCategory(id: Long): Int = transactions.countForCategory(id)

    // ── Writes ───────────────────────────────────────────────────────────────

    /** Inserts when [tx] has no id, updates otherwise. Returns the row's id. */
    suspend fun save(tx: TransactionEntity): Long {
        val clean = when (tx.type) {
            TxType.TRANSFER -> tx.copy(categoryId = null)
            else -> tx.copy(toAccountId = null)
        }.copy(note = tx.note.trim())
        require(clean.amount > 0) { "An entry needs an amount" }
        require(clean.type != TxType.TRANSFER || (clean.toAccountId != null && clean.toAccountId != clean.accountId)) {
            "A transfer needs two different accounts"
        }
        return if (clean.id == 0L) {
            transactions.insert(clean.copy(createdAt = clock.nowMillis()))
        } else {
            transactions.update(clean)
            clean.id
        }
    }

    /** Deletes and hands back the row, so the caller's Undo can put it back exactly. */
    suspend fun delete(id: Long): TransactionEntity? {
        val row = transactions.get(id) ?: return null
        transactions.delete(id)
        return row
    }

    suspend fun restore(tx: TransactionEntity) {
        transactions.insert(tx)
    }

    suspend fun saveAccount(account: AccountEntity): Long =
        if (account.id == 0L) accounts.insert(account.copy(name = account.name.trim(), sortOrder = accounts.nextSortOrder()))
        else { accounts.update(account.copy(name = account.name.trim())); account.id }

    /** Removes the account and, through the foreign key, every entry that touches it. */
    suspend fun deleteAccount(id: Long) {
        accounts.get(id)?.let { accounts.delete(it) }
    }

    suspend fun saveCategory(category: CategoryEntity): Long =
        if (category.id == 0L) categories.insert(category.copy(name = category.name.trim(), sortOrder = categories.nextSortOrder(category.kind)))
        else { categories.update(category.copy(name = category.name.trim())); category.id }

    /**
     * Deletes a category. Its entries and bills move to [moveTo] when given, or become
     * uncategorized; its budget goes with it.
     */
    suspend fun deleteCategory(id: Long, moveTo: Long?) = db.withTransaction {
        if (moveTo != null && moveTo != id) {
            transactions.moveCategory(id, moveTo)
            // Bills filed under it move too, or they would post uncategorized from now on.
            db.recurring().moveCategory(id, moveTo)
        }
        budgets.deleteFor(id)
        categories.delete(id)
    }

    /** First run: the default categories, once. */
    suspend fun seedCategoriesIfEmpty() {
        if (categories.count() > 0) return
        categories.insertAll(
            Defaults.categories.mapIndexed { i, c ->
                CategoryEntity(name = c.name, kind = c.kind, color = c.color, icon = c.icon, sortOrder = i)
            }
        )
    }

    companion object {
        const val SEARCH_LIMIT = 400
    }
}
