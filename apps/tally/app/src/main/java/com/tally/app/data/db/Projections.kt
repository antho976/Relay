package com.tally.app.data.db

import androidx.compose.runtime.Immutable
import androidx.room.Embedded
import com.tally.core.AccountType
import com.tally.core.TxType
import java.time.LocalDate

/** A ledger row with the names it needs to render, joined in SQL rather than per row in Kotlin. */
@Immutable
data class TransactionRow(
    val id: Long,
    val type: TxType,
    val amount: Long,
    val date: LocalDate,
    val note: String,
    val accountId: Long,
    val accountName: String,
    val toAccountId: Long?,
    val toAccountName: String?,
    val categoryId: Long?,
    val categoryName: String?,
    val categoryColor: Int?,
    val categoryIcon: String?,
    val recurringId: Long?,
)

@Immutable
data class CategoryTotal(val categoryId: Long?, val total: Long, val count: Int)

@Immutable
data class DayTotal(val date: LocalDate, val total: Long)

@Immutable
data class TypeTotal(val type: TxType, val total: Long)

@Immutable
data class AccountBalance(
    val id: Long,
    val name: String,
    val type: AccountType,
    val openingBalance: Long,
    val archived: Boolean,
    val sortOrder: Int,
    val balance: Long,
    val entryCount: Int,
)

@Immutable
data class BudgetRow(
    val id: Long,
    val categoryId: Long,
    val amount: Long,
    val categoryName: String?,
    val categoryColor: Int?,
    val categoryIcon: String?,
)

@Immutable
data class RecurringRow(
    @Embedded val recurring: RecurringEntity,
    val accountName: String,
    val toAccountName: String?,
    val categoryName: String?,
    val categoryColor: Int?,
    val categoryIcon: String?,
)

@Immutable
data class CategoryWithCount(
    @Embedded val category: CategoryEntity,
    val entryCount: Int,
)

@Immutable
data class GoalWithSaved(
    @Embedded val goal: GoalEntity,
    val saved: Long,
)
