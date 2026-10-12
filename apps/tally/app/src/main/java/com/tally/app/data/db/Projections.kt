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

/** What a searched set of entries adds up to, counted in SQL over every match, not the shown rows. */
@Immutable
data class LedgerTotals(
    val count: Int,
    val spent: Long,
    val income: Long,
    val moved: Long,
    val expenses: Int,
    val incomes: Int,
    val transfers: Int,
    val activeDays: Int,
)

@Immutable
data class DayTotal(val date: LocalDate, val total: Long)

@Immutable
data class TypeTotal(val type: TxType, val total: Long)

/** The transfers between one account and [otherId]: how many, and what they added to [otherId]'s balance. */
@Immutable
data class TransferLink(val otherId: Long, val count: Int, val net: Long)

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
    /** The day of the newest recorded value, when the balance starts from one. */
    val valuedOn: LocalDate? = null,
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

/** A goal with what its contributions add up to, and the day of the first one (null with none). */
@Immutable
data class GoalWithSaved(
    @Embedded val goal: GoalEntity,
    val saved: Long,
    /** Where an even saving towards the target date starts: the pace tick's day zero. */
    val firstDate: LocalDate? = null,
)

/** One transfer, by the accounts at each end. */
@Immutable
data class TransferRow(val date: LocalDate, val amount: Long, val accountId: Long, val toAccountId: Long)

/** A transfer as one investment account sees it: [amount] above zero came in, below zero went out. */
@Immutable
data class AccountTransfer(val accountId: Long, val date: LocalDate, val amount: Long)

/** An entry logged often: its note and category, and the amount it was last logged at. */
@Immutable
data class QuickPick(
    val note: String,
    val categoryId: Long,
    val uses: Int,
    val lastAmount: Long,
    val categoryName: String,
    val categoryIcon: String?,
    val categoryColor: Int?,
)

/** A note and the category it was filed under. */
@Immutable
data class NoteUse(val note: String, val categoryId: Long, val type: TxType)

/** One entry's effect on balances. */
@Immutable
data class FlowRow(val date: LocalDate, val type: TxType, val amount: Long, val accountId: Long, val toAccountId: Long?)

/** What one payee (an entry's note) took over a stretch, and how many times. */
@Immutable
data class PayeeTotal(val note: String, val total: Long, val count: Int)
