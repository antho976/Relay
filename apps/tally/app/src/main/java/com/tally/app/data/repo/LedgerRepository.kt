package com.tally.app.data.repo

import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountDao
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.BudgetDao
import com.tally.app.data.db.CategoryDao
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.DayTotal
import com.tally.app.data.db.FlowRow
import com.tally.app.data.db.LedgerTotals
import com.tally.app.data.db.NoteUse
import com.tally.app.data.db.PayeeTotal
import com.tally.app.data.db.QuickPick
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionDao
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.db.TransferRow
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

/** What deleting an account takes with it, counted when the delete is asked for. */
data class AccountUse(
    /** Every entry that touches the account, transfers either way included. */
    val entries: Int = 0,
    /** Bills paying from or into it. */
    val bills: Int = 0,
    /** The accounts that stay but read differently once the transfers with this one are gone. */
    val others: List<OtherAccountChange> = emptyList(),
) {
    val isEmpty: Boolean get() = entries == 0 && bills == 0
}

/** One account that stays: [transfers] with the deleted one go, which moves its balance by [change]. */
data class OtherAccountChange(val name: String, val transfers: Int, val change: Long)

/** What deleting a category touches: entries and bills it files (they can move), and its budget (it goes). */
data class CategoryUse(val entries: Int = 0, val bills: Int = 0, val budget: Long? = null) {
    /** Entries and bills can move to another category; a budget cannot, so it alone needs no choice. */
    val hasMovable: Boolean get() = entries > 0 || bills > 0
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

    fun transfers(start: LocalDate, endExclusive: LocalDate): Flow<List<TransferRow>> =
        transactions.observeTransfers(start, endExclusive)

    /** The entries logged most often since [since]: the editor's quick picks. */
    fun quickPicks(type: TxType, since: LocalDate, limit: Int): Flow<List<QuickPick>> =
        transactions.observeQuickPicks(type, since, limit)

    fun payees(type: TxType, start: LocalDate, endExclusive: LocalDate, limit: Int): Flow<List<PayeeTotal>> =
        transactions.observePayees(type, start, endExclusive, limit)

    /** Every entry's effect on balances, oldest first. */
    fun flows(): Flow<List<FlowRow>> = transactions.observeFlows()

    suspend fun touching(accountId: Long, start: LocalDate, endInclusive: LocalDate): List<TransactionEntity> =
        transactions.touching(accountId, start, endInclusive)

    suspend fun notesSince(since: LocalDate): List<NoteUse> = transactions.notesSince(since)

    // ── Account values ───────────────────────────────────────────────────────

    fun values(accountId: Long): Flow<List<AccountValueEntity>> = db.values().observeFor(accountId)
    fun allValues(): Flow<List<AccountValueEntity>> = db.values().observeAll()

    /** Records what [accountId] was worth on [date]; a second value the same day replaces the first. */
    suspend fun setValue(accountId: Long, date: LocalDate, value: Long): Long = db.withTransaction {
        val same = db.values().on(accountId, date)
        if (same != null) {
            db.values().update(same.copy(value = value))
            same.id
        } else {
            db.values().insert(AccountValueEntity(accountId = accountId, date = date, value = value))
        }
    }

    /** Deletes one recorded value and hands it back for Undo. */
    suspend fun deleteValue(id: Long): AccountValueEntity? {
        val row = db.values().get(id) ?: return null
        db.values().delete(id)
        return row
    }

    suspend fun restoreValue(value: AccountValueEntity) { db.values().insertAll(listOf(value)) }

    suspend fun transaction(id: Long): TransactionEntity? = transactions.get(id)
    suspend fun account(id: Long): AccountEntity? = accounts.get(id)
    suspend fun category(id: Long): CategoryEntity? = categories.get(id)
    suspend fun noteSuggestions(prefix: String): List<String> =
        if (prefix.isBlank()) emptyList() else transactions.noteSuggestions(TextMatch.prefixPattern(prefix))
    suspend fun lastCategoryForNote(note: String): Long? = transactions.lastCategoryForNote(note.trim())
    suspend fun entriesForAccount(id: Long): Int = transactions.countForAccount(id)
    suspend fun entriesForCategory(id: Long): Int = transactions.countForCategory(id)

    /** Everything [deleteAccount] would take or change, read in one transaction so the counts agree. */
    suspend fun accountUse(id: Long): AccountUse = db.withTransaction {
        val links = transactions.transfersWith(id).associateBy { it.otherId }
        AccountUse(
            entries = transactions.countForAccount(id),
            bills = db.recurring().countForAccount(id),
            // In the accounts' own order; deleting the transfers undoes what they added.
            others = accounts.all().mapNotNull { a ->
                links[a.id]?.let { OtherAccountChange(a.name, it.count, -it.net) }
            },
        )
    }

    /** Everything [deleteCategory] would move or remove. */
    suspend fun categoryUse(id: Long): CategoryUse = db.withTransaction {
        CategoryUse(
            entries = transactions.countForCategory(id),
            bills = db.recurring().countForCategory(id),
            budget = budgets.getFor(id)?.amount,
        )
    }

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

    /** Several new accounts in one write, in order: a failure adds none of them. Their ids, same order. */
    suspend fun saveNewAccounts(list: List<AccountEntity>): List<Long> = db.withTransaction { list.map { saveAccount(it) } }

    suspend fun saveAccount(account: AccountEntity): Long =
        if (account.id == 0L) accounts.insert(account.copy(name = account.name.trim(), sortOrder = accounts.nextSortOrder()))
        else { accounts.update(account.copy(name = account.name.trim())); account.id }

    /**
     * Removes the account and, through the foreign keys, every entry that touches it (transfers
     * with other accounts included, so their balances move) and every bill paying from or into it.
     * [accountUse] counts all of that first, for the dialog that asks.
     */
    suspend fun deleteAccount(id: Long) {
        db.withTransaction {
            // Its goals read everything from now on; its values go with it through the foreign key.
            db.goals().forgetAccount(id)
            accounts.get(id)?.let { accounts.delete(it) }
        }
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

    /**
     * Undo for a category deleted with nothing filed under it: the row comes back with its id, and
     * its budget with it. (With entries or bills moved there is no exact way back, so no Undo.)
     */
    suspend fun restoreCategory(category: CategoryEntity, budget: Long?) {
        db.withTransaction {
            categories.insert(category)
            if (budget != null && budget > 0L) budgets.setAmount(category.id, budget)
        }
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
