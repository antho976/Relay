package com.margin.app.data

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.margin.app.data.db.AccountEntity
import com.margin.app.data.db.CategoryEntity
import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.TransactionEntity
import com.margin.app.data.repo.LedgerRepository
import com.margin.app.data.repo.PlanRepository
import com.margin.app.data.repo.RecurringPoster
import com.margin.core.AccountType
import com.margin.core.CategoryKind
import com.margin.core.TxType
import java.time.LocalDate
import java.time.ZoneOffset

/**
 * A fresh in-memory [MarginDatabase] for one test: the real Room schema on SQLite, foreign keys
 * on, nothing written to disk. Close it in `@After`.
 */
object RoomTestDb {
    fun create(): MarginDatabase =
        Room.inMemoryDatabaseBuilder(ApplicationProvider.getApplicationContext<Context>(), MarginDatabase::class.java)
            .allowMainThreadQueries()
            .build()
}

/** "Today" pinned to [date], and movable, so catch-up and resume can be driven day by day. */
class FixedClock(var date: LocalDate) : Clock {
    override fun today(): LocalDate = date
    override fun nowMillis(): Long = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
}

// ── Repositories wired the way Hilt wires them, over one test database ───────────────────────

fun MarginDatabase.ledgerRepository(clock: Clock): LedgerRepository =
    LedgerRepository(this, transactions(), accounts(), categories(), budgets(), clock)

fun MarginDatabase.planRepository(clock: Clock): PlanRepository =
    PlanRepository(budgets(), recurring(), goals(), clock)

fun MarginDatabase.recurringPoster(clock: Clock): RecurringPoster =
    RecurringPoster(this, recurring(), transactions(), clock)

// ── Row builders: one line per fact a test sets up ───────────────────────────────────────────

suspend fun MarginDatabase.addAccount(
    name: String,
    type: AccountType = AccountType.CHEQUING,
    opening: Long = 0,
    archived: Boolean = false,
    sortOrder: Int = 0,
): Long = accounts().insert(
    AccountEntity(name = name, type = type, openingBalance = opening, archived = archived, sortOrder = sortOrder)
)

suspend fun MarginDatabase.addCategory(
    name: String,
    kind: CategoryKind = CategoryKind.EXPENSE,
    color: Int = 0,
    icon: String = "dots",
    sortOrder: Int = 0,
): Long = categories().insert(
    CategoryEntity(name = name, kind = kind, color = color, icon = icon, sortOrder = sortOrder)
)

suspend fun MarginDatabase.addExpense(
    amount: Long,
    date: LocalDate,
    accountId: Long,
    categoryId: Long? = null,
    note: String = "",
): Long = transactions().insert(
    TransactionEntity(type = TxType.EXPENSE, amount = amount, date = date, accountId = accountId, categoryId = categoryId, note = note)
)

suspend fun MarginDatabase.addIncome(
    amount: Long,
    date: LocalDate,
    accountId: Long,
    categoryId: Long? = null,
    note: String = "",
): Long = transactions().insert(
    TransactionEntity(type = TxType.INCOME, amount = amount, date = date, accountId = accountId, categoryId = categoryId, note = note)
)

suspend fun MarginDatabase.addTransfer(
    amount: Long,
    date: LocalDate,
    fromAccountId: Long,
    toAccountId: Long,
    note: String = "",
): Long = transactions().insert(
    TransactionEntity(type = TxType.TRANSFER, amount = amount, date = date, accountId = fromAccountId, toAccountId = toAccountId, note = note)
)
