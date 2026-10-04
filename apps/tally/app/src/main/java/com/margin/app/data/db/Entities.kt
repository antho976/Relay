package com.margin.app.data.db

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey
import com.margin.core.AccountType
import com.margin.core.CategoryKind
import com.margin.core.Frequency
import com.margin.core.TxType
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

@Entity(tableName = "goals")
data class GoalEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val target: Long,
    val targetDate: LocalDate? = null,
    val color: Int = 0,
    val archived: Boolean = false,
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
