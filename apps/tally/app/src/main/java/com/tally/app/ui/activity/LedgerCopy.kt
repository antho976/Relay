package com.tally.app.ui.activity

import com.tally.app.data.db.TransactionRow
import com.tally.core.AccountType
import com.tally.core.TxType

// The words the ledger screens build from their numbers. Pure, so a JVM test holds them.

/** What an entry is called on screen, the same rule the ledger row uses for its title. */
internal fun entryTitle(row: TransactionRow): String = when {
    row.note.isNotBlank() -> row.note
    row.type == TxType.TRANSFER -> "Transfer"
    else -> row.categoryName ?: "Uncategorized"
}

internal fun accountTypeLabel(type: AccountType): String = when (type) {
    AccountType.CASH -> "Cash"
    AccountType.CHEQUING -> "Chequing"
    AccountType.SAVINGS -> "Savings"
    AccountType.CREDIT -> "Credit card"
    AccountType.INVESTMENT -> "Investment"
}

/** The noun the largest and average readings speak of. */
internal fun basisNoun(type: TxType): String = when (type) {
    TxType.EXPENSE -> "spend"
    TxType.INCOME -> "income"
    TxType.TRANSFER -> "transfer"
}

/** "$3,016 more in than out", read from a set's two totals. */
internal fun netLine(spent: Long, income: Long, format: (Long) -> String): String = when {
    spent == 0L && income == 0L -> "Nothing in or out"
    income >= spent -> format(income - spent) + " more in than out"
    else -> format(spent - income) + " more out than in"
}
