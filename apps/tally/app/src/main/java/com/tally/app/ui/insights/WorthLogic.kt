package com.tally.app.ui.insights

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.FlowRow
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import com.tally.core.TxType
import java.time.LocalDate

/*
 * The Worth lens's pure half: what every open account held on a given day, rebuilt from its
 * opening balance (or its newest recorded value) and the entries since, and the readings the lens
 * draws from that. No Android, so InsightsLogicTest holds all of it.
 */

/** How many periods the worth chart shows, ending at the one picked. */
internal const val WORTH_PERIODS = 6

/** Net worth at the close of one period: its last day, or today for the one still running. */
@Immutable
data class WorthPoint(val period: BudgetPeriod, val offset: Int, val worth: Long)

/** One open account as the lens lists it. */
@Immutable
data class AccountShare(
    val id: Long,
    val name: String,
    val type: AccountType,
    val balance: Long,
    /** The day of the value the balance starts from, when it starts from one. */
    val valuedOn: LocalDate?,
)

@Immutable
data class WorthReading(
    /** What the open accounts held at the close of the period picked. */
    val worth: Long = 0,
    /** What they held the day before it began. */
    val startWorth: Long = 0,
    /** The accounts in credit, added up. */
    val assets: Long = 0,
    /** What the accounts in debt owe, as a positive figure. */
    val debts: Long = 0,
    /** Moved into investment accounts over the period, less what came back out. */
    val invested: Long = 0,
    /** What the investment accounts hold at the close of the period. */
    val investments: Long = 0,
    val investmentAccounts: Int = 0,
    /** Oldest first, ending at the period picked. */
    val points: List<WorthPoint> = emptyList(),
    /** In credit first, largest first; then the debts, largest first. */
    val accounts: List<AccountShare> = emptyList(),
) {
    val change: Long get() = worth - startWorth
}

/**
 * What each open account held at the end of [day]: its newest value on or before that day (else
 * its opening balance), plus every entry after the value up to and including [day]. The same
 * reckoning as the balances query, read at any day instead of now.
 */
internal fun balancesAt(
    accounts: List<AccountBalance>,
    values: List<AccountValueEntity>,
    flows: List<FlowRow>,
    day: LocalDate,
): Map<Long, Long> {
    val open = accounts.filter { !it.archived }
    val newest = values
        .filter { !it.date.isAfter(day) }
        .groupBy { it.accountId }
        .mapValues { (_, vs) -> vs.maxWith(compareBy<AccountValueEntity>({ it.date }, { it.id })) }
    val held = open.associate { a -> a.id to (newest[a.id]?.value ?: a.openingBalance) }.toMutableMap()
    flows.forEach { f ->
        if (f.date.isAfter(day)) return@forEach
        val from = held[f.accountId]
        if (from != null && newest[f.accountId]?.date?.let { f.date.isAfter(it) } != false) {
            held[f.accountId] = from + if (f.type == TxType.INCOME) f.amount else -f.amount
        }
        val toId = f.toAccountId
        if (f.type == TxType.TRANSFER && toId != null) {
            val to = held[toId]
            if (to != null && newest[toId]?.date?.let { f.date.isAfter(it) } != false) held[toId] = to + f.amount
        }
    }
    return held
}

/** The day a period's reading closes on: its last day, or [today] while it runs. */
internal fun closeOf(period: BudgetPeriod, today: LocalDate): LocalDate =
    if (today in period) today else period.lastDay

/**
 * The Worth lens for [period] ([offset] periods from [current]): net worth at its close and the
 * day before it began, the split between what is held and what is owed, what moved into the
 * investment accounts, and the close of the [WORTH_PERIODS] periods ending at it.
 */
internal fun worthReading(
    accounts: List<AccountBalance>,
    values: List<AccountValueEntity>,
    flows: List<FlowRow>,
    current: BudgetPeriod,
    offset: Int,
    today: LocalDate,
): WorthReading {
    val period = current.shift(offset.toLong())
    val close = closeOf(period, today)
    val atClose = balancesAt(accounts, values, flows, close)
    val atStart = balancesAt(accounts, values, flows, period.start.minusDays(1))
    val open = accounts.filter { !it.archived }
    val investing = open.filter { it.type == AccountType.INVESTMENT }.map { it.id }.toSet()
    val points = ((offset - WORTH_PERIODS + 1)..offset).map { o ->
        val p = current.shift(o.toLong())
        WorthPoint(p, o, balancesAt(accounts, values, flows, closeOf(p, today)).values.sum())
    }
    val newestValue = values.filter { !it.date.isAfter(close) }.groupBy { it.accountId }
        .mapValues { (_, vs) -> vs.maxOf { it.date } }
    val shares = open.map { a -> AccountShare(a.id, a.name, a.type, atClose[a.id] ?: 0L, newestValue[a.id]) }
    return WorthReading(
        worth = atClose.values.sum(),
        startWorth = atStart.values.sum(),
        assets = atClose.values.filter { it > 0L }.sum(),
        debts = -atClose.values.filter { it < 0L }.sum(),
        invested = flows.filter { it.type == TxType.TRANSFER && it.date in period && !it.date.isAfter(close) }.sumOf { f ->
            val toIn = f.toAccountId in investing
            val fromIn = f.accountId in investing
            when {
                toIn && !fromIn -> f.amount
                fromIn && !toIn -> -f.amount
                else -> 0L
            }
        },
        investments = investing.sumOf { atClose[it] ?: 0L },
        investmentAccounts = investing.size,
        points = points,
        accounts = shares.filter { it.balance >= 0L }.sortedByDescending { it.balance } +
            shares.filter { it.balance < 0L }.sortedBy { it.balance },
    )
}

/** The share of [income] that was not spent, in whole percent; null with no income to read it against. */
internal fun keptShare(income: Long, spent: Long): Int? =
    if (income <= 0L) null else Math.round((income - spent) * 100.0 / income).toInt()
