package com.margin.core

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
    val accounts: List<AccountDto> = emptyList(),
    val categories: List<CategoryDto> = emptyList(),
    val transactions: List<TransactionDto> = emptyList(),
    val budgets: List<BudgetDto> = emptyList(),
    val recurring: List<RecurringDto> = emptyList(),
    val goals: List<GoalDto> = emptyList(),
    val contributions: List<ContributionDto> = emptyList(),
) {
    companion object {
        const val FORMAT = "margin-backup"
        const val VERSION = 1
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

    fun encode(file: BackupFile): String = json.encodeToString(BackupFile.serializer(), file)

    fun decode(text: String): BackupReadResult {
        val file = try {
            json.decodeFromString(BackupFile.serializer(), text)
        } catch (e: Exception) {
            return BackupReadResult.Invalid("This file is not a Margin backup.")
        }
        if (file.format != BackupFile.FORMAT) return BackupReadResult.Invalid("This file is not a Margin backup.")
        if (file.version > BackupFile.VERSION) {
            return BackupReadResult.Invalid("This backup is from a newer version of Margin. Update the app, then restore.")
        }
        return validate(file)
    }

    /** References must resolve and amounts must be sane, or the restore would write orphans. */
    fun validate(file: BackupFile): BackupReadResult {
        val accounts = file.accounts.map { it.id }.toSet()
        val categories = file.categories.map { it.id }.toSet()
        val goals = file.goals.map { it.id }.toSet()
        file.transactions.forEach { t ->
            if (t.amount < 0) return BackupReadResult.Invalid("A transaction has a negative amount.")
            if (t.accountId !in accounts) return BackupReadResult.Invalid("A transaction points at an account that is not in the file.")
            if (t.toAccountId != null && t.toAccountId !in accounts) return BackupReadResult.Invalid("A transfer points at an account that is not in the file.")
            if (t.categoryId != null && t.categoryId !in categories) return BackupReadResult.Invalid("A transaction points at a category that is not in the file.")
            if (runCatching { java.time.LocalDate.parse(t.date) }.isFailure) return BackupReadResult.Invalid("A transaction has an unreadable date.")
        }
        file.contributions.forEach { c ->
            if (c.goalId !in goals) return BackupReadResult.Invalid("A contribution points at a goal that is not in the file.")
        }
        file.budgets.forEach { b ->
            if (b.categoryId != null && b.categoryId !in categories) return BackupReadResult.Invalid("A budget points at a category that is not in the file.")
        }
        return BackupReadResult.Ok(file)
    }
}
