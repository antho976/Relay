package com.tally.app.testing

import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.TransactionRow
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import com.tally.core.TxType
import java.time.LocalDate

/** Deterministic demo data for screenshot tests. Dated around [TODAY] so goldens never drift. */
object Fixtures {
    val TODAY: LocalDate = LocalDate.of(2026, 10, 14)
    val PERIOD: BudgetPeriod = BudgetPeriod.containing(TODAY)

    fun row(
        id: Long,
        note: String,
        amount: Long,
        daysAgo: Long = 0,
        type: TxType = TxType.EXPENSE,
        category: String? = "Groceries",
        color: Int? = 0,
        icon: String? = "cart",
        account: String = "Visa",
        toAccount: String? = null,
    ) = TransactionRow(
        id = id, type = type, amount = amount, date = TODAY.minusDays(daysAgo), note = note,
        accountId = 2, accountName = account, toAccountId = if (toAccount != null) 1 else null, toAccountName = toAccount,
        categoryId = if (type == TxType.TRANSFER) null else id % 5 + 1, categoryName = if (type == TxType.TRANSFER) null else category,
        categoryColor = if (type == TxType.TRANSFER) null else color, categoryIcon = if (type == TxType.TRANSFER) null else icon,
        recurringId = null,
    )

    val rows: List<TransactionRow> = listOf(
        row(1, "Metro", 6_842, 0),
        row(2, "Café Olimpico", 465, 0, category = "Dining", color = 6, icon = "dining"),
        row(3, "OPUS refill", 9_400, 1, category = "Transport", color = 2, icon = "transport", account = "Chequing"),
        row(4, "Salary", 215_000, 2, type = TxType.INCOME, category = "Salary", color = 0, icon = "work", account = "Chequing"),
        row(5, "Card payment", 90_000, 3, type = TxType.TRANSFER, account = "Chequing", toAccount = "Visa"),
        row(6, "Winter boots, the warm ones with the long laces", 15_999, 4, category = "Shopping", color = 4, icon = "bag"),
        row(7, "Pho Lien", 2_310, 5, category = "Dining", color = 6, icon = "dining"),
    )

    val accounts: List<AccountBalance> = listOf(
        AccountBalance(1, "Chequing", AccountType.CHEQUING, 240_000, false, 0, 318_450, 40),
        AccountBalance(2, "Visa", AccountType.CREDIT, 0, false, 1, -41_220, 61),
        AccountBalance(3, "Savings", AccountType.SAVINGS, 650_000, false, 2, 725_000, 3),
        AccountBalance(4, "Cash", AccountType.CASH, 12_000, false, 3, 7_350, 9),
    )
}
