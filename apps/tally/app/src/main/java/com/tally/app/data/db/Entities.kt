package com.tally.app.data.db

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.TxType
import java.time.LocalDate

// Every amount is a Long of minor units (cents). Enums are stored by name; renaming a constant
// is a migration, and release keeps enum names (proguard-rules.pro).

@Entity(tableName = "accounts")
data class AccountEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val type: AccountType,
    val openingBalance: Long = 0,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
)

@Entity(tableName = "categories", indices = [Index("kind")])
data class CategoryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val kind: CategoryKind,
    /** Index into the twelve-hue category palette. */
    val color: Int,
    val icon: String,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
)

@Entity(
    tableName = "transactions",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["toAccountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = CategoryEntity::class, parentColumns = ["id"], childColumns = ["categoryId"], onDelete = ForeignKey.SET_NULL),
        ForeignKey(entity = RecurringEntity::class, parentColumns = ["id"], childColumns = ["recurringId"], onDelete = ForeignKey.SET_NULL),
    ],
    indices = [Index("date"), Index("accountId"), Index("toAccountId"), Index("categoryId"), Index("recurringId"), Index(value = ["type", "date"])],
)
data class TransactionEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val type: TxType,
    /** Always positive; [type] carries the direction. */
    val amount: Long,
    val date: LocalDate,
    val accountId: Long,
    /** The receiving account of a transfer; null otherwise. */
    val toAccountId: Long? = null,
    /** Null for transfers, and for entries whose category was deleted. */
    val categoryId: Long? = null,
    val note: String = "",
    val recurringId: Long? = null,
    @ColumnInfo(defaultValue = "0") val createdAt: Long = 0,
)

/**
 * A standing monthly limit. [categoryId] 0 is the overall budget: a real column value rather than
 * NULL, because SQLite lets any number of NULLs through a unique index and there must be one.
 */
@Entity(tableName = "budgets", indices = [Index(value = ["categoryId"], unique = true)])
data class BudgetEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val categoryId: Long,
    val amount: Long,
) {
    companion object { const val OVERALL = 0L }
}

@Entity(
    tableName = "recurring",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["toAccountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = CategoryEntity::class, parentColumns = ["id"], childColumns = ["categoryId"], onDelete = ForeignKey.SET_NULL),
    ],
    indices = [Index("accountId"), Index("toAccountId"), Index("categoryId"), Index("nextDate")],
)
data class RecurringEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val type: TxType,
    val amount: Long,
    val accountId: Long,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val frequency: Frequency,
    val interval: Int = 1,
    val anchorDate: LocalDate,
    /** The next date this posts on; advanced when it posts. */
    val nextDate: LocalDate,
    val endDate: LocalDate? = null,
    /** Off means it is a reminder: shown as upcoming, never written to the ledger on its own. */
    val autoPost: Boolean = true,
    val active: Boolean = true,
)

/**
 * A goal. [kind] says what it measures (see [GoalKind]); the columns past [archived] came with
 * version 2 and default to a plain savings goal, which is what every version 1 goal was.
 */
@Entity(tableName = "goals")
data class GoalEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    /** The amount to reach; for a monthly goal set as a share of income, unused (0). */
    val target: Long,
    val targetDate: LocalDate? = null,
    val color: Int = 0,
    val archived: Boolean = false,
    @ColumnInfo(defaultValue = "SAVINGS") val kind: GoalKind = GoalKind.SAVINGS,
    /**
     * The account a balance goal reads, or the one an invest goal counts money into. Null reads
     * everything: the net of every open account, or every investment account. Not a foreign key:
     * deleting the account clears it ([GoalDao.forgetAccount]).
     */
    val accountId: Long? = null,
    /** A monthly goal's share of what came in, 1 to 100; 0 when [target] is a set amount a month. */
    @ColumnInfo(defaultValue = "0") val percent: Int = 0,
    /** When a balance goal was set, and what it read then: the start of its pace. */
    val startDate: LocalDate? = null,
    @ColumnInfo(defaultValue = "0") val startAmount: Long = 0,
)

@Entity(
    tableName = "goal_contributions",
    foreignKeys = [ForeignKey(entity = GoalEntity::class, parentColumns = ["id"], childColumns = ["goalId"], onDelete = ForeignKey.CASCADE)],
    indices = [Index("goalId")],
)
data class ContributionEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val goalId: Long,
    /** Negative for a withdrawal. */
    val amount: Long,
    val date: LocalDate,
    val note: String = "",
)

/**
 * What an investment account was worth on [date], as the owner read it off a statement. The
 * newest one stands in for the opening balance and every entry dated on or before it, so market
 * moves land in the balance without passing for income or spending.
 */
@Entity(
    tableName = "account_values",
    foreignKeys = [ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE)],
    indices = [Index(value = ["accountId", "date"])],
)
data class AccountValueEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val accountId: Long,
    val date: LocalDate,
    val value: Long,
)
