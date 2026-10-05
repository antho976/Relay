package com.tally.core

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * The JSON backup: the whole database as one human-readable file. Ids are kept so references
 * survive a restore; dates are ISO strings ("2026-10-04") so the file reads without a decoder.
 */
@Serializable
data class BackupFile(
    val format: String = FORMAT,
    val version: Int = VERSION,
    val exportedAt: String,
    val currency: String,
    /**
     * The day the budget month starts on, so a payday month survives a restore. Null when the
     * file does not say (the sample, or a file from before it was kept): a restore leaves the
     * phone's own setting alone then.
     */
    val monthStartDay: Int? = null,
    /** Whether weeks start on Monday; null when the file does not say, as for [monthStartDay]. */
    val weekStartsMonday: Boolean? = null,
    val accounts: List<AccountDto> = emptyList(),
    val categories: List<CategoryDto> = emptyList(),
    val transactions: List<TransactionDto> = emptyList(),
    val budgets: List<BudgetDto> = emptyList(),
    val recurring: List<RecurringDto> = emptyList(),
    val goals: List<GoalDto> = emptyList(),
    val contributions: List<ContributionDto> = emptyList(),
    /** What investment accounts were worth on a day, as typed by the owner. Absent before version 2. */
    val values: List<AccountValueDto> = emptyList(),
) {
    companion object {
        const val FORMAT = "tally-backup"

        /** 2 added goal kinds, investment accounts and their values. A version 1 file still reads. */
        const val VERSION = 2
    }
}

@Serializable
data class AccountDto(
    val id: Long,
    val name: String,
    val type: AccountType,
    val openingBalance: Long,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
)

@Serializable
data class CategoryDto(
    val id: Long,
    val name: String,
    val kind: CategoryKind,
    val color: Int,
    val icon: String,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
)

@Serializable
data class TransactionDto(
    val id: Long,
    val type: TxType,
    val amount: Long,
    val date: String,
    val accountId: Long,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val note: String = "",
    val recurringId: Long? = null,
)

@Serializable
data class BudgetDto(
    val id: Long,
    /** Null is the overall monthly budget. */
    val categoryId: Long? = null,
    val amount: Long,
)

@Serializable
data class RecurringDto(
    val id: Long,
    val name: String,
    val type: TxType,
    val amount: Long,
    val accountId: Long,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val frequency: Frequency,
    val interval: Int = 1,
    val anchorDate: String,
    val nextDate: String,
    val endDate: String? = null,
    val autoPost: Boolean = true,
    val active: Boolean = true,
)

@Serializable
data class GoalDto(
    val id: Long,
    val name: String,
    val target: Long,
    val targetDate: String? = null,
    val color: Int = 0,
    val archived: Boolean = false,
    val kind: GoalKind = GoalKind.SAVINGS,
    /** The account a balance or invest goal reads; null reads every account (or every investment one). */
    val accountId: Long? = null,
    /** A monthly goal's share of what came in, 1 to 100; 0 means [target] is a set amount a month. */
    val percent: Int = 0,
    /** Where a balance goal started from: the day it was set, and what it read then. */
    val startDate: String? = null,
    val startAmount: Long = 0,
)

@Serializable
data class AccountValueDto(
    val id: Long,
    val accountId: Long,
    val date: String,
    val value: Long,
)

@Serializable
data class ContributionDto(
    val id: Long,
    val goalId: Long,
    val amount: Long,
    val date: String,
    val note: String = "",
)

sealed interface BackupReadResult {
    data class Ok(val file: BackupFile) : BackupReadResult
    /** [reason] is shown to the owner as-is, so it names the problem and not the parser. */
    data class Invalid(val reason: String) : BackupReadResult
}

object BackupCodec {

    private val json = Json {
        prettyPrint = true
        encodeDefaults = true
        ignoreUnknownKeys = true
    }

    /** The intervals [Recurrence] accepts; a bill outside them would throw where it is read. */
    private val BILL_INTERVALS = 1..52

    fun encode(file: BackupFile): String = json.encodeToString(BackupFile.serializer(), file)

    fun decode(text: String): BackupReadResult {
        val file = try {
            json.decodeFromString(BackupFile.serializer(), text)
        } catch (e: Exception) {
            return BackupReadResult.Invalid("This file is not a Tally backup.")
        }
        if (file.format != BackupFile.FORMAT) return BackupReadResult.Invalid("This file is not a Tally backup.")
        if (file.version > BackupFile.VERSION) {
            return BackupReadResult.Invalid("This backup is from a newer version of Tally. Update the app, then restore.")
        }
        return validate(file)
    }

    /**
     * References must resolve and values must be ones the app itself could have written, or the
     * restore would write orphans, or rows that crash the screen or the poster that reads them.
     * The file is human-readable and may have been edited by hand, so nothing is taken on trust.
     */
    fun validate(file: BackupFile): BackupReadResult {
        val accounts = file.accounts.map { it.id }.toSet()
        val categories = file.categories.map { it.id }.toSet()
        val goals = file.goals.map { it.id }.toSet()
        val bills = file.recurring.map { it.id }.toSet()
        fun readable(date: String?) = date == null || runCatching { java.time.LocalDate.parse(date) }.isSuccess
        file.transactions.forEach { t ->
            if (t.amount < 0) return BackupReadResult.Invalid("A transaction has a negative amount.")
            if (t.accountId !in accounts) return BackupReadResult.Invalid("A transaction points at an account that is not in the file.")
            if (t.toAccountId != null && t.toAccountId !in accounts) return BackupReadResult.Invalid("A transfer points at an account that is not in the file.")
            if (t.categoryId != null && t.categoryId !in categories) return BackupReadResult.Invalid("A transaction points at a category that is not in the file.")
            if (t.recurringId != null && t.recurringId !in bills) return BackupReadResult.Invalid("A transaction points at a bill that is not in the file.")
            if (!readable(t.date)) return BackupReadResult.Invalid("A transaction has an unreadable date.")
        }
        file.recurring.forEach { r ->
            if (r.amount <= 0) return BackupReadResult.Invalid("A bill has an amount of zero or less.")
            if (r.interval !in BILL_INTERVALS) {
                return BackupReadResult.Invalid("A bill repeats at an interval outside ${BILL_INTERVALS.first} to ${BILL_INTERVALS.last}.")
            }
            if (r.accountId !in accounts) return BackupReadResult.Invalid("A bill points at an account that is not in the file.")
            if (r.toAccountId != null && r.toAccountId !in accounts) return BackupReadResult.Invalid("A bill points at an account that is not in the file.")
            if (r.type == TxType.TRANSFER && (r.toAccountId == null || r.toAccountId == r.accountId)) {
                return BackupReadResult.Invalid("A transfer bill needs two different accounts.")
            }
            if (r.categoryId != null && r.categoryId !in categories) return BackupReadResult.Invalid("A bill points at a category that is not in the file.")
            if (!readable(r.anchorDate) || !readable(r.nextDate) || !readable(r.endDate)) {
                return BackupReadResult.Invalid("A bill has an unreadable date.")
            }
        }
        file.goals.forEach { g ->
            if (g.target < 0) return BackupReadResult.Invalid("A goal has a negative target.")
            if (!readable(g.targetDate) || !readable(g.startDate)) return BackupReadResult.Invalid("A goal has an unreadable date.")
            if (g.percent !in 0..100) return BackupReadResult.Invalid("A goal asks for a share outside 0 to 100 percent.")
            if (g.accountId != null && g.accountId !in accounts) return BackupReadResult.Invalid("A goal points at an account that is not in the file.")
        }
        file.values.forEach { v ->
            if (v.accountId !in accounts) return BackupReadResult.Invalid("An account value points at an account that is not in the file.")
            if (!readable(v.date)) return BackupReadResult.Invalid("An account value has an unreadable date.")
        }
        file.contributions.forEach { c ->
            if (c.goalId !in goals) return BackupReadResult.Invalid("A contribution points at a goal that is not in the file.")
            if (!readable(c.date)) return BackupReadResult.Invalid("A contribution has an unreadable date.")
        }
        file.budgets.forEach { b ->
            if (b.amount < 0) return BackupReadResult.Invalid("A budget has a negative amount.")
            if (b.categoryId != null && b.categoryId !in categories) return BackupReadResult.Invalid("A budget points at a category that is not in the file.")
        }
        return BackupReadResult.Ok(file)
    }
}
