package com.margin.app.data.db

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import androidx.room.Update
import androidx.room.Upsert
import com.margin.core.CategoryKind
import com.margin.core.TxType
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

@Dao
interface TransactionDao {

    @Insert suspend fun insert(tx: TransactionEntity): Long
    @Insert suspend fun insertAll(tx: List<TransactionEntity>)
    @Update suspend fun update(tx: TransactionEntity)
    @Query("DELETE FROM transactions WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM transactions WHERE id = :id") suspend fun get(id: Long): TransactionEntity?

    @Query("$ROW_SELECT ORDER BY t.date DESC, t.id DESC LIMIT :limit")
    fun observeRecent(limit: Int): Flow<List<TransactionRow>>

    @Query("$ROW_SELECT WHERE t.date >= :start AND t.date < :end ORDER BY t.date DESC, t.id DESC")
    fun observeBetween(start: LocalDate, end: LocalDate): Flow<List<TransactionRow>>

    /**
     * The Activity search. Every filter is optional (NULL skips it). Capped, because a search over
     * years of entries renders the newest few hundred and says how many it is showing.
     */
    @Query(
        """$ROW_SELECT
        WHERE (:start IS NULL OR t.date >= :start) AND (:end IS NULL OR t.date < :end)
          AND (:type IS NULL OR t.type = :type)
          AND (:categoryId IS NULL OR t.categoryId = :categoryId)
          AND (:accountId IS NULL OR t.accountId = :accountId OR t.toAccountId = :accountId)
          AND (:query = '' OR t.note LIKE '%' || :query || '%' OR c.name LIKE '%' || :query || '%'
               OR a.name LIKE '%' || :query || '%')
        ORDER BY t.date DESC, t.id DESC
        LIMIT :limit"""
    )
    fun search(
        query: String,
        type: TxType?,
        categoryId: Long?,
        accountId: Long?,
        start: LocalDate?,
        end: LocalDate?,
        limit: Int,
    ): Flow<List<TransactionRow>>

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

    /** Notes used before, most frequent first, for the editor's suggestions. */
    @Query(
        """SELECT note FROM transactions WHERE note != '' AND note LIKE :prefix || '%'
           GROUP BY note ORDER BY COUNT(*) DESC, MAX(date) DESC LIMIT 5"""
    )
    suspend fun noteSuggestions(prefix: String): List<String>

    /** The category last used with a note, so typing "Metro" picks Groceries. */
    @Query("SELECT categoryId FROM transactions WHERE note = :note AND categoryId IS NOT NULL ORDER BY date DESC, id DESC LIMIT 1")
    suspend fun lastCategoryForNote(note: String): Long?

    @Query("SELECT COUNT(*) FROM transactions WHERE accountId = :accountId OR toAccountId = :accountId")
    suspend fun countForAccount(accountId: Long): Int

    @Query("SELECT COUNT(*) FROM transactions WHERE categoryId = :categoryId")
    suspend fun countForCategory(categoryId: Long): Int

    @Query("UPDATE transactions SET categoryId = :to WHERE categoryId = :from")
    suspend fun moveCategory(from: Long, to: Long)

    @Query("DELETE FROM transactions") suspend fun deleteAll()
}

@Dao
interface AccountDao {
    @Insert suspend fun insert(account: AccountEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(accounts: List<AccountEntity>)
    @Update suspend fun update(account: AccountEntity)
    @Delete suspend fun delete(account: AccountEntity)
    @Query("SELECT * FROM accounts WHERE id = :id") suspend fun get(id: Long): AccountEntity?
    @Query("SELECT * FROM accounts ORDER BY archived, sortOrder, id") fun observeAll(): Flow<List<AccountEntity>>
    @Query("SELECT * FROM accounts ORDER BY archived, sortOrder, id") suspend fun all(): List<AccountEntity>
    @Query("SELECT COUNT(*) FROM accounts") suspend fun count(): Int
    @Query("SELECT COALESCE(MAX(sortOrder), -1) + 1 FROM accounts") suspend fun nextSortOrder(): Int

    /** Opening balance, plus income, minus expenses and outgoing transfers, plus incoming ones. */
    @Query(
        """SELECT a.id, a.name, a.type, a.openingBalance, a.archived, a.sortOrder,
              a.openingBalance
              + COALESCE((SELECT SUM(CASE WHEN t.type = 'INCOME' THEN t.amount ELSE -t.amount END)
                          FROM transactions t WHERE t.accountId = a.id), 0)
              + COALESCE((SELECT SUM(t.amount) FROM transactions t
                          WHERE t.type = 'TRANSFER' AND t.toAccountId = a.id), 0) AS balance,
              (SELECT COUNT(*) FROM transactions t WHERE t.accountId = a.id OR t.toAccountId = a.id) AS entryCount
           FROM accounts a ORDER BY a.archived, a.sortOrder, a.id"""
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
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(budgets: List<BudgetEntity>)
    @Query("DELETE FROM budgets WHERE categoryId = :categoryId") suspend fun deleteFor(categoryId: Long)
    @Query("SELECT * FROM budgets WHERE categoryId = :categoryId") suspend fun getFor(categoryId: Long): BudgetEntity?
    @Query(
        """SELECT b.id, b.categoryId, b.amount, c.name AS categoryName, c.color AS categoryColor, c.icon AS categoryIcon
           FROM budgets b LEFT JOIN categories c ON c.id = b.categoryId
           ORDER BY b.categoryId != 0, c.sortOrder"""
    )
    fun observeAll(): Flow<List<BudgetRow>>
    @Query("SELECT * FROM budgets") suspend fun all(): List<BudgetEntity>
    @Query("DELETE FROM budgets") suspend fun deleteAll()
}

@Dao
interface RecurringDao {
    @Insert suspend fun insert(r: RecurringEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(r: List<RecurringEntity>)
    @Update suspend fun update(r: RecurringEntity)
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

    @Query("DELETE FROM recurring") suspend fun deleteAll()
}

@Dao
interface GoalDao {
    @Insert suspend fun insert(goal: GoalEntity): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun insertAll(goals: List<GoalEntity>)
    @Update suspend fun update(goal: GoalEntity)
    @Query("DELETE FROM goals WHERE id = :id") suspend fun delete(id: Long)
    @Query("SELECT * FROM goals WHERE id = :id") suspend fun get(id: Long): GoalEntity?
    @Query("SELECT * FROM goals") suspend fun all(): List<GoalEntity>

    @Query(
        """SELECT g.*, COALESCE((SELECT SUM(amount) FROM goal_contributions WHERE goalId = g.id), 0) AS saved
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
}
