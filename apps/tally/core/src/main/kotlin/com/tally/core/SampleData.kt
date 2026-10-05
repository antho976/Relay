package com.tally.core

import java.time.LocalDate
import kotlin.random.Random

/**
 * A synthetic household for testing the app with something in it: three months of plausible
 * spending, a salary, rent and a few subscriptions. Deterministic for a given [today] and seed, so
 * screenshots and tests see the same month every time. Every account it creates is named
 * "Sample ..." so the data can never pass for the owner's own.
 */
object SampleData {

    const val ACCOUNT_PREFIX = "Sample"

    fun build(today: LocalDate, fractionDigits: Int, currency: String, seed: Int = 7): BackupFile {
        val rnd = Random(seed)
        val unit = MoneyFormatter.pow10(fractionDigits)
        fun money(major: Double): Long = Math.round(major * unit)

        val accounts = listOf(
            AccountDto(1, "$ACCOUNT_PREFIX chequing", AccountType.CHEQUING, money(2_400.0), sortOrder = 0),
            AccountDto(2, "$ACCOUNT_PREFIX Visa", AccountType.CREDIT, 0, sortOrder = 1),
            AccountDto(3, "$ACCOUNT_PREFIX savings", AccountType.SAVINGS, money(6_500.0), sortOrder = 2),
            AccountDto(4, "$ACCOUNT_PREFIX cash", AccountType.CASH, money(120.0), sortOrder = 3),
            AccountDto(5, "$ACCOUNT_PREFIX TFSA", AccountType.INVESTMENT, money(8_200.0), sortOrder = 4),
        )
        val categories = Defaults.categories.mapIndexed { i, c ->
            CategoryDto(id = i + 1L, name = c.name, kind = c.kind, color = c.color, icon = c.icon, sortOrder = i)
        }
        fun cat(name: String) = categories.first { it.name == name }.id

        val start = today.withDayOfMonth(1).minusMonths(2)
        val tx = ArrayList<TransactionDto>()
        var id = 1L
        fun add(type: TxType, amount: Long, date: LocalDate, account: Long, category: Long?, note: String, to: Long? = null, recurring: Long? = null) {
            if (date.isAfter(today)) return
            tx += TransactionDto(id++, type, amount, date.toString(), account, to, category, note, recurring)
        }

        val groceries = listOf("Metro", "IGA", "Provigo", "Marché Jean-Talon", "Costco")
        val dining = listOf("Café Olimpico", "Pho Lien", "Lunch with Sam", "Burger night", "Bagels", "Thai takeout")
        val transport = listOf("OPUS refill", "Bixi", "Taxi home", "Gas")
        val shopping = listOf("Winter boots", "Hardware store", "Books", "Kitchen stuff", "Phone case")

        var d = start
        while (!d.isAfter(today)) {
            // Groceries twice a week, dining most days, the rest scattered.
            if (d.dayOfWeek.value == 2 || d.dayOfWeek.value == 6) {
                add(TxType.EXPENSE, money(45.0 + rnd.nextInt(0, 95) + rnd.nextInt(0, 99) / 100.0), d, 2, cat("Groceries"), groceries.random(rnd))
            }
            if (rnd.nextFloat() < 0.55f) {
                add(TxType.EXPENSE, money(8.0 + rnd.nextInt(0, 42) + rnd.nextInt(0, 99) / 100.0), d, if (rnd.nextBoolean()) 2 else 4, cat("Dining"), dining.random(rnd))
            }
            if (rnd.nextFloat() < 0.25f) {
                add(TxType.EXPENSE, money(3.5 + rnd.nextInt(0, 60)), d, 2, cat("Transport"), transport.random(rnd))
            }
            if (rnd.nextFloat() < 0.10f) {
                add(TxType.EXPENSE, money(20.0 + rnd.nextInt(0, 140) + 0.99), d, 2, cat("Shopping"), shopping.random(rnd))
            }
            if (rnd.nextFloat() < 0.06f) {
                add(TxType.EXPENSE, money(15.0 + rnd.nextInt(0, 60)), d, 2, cat("Entertainment"), listOf("Cinema", "Concert", "Board game cafe").random(rnd))
            }
            if (rnd.nextFloat() < 0.04f) {
                add(TxType.EXPENSE, money(12.0 + rnd.nextInt(0, 80)), d, 2, cat("Health"), listOf("Pharmacy", "Physio").random(rnd))
            }
            d = d.plusDays(1)
        }

        val recurring = ArrayList<RecurringDto>()
        fun bill(rid: Long, name: String, type: TxType, amount: Long, account: Long, category: Long?, freq: Frequency, interval: Int, anchor: LocalDate, to: Long? = null) {
            val rule = Recurrence(anchor, freq, interval)
            rule.between(anchor, today).forEach { date -> add(type, amount, date, account, category, name, to, rid) }
            recurring += RecurringDto(
                id = rid, name = name, type = type, amount = amount, accountId = account, toAccountId = to,
                categoryId = category, frequency = freq, interval = interval, anchorDate = anchor.toString(),
                nextDate = rule.after(today).toString(),
            )
        }
        bill(1, "Rent", TxType.EXPENSE, money(1_350.0), 1, cat("Housing"), Frequency.MONTHLY, 1, start)
        bill(2, "Hydro-Québec", TxType.EXPENSE, money(78.40), 1, cat("Utilities"), Frequency.MONTHLY, 1, start.plusDays(17))
        bill(3, "Phone plan", TxType.EXPENSE, money(45.0), 2, cat("Phone & internet"), Frequency.MONTHLY, 1, start.plusDays(9))
        bill(4, "Internet", TxType.EXPENSE, money(60.0), 2, cat("Phone & internet"), Frequency.MONTHLY, 1, start.plusDays(21))
        bill(5, "Music streaming", TxType.EXPENSE, money(11.99), 2, cat("Subscriptions"), Frequency.MONTHLY, 1, start.plusDays(4))
        bill(6, "Gym", TxType.EXPENSE, money(39.0), 2, cat("Health"), Frequency.MONTHLY, 1, start.plusDays(13))
        bill(7, "Salary", TxType.INCOME, money(2_150.0), 1, cat("Salary"), Frequency.WEEKLY, 2, start.plusDays(4))
        bill(8, "Card payment", TxType.TRANSFER, money(900.0), 1, null, Frequency.MONTHLY, 1, start.plusDays(24), to = 2)
        bill(9, "To savings", TxType.TRANSFER, money(250.0), 1, null, Frequency.MONTHLY, 1, start.plusDays(5), to = 3)
        bill(10, "To TFSA", TxType.TRANSFER, money(300.0), 1, null, Frequency.MONTHLY, 1, start.plusDays(6), to = 5)

        val budgets = listOf(
            BudgetDto(1, null, money(2_900.0)),
            BudgetDto(2, cat("Groceries"), money(500.0)),
            BudgetDto(3, cat("Dining"), money(320.0)),
            BudgetDto(4, cat("Transport"), money(120.0)),
            BudgetDto(5, cat("Shopping"), money(150.0)),
            BudgetDto(6, cat("Entertainment"), money(80.0)),
        )
        val goals = listOf(
            GoalDto(1, "Emergency fund", money(10_000.0), today.plusMonths(14).withDayOfMonth(1).toString(), color = 1),
            GoalDto(2, "Lisbon trip", money(2_400.0), today.plusMonths(6).withDayOfMonth(1).toString(), color = 6),
            GoalDto(3, "Invest a tenth of pay", 0, color = 2, kind = GoalKind.INVEST, percent = 10),
            GoalDto(
                4, "Worth 25k", money(25_000.0), today.plusMonths(12).withDayOfMonth(1).toString(), color = 7,
                kind = GoalKind.BALANCE, startDate = start.toString(), startAmount = money(17_220.0),
            ),
        )
        // The TFSA's value as read off the statement once, a little above what was put in.
        val values = listOf(AccountValueDto(1, 5, start.plusMonths(1).plusDays(27).toString(), money(9_050.0)))
            .filter { !LocalDate.parse(it.date).isAfter(today) }
        val contributions = listOf(
            ContributionDto(1, 1, money(3_200.0), start.toString(), "Starting balance"),
            ContributionDto(2, 1, money(250.0), start.plusMonths(1).plusDays(5).toString()),
            ContributionDto(3, 2, money(400.0), start.plusDays(12).toString()),
            ContributionDto(4, 2, money(300.0), start.plusMonths(1).plusDays(12).toString()),
        ).filter { !LocalDate.parse(it.date).isAfter(today) }

        return BackupFile(
            exportedAt = today.toString(),
            currency = currency,
            accounts = accounts,
            categories = categories,
            transactions = tx.sortedBy { it.date },
            budgets = budgets,
            recurring = recurring,
            goals = goals,
            contributions = contributions,
            values = values,
        )
    }
}
