package com.tally.app.data.db

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import androidx.room.Update
import androidx.room.Upsert
import com.tally.core.CategoryKind
import com.tally.core.TxType
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate

private const val ROW_SELECT = """
    SELECT t.id, t.type, t.amount, t.date, t.note, t.accountId, a.name AS accountName,
           t.toAccountId, ta.name AS toAccountName, t.categoryId, c.name AS categoryName,
           c.color AS categoryColor, c.icon AS categoryIcon, t.recurringId
    FROM transactions t
    JOIN accounts a ON a.id = t.accountId
    LEFT JOIN accounts ta ON ta.id = t.toAccountId
    LEFT JOIN categories c ON c.id = t.categoryId
"""

/**
 * The Activity search's filters, shared by its rows, its totals and its largest entry so the three
 * always describe the same set. Every filter is optional (NULL, or an empty pattern, skips it).
 * [pattern] is a GLOB pattern from TextMatch: GLOB with a set per letter folds case for every
 * alphabet, where LIKE folds ASCII only and would miss "Épicerie" for "épicerie".
 */
private const val LEDGER_WHERE = """
    WHERE (:start IS NULL OR t.date >= :start) AND (:end IS NULL OR t.date < :end)
      AND (:type IS NULL OR t.type = :type)
      AND (:categoryId IS NULL OR t.categoryId = :categoryId)
      AND (:accountId IS NULL OR t.accountId = :accountId OR t.toAccountId = :accountId)
      AND (:pattern = '' OR t.note GLOB :pattern OR c.name GLOB :pattern OR a.name GLOB :pattern)
"""

@Dao
interface TransactionDao {

    @Insert suspend fun insert(tx: TransactionEntity): Long
    @Insert suspend fun insertAll(tx: List<TransactionEntity>)
    @Update suspend fun update(tx: TransactionEntity)
    @Update suspend fun updateAll(tx: List<TransactionEntity>)
    @Query("DELETE FROM transactions WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM transactions WHERE id = :id") suspend fun get(id: Long): TransactionEntity?

    @Query("$ROW_SELECT ORDER BY t.date DESC, t.id DESC LIMIT :limit")
    fun observeRecent(limit: Int): Flow<List<TransactionRow>>

    @Query("$ROW_SELECT WHERE t.date >= :start AND t.date < :end ORDER BY t.date DESC, t.id DESC")
    fun observeBetween(start: LocalDate, end: LocalDate): Flow<List<TransactionRow>>

    /**
     * The Activity search's rows. Capped, because a search over years of entries renders the
     * newest few hundred and says how many it is showing; the readings come from [searchTotals]
     * and [searchLargest], which are not capped.
     */
    @Query(
        """$ROW_SELECT
        $LEDGER_WHERE
        ORDER BY t.date DESC, t.id DESC
        LIMIT :limit"""
    )
    fun search(
        pattern: String,
        type: TxType?,
        categoryId: Long?,
        accountId: Long?,
        start: LocalDate?,
        end: LocalDate?,
        limit: Int,
    ): Flow<List<TransactionRow>>

    /** What EVERY entry the search matches adds up to, however many the capped list shows. */
    @Query(
        """SELECT COUNT(*) AS count,
                  COALESCE(SUM(CASE WHEN t.type = 'EXPENSE' THEN t.amount END), 0) AS spent,
                  COALESCE(SUM(CASE WHEN t.type = 'INCOME' THEN t.amount END), 0) AS income,
                  COALESCE(SUM(CASE WHEN t.type = 'TRANSFER' THEN t.amount END), 0) AS moved,
                  COUNT(CASE WHEN t.type = 'EXPENSE' THEN 1 END) AS expenses,
                  COUNT(CASE WHEN t.type = 'INCOME' THEN 1 END) AS incomes,
                  COUNT(CASE WHEN t.type = 'TRANSFER' THEN 1 END) AS transfers,
                  COUNT(DISTINCT t.date) AS activeDays
           FROM transactions t
           JOIN accounts a ON a.id = t.accountId
           LEFT JOIN categories c ON c.id = t.categoryId
           $LEDGER_WHERE"""
    )
    fun searchTotals(
        pattern: String,
        type: TxType?,
        categoryId: Long?,
        accountId: Long?,
        start: LocalDate?,
        end: LocalDate?,
    ): Flow<LedgerTotals>

    /**
     * The largest entry the search matches, of the type the readings are about: spending when
     * there is any, else income, else transfers. Ordering by that rank first means the top row is
     * the largest of the first type present. Ties go to the newest, as the list shows them.
     */
    @Query(
        """$ROW_SELECT
        $LEDGER_WHERE
        ORDER BY CASE t.type WHEN 'EXPENSE' THEN 0 WHEN 'INCOME' THEN 1 ELSE 2 END,
                 t.amount DESC, t.date DESC, t.id DESC
        LIMIT 1"""
    )
    fun searchLargest(
        pattern: String,
        type: TxType?,
        categoryId: Long?,
        accountId: Long?,
        start: LocalDate?,
        end: LocalDate?,
    ): Flow<TransactionRow?>

    @Query(
        """SELECT categoryId, SUM(amount) AS total, COUNT(*) AS count FROM transactions
           WHERE type = :type AND date >= :start AND date < :end GROUP BY categoryId"""
    )
    fun observeByCategory(type: TxType, start: LocalDate, end: LocalDate): Flow<List<CategoryTotal>>

    @Query(
        """SELECT date, SUM(amount) AS total FROM transactions
           WHERE type = 'EXPENSE' AND date >= :start AND date < :end GROUP BY date ORDER BY date"""
    )
    fun observeDailyExpense(start: LocalDate, end: LocalDate): Flow<List<DayTotal>>

    @Query(
        """SELECT type, SUM(amount) AS total FROM transactions
           WHERE type != 'TRANSFER' AND date >= :start AND date < :end GROUP BY type"""
    )
    fun observeTypeTotals(start: LocalDate, end: LocalDate): Flow<List<TypeTotal>>

    @Query("SELECT COUNT(*) FROM transactions") fun observeCount(): Flow<Int>
    @Query("SELECT COUNT(*) FROM transactions") suspend fun count(): Int
    @Query("SELECT MIN(date) FROM transactions") suspend fun firstDate(): LocalDate?

    @Query("SELECT * FROM transactions ORDER BY date, id") suspend fun all(): List<TransactionEntity>

    @Query("$ROW_SELECT ORDER BY t.date, t.id") suspend fun allRows(): List<TransactionRow>

    /**
     * Notes used before, most frequent first, for the editor's suggestions. [pattern] is a GLOB
     * prefix pattern from TextMatch, so "é" suggests "Épicerie" as "m" suggests "Metro".
     */
    @Query(
        """SELECT note FROM transactions WHERE note != '' AND note GLOB :pattern
           GROUP BY note ORDER BY COUNT(*) DESC, MAX(date) DESC LIMIT 5"""
    )
    suspend fun noteSuggestions(pattern: String): List<String>

    /** The category last used with a note, so typing "Metro" picks Groceries. */
    @Query("SELECT categoryId FROM transactions WHERE note = :note AND categoryId IS NOT NULL ORDER BY date DESC, id DESC LIMIT 1")
    suspend fun lastCategoryForNote(note: String): Long?

    @Query("SELECT COUNT(*) FROM transactions WHERE accountId = :accountId OR toAccountId = :accountId")
    suspend fun countForAccount(accountId: Long): Int

    @Query("SELECT COUNT(*) FROM transactions WHERE categoryId = :categoryId")
    suspend fun countForCategory(categoryId: Long): Int

    /**
     * The transfers between [accountId] and each other account: how many, and what they added to
     * that other account's balance (below zero when it sent more than it got).
     */
    @Query(
        """SELECT CASE WHEN accountId = :accountId THEN toAccountId ELSE accountId END AS otherId,
                  COUNT(*) AS count,
                  SUM(CASE WHEN accountId = :accountId THEN amount ELSE -amount END) AS net
           FROM transactions
           WHERE type = 'TRANSFER' AND toAccountId IS NOT NULL AND accountId != toAccountId
             AND (accountId = :accountId OR toAccountId = :accountId)
           GROUP BY otherId"""
    )
    suspend fun transfersWith(accountId: Long): List<TransferLink>

    @Query("UPDATE transactions SET categoryId = :to WHERE categoryId = :from")
    suspend fun moveCategory(from: Long, to: Long)

    /** Every transfer between [start] and [end]: what the invest goals count, by the accounts at each end. */
    @Query(
        """SELECT date, amount, accountId, toAccountId FROM transactions
           WHERE type = 'TRANSFER' AND toAccountId IS NOT NULL AND date >= :start AND date < :end"""
    )
    fun observeTransfers(start: LocalDate, end: LocalDate): Flow<List<TransferRow>>

    /**
     * The entries logged most often since [since], by note and category, each with the amount it
     * was last logged at: the editor's one-tap quick picks. Archived categories are left out.
     */
    @Query(
        """SELECT t.note AS note, t.categoryId AS categoryId, COUNT(*) AS uses,
                  (SELECT t2.amount FROM transactions t2
                   WHERE t2.note = t.note AND t2.type = :type AND t2.categoryId = t.categoryId
                   ORDER BY t2.date DESC, t2.id DESC LIMIT 1) AS lastAmount,
                  c.name AS categoryName, c.icon AS categoryIcon, c.color AS categoryColor
           FROM transactions t JOIN categories c ON c.id = t.categoryId
           WHERE t.type = :type AND t.note != '' AND t.date >= :since AND c.archived = 0
           GROUP BY t.note, t.categoryId
           HAVING COUNT(*) >= 2
           ORDER BY uses DESC, MAX(t.date) DESC
           LIMIT :limit"""
    )
    fun observeQuickPicks(type: TxType, since: LocalDate, limit: Int): Flow<List<QuickPick>>

    /** Every entry that moves [accountId] between [start] and [end]: what an import checks for lines already logged. */
    @Query(
        """SELECT * FROM transactions
           WHERE (accountId = :accountId OR toAccountId = :accountId) AND date >= :start AND date <= :end"""
    )
    suspend fun touching(accountId: Long, start: LocalDate, end: LocalDate): List<TransactionEntity>

    /** Notes and the category they were filed under since [since]: what an import learns payees from. */
    @Query("SELECT note, categoryId, type FROM transactions WHERE note != '' AND categoryId IS NOT NULL AND date >= :since")
    suspend fun notesSince(since: LocalDate): List<NoteUse>

    /** Every entry's effect on balances, oldest first: what net worth over time is rebuilt from. */
    @Query("SELECT date, type, amount, accountId, toAccountId FROM transactions ORDER BY date, id")
    fun observeFlows(): Flow<List<FlowRow>>

    /** Where the money went by payee (the note): totals of [type] between [start] and [end], largest first. */
    @Query(
        """SELECT note, SUM(amount) AS total, COUNT(*) AS count FROM transactions
           WHERE type = :type AND note != '' AND date >= :start AND date < :end
           GROUP BY note ORDER BY total DESC LIMIT :limit"""
    )
    fun observePayees(type: TxType, start: LocalDate, end: LocalDate, limit: Int): Flow<List<PayeeTotal>>

    @Query("DELETE FROM transactions") suspend fun deleteAll()
}

@Dao
interface AccountDao {
    @Insert suspend fun insert(account: AccountEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(accounts: List<AccountEntity>)
    @Update suspend fun update(account: AccountEntity)
    @Update suspend fun updateAll(accounts: List<AccountEntity>)
    @Delete suspend fun delete(account: AccountEntity)
    @Query("SELECT * FROM accounts WHERE id = :id") suspend fun get(id: Long): AccountEntity?
    @Query("SELECT * FROM accounts ORDER BY archived, sortOrder, id") fun observeAll(): Flow<List<AccountEntity>>
    @Query("SELECT * FROM accounts ORDER BY archived, sortOrder, id") suspend fun all(): List<AccountEntity>
    @Query("SELECT COUNT(*) FROM accounts") suspend fun count(): Int
    @Query("SELECT COALESCE(MAX(sortOrder), -1) + 1 FROM accounts") suspend fun nextSortOrder(): Int

    /**
     * Opening balance, plus income, minus expenses and outgoing transfers, plus incoming ones. An
     * account with a recorded value starts from its newest value instead, and counts only the
     * entries dated after it: the value already holds everything up to its day.
     */
    @Query(
        """SELECT a.id, a.name, a.type, a.openingBalance, a.archived, a.sortOrder,
              COALESCE(v.value, a.openingBalance)
              + COALESCE((SELECT SUM(CASE WHEN t.type = 'INCOME' THEN t.amount ELSE -t.amount END)
                          FROM transactions t WHERE t.accountId = a.id AND (v.date IS NULL OR t.date > v.date)), 0)
              + COALESCE((SELECT SUM(t.amount) FROM transactions t
                          WHERE t.type = 'TRANSFER' AND t.toAccountId = a.id AND (v.date IS NULL OR t.date > v.date)), 0) AS balance,
              (SELECT COUNT(*) FROM transactions t WHERE t.accountId = a.id OR t.toAccountId = a.id) AS entryCount,
              v.date AS valuedOn
           FROM accounts a
           LEFT JOIN account_values v ON v.id = (
               SELECT v2.id FROM account_values v2 WHERE v2.accountId = a.id ORDER BY v2.date DESC, v2.id DESC LIMIT 1
           )
           ORDER BY a.archived, a.sortOrder, a.id"""
    )
    fun observeBalances(): Flow<List<AccountBalance>>

    @Query("DELETE FROM accounts") suspend fun deleteAll()
}

@Dao
interface CategoryDao {
    @Insert suspend fun insert(category: CategoryEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(categories: List<CategoryEntity>)
    @Update suspend fun update(category: CategoryEntity)
    @Query("DELETE FROM categories WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM categories WHERE id = :id") suspend fun get(id: Long): CategoryEntity?
    @Query("SELECT * FROM categories ORDER BY kind, archived, sortOrder, id") fun observeAll(): Flow<List<CategoryEntity>>
    @Query("SELECT * FROM categories ORDER BY kind, archived, sortOrder, id") suspend fun all(): List<CategoryEntity>
    @Query("SELECT * FROM categories WHERE kind = :kind AND archived = 0 ORDER BY sortOrder, id")
    fun observeActive(kind: CategoryKind): Flow<List<CategoryEntity>>
    @Query("SELECT COUNT(*) FROM categories") suspend fun count(): Int
    @Query("SELECT COALESCE(MAX(sortOrder), -1) + 1 FROM categories WHERE kind = :kind") suspend fun nextSortOrder(kind: CategoryKind): Int
    /** NOCASE folds ASCII only ("groceries" finds "Groceries", "épicerie" misses "Épicerie"). */
    @Query("SELECT * FROM categories WHERE name = :name COLLATE NOCASE AND kind = :kind LIMIT 1")
    suspend fun findByName(name: String, kind: CategoryKind): CategoryEntity?
    @Query(
        """SELECT c.*, (SELECT COUNT(*) FROM transactions t WHERE t.categoryId = c.id) AS entryCount
           FROM categories c ORDER BY c.kind, c.archived, c.sortOrder, c.id"""
    )
    fun observeWithCounts(): Flow<List<CategoryWithCount>>

    @Query("DELETE FROM categories") suspend fun deleteAll()
}

@Dao
interface BudgetDao {
    @Upsert suspend fun upsert(budget: BudgetEntity)
    @Update suspend fun updateAll(budgets: List<BudgetEntity>)
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(budgets: List<BudgetEntity>)
    @Query("DELETE FROM budgets WHERE categoryId = :categoryId") suspend fun deleteFor(categoryId: Long)
    @Query("SELECT * FROM budgets WHERE categoryId = :categoryId") suspend fun getFor(categoryId: Long): BudgetEntity?

    /**
     * Sets [categoryId]'s limit. The read and the write are one transaction: apart, two quick
     * saves for a new budget both read "none", and the second upsert lands on id 0 and vanishes.
     * (SQLite's own UPSERT clause is 3.24+, newer than the platform SQLite on API 26 to 29.)
     */
    @Transaction
    suspend fun setAmount(categoryId: Long, amount: Long) {
        val existing = getFor(categoryId)
        upsert(BudgetEntity(id = existing?.id ?: 0, categoryId = categoryId, amount = amount))
    }
    @Query(
        """SELECT b.id, b.categoryId, b.amount, c.name AS categoryName, c.color AS categoryColor, c.icon AS categoryIcon
           FROM budgets b LEFT JOIN categories c ON c.id = b.categoryId
           ORDER BY b.categoryId != 0, c.sortOrder"""
    )
    fun observeAll(): Flow<List<BudgetRow>>
    @Query("SELECT * FROM budgets") suspend fun all(): List<BudgetEntity>
    @Query("SELECT COUNT(*) FROM budgets") suspend fun count(): Int
    @Query("DELETE FROM budgets") suspend fun deleteAll()
}

@Dao
interface RecurringDao {
    @Insert suspend fun insert(r: RecurringEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(r: List<RecurringEntity>)
    @Update suspend fun update(r: RecurringEntity)
    @Update suspend fun updateAll(r: List<RecurringEntity>)
    @Query("DELETE FROM recurring WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM recurring WHERE id = :id") suspend fun get(id: Long): RecurringEntity?
    @Query("SELECT * FROM recurring") suspend fun all(): List<RecurringEntity>

    @Query(
        """SELECT r.*, a.name AS accountName, ta.name AS toAccountName, c.name AS categoryName,
                  c.color AS categoryColor, c.icon AS categoryIcon
           FROM recurring r JOIN accounts a ON a.id = r.accountId
           LEFT JOIN accounts ta ON ta.id = r.toAccountId
           LEFT JOIN categories c ON c.id = r.categoryId
           ORDER BY r.active DESC, r.nextDate, r.id"""
    )
    fun observeAll(): Flow<List<RecurringRow>>

    @Query("SELECT * FROM recurring WHERE active = 1 AND autoPost = 1 AND nextDate <= :today")
    suspend fun due(today: LocalDate): List<RecurringEntity>

    /** Running reminders whose date is over. They never post; the poster only moves them on. */
    @Query("SELECT * FROM recurring WHERE active = 1 AND autoPost = 0 AND nextDate < :today")
    suspend fun remindersBehind(today: LocalDate): List<RecurringEntity>

    /** The latest date entered against bill [id]: the entries it posted, and the one a repeat was made from. */
    @Query("SELECT MAX(date) FROM transactions WHERE recurringId = :id")
    suspend fun lastPosted(id: Long): LocalDate?

    /** Re-files bills when their category is deleted into another one. */
    @Query("UPDATE recurring SET categoryId = :to WHERE categoryId = :from")
    suspend fun moveCategory(from: Long, to: Long)

    /** Bills that pay from or into the account; the foreign key deletes them with it. */
    @Query("SELECT COUNT(*) FROM recurring WHERE accountId = :accountId OR toAccountId = :accountId")
    suspend fun countForAccount(accountId: Long): Int

    /** Bills filed under the category; they post uncategorized once it is deleted, unless moved. */
    @Query("SELECT COUNT(*) FROM recurring WHERE categoryId = :categoryId")
    suspend fun countForCategory(categoryId: Long): Int

    @Query("SELECT COUNT(*) FROM recurring") suspend fun count(): Int

    @Query("DELETE FROM recurring") suspend fun deleteAll()
}

@Dao
interface GoalDao {
    @Insert suspend fun insert(goal: GoalEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(goals: List<GoalEntity>)
    @Update suspend fun update(goal: GoalEntity)
    @Update suspend fun updateAll(goals: List<GoalEntity>)
    @Update suspend fun updateContributions(c: List<ContributionEntity>)
    @Query("DELETE FROM goals WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM goals WHERE id = :id") suspend fun get(id: Long): GoalEntity?
    @Query("SELECT * FROM goals") suspend fun all(): List<GoalEntity>
    @Query("SELECT COUNT(*) FROM goals") suspend fun count(): Int

    @Query(
        """SELECT g.*, COALESCE((SELECT SUM(amount) FROM goal_contributions WHERE goalId = g.id), 0) AS saved,
                  (SELECT MIN(date) FROM goal_contributions WHERE goalId = g.id) AS firstDate
           FROM goals g ORDER BY g.archived, g.targetDate IS NULL, g.targetDate, g.id"""
    )
    fun observeAll(): Flow<List<GoalWithSaved>>

    @Insert suspend fun insertContribution(c: ContributionEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertContributions(c: List<ContributionEntity>)
    @Query("DELETE FROM goal_contributions WHERE id = :id") suspend fun deleteContribution(id: Long)
    @Query("SELECT * FROM goal_contributions WHERE goalId = :goalId ORDER BY date DESC, id DESC")
    fun observeContributions(goalId: Long): Flow<List<ContributionEntity>>
    @Query("SELECT * FROM goal_contributions") suspend fun allContributions(): List<ContributionEntity>
    @Query("SELECT * FROM goal_contributions WHERE goalId = :goalId") suspend fun contributionsFor(goalId: Long): List<ContributionEntity>
    @Query("SELECT * FROM goal_contributions WHERE id = :id") suspend fun contribution(id: Long): ContributionEntity?

    @Query("DELETE FROM goal_contributions") suspend fun deleteAllContributions()
    @Query("DELETE FROM goals") suspend fun deleteAll()

    /** A deleted account's goals read everything from then on, rather than an account that is gone. */
    @Query("UPDATE goals SET accountId = NULL WHERE accountId = :accountId")
    suspend fun forgetAccount(accountId: Long)
}

@Dao
interface AccountValueDao {
    @Insert suspend fun insert(value: AccountValueEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(values: List<AccountValueEntity>)
    @Update suspend fun updateAll(values: List<AccountValueEntity>)
    @Update suspend fun update(value: AccountValueEntity)
    @Query("DELETE FROM account_values WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM account_values WHERE accountId = :accountId AND date = :date LIMIT 1")
    suspend fun on(accountId: Long, date: LocalDate): AccountValueEntity?
    @Query("SELECT * FROM account_values WHERE id = :id") suspend fun get(id: Long): AccountValueEntity?
    @Query("SELECT * FROM account_values WHERE accountId = :accountId ORDER BY date DESC, id DESC")
    fun observeFor(accountId: Long): Flow<List<AccountValueEntity>>
    @Query("SELECT * FROM account_values ORDER BY date, id") fun observeAll(): Flow<List<AccountValueEntity>>
    @Query("SELECT * FROM account_values ORDER BY date, id") suspend fun all(): List<AccountValueEntity>
    @Query("DELETE FROM account_values") suspend fun deleteAll()
}
