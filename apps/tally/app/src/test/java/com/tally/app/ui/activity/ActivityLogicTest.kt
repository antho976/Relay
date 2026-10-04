package com.tally.app.ui.activity

import com.tally.app.data.db.LedgerTotals
import com.tally.app.data.db.TransactionRow
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class ActivityLogicTest {

    private val today = LocalDate.of(2026, 10, 14)

    private fun row(
        id: Long,
        amount: Long,
        daysAgo: Long,
        type: TxType = TxType.EXPENSE,
        account: Long = 2,
        to: Long? = null,
    ) = TransactionRow(
        id = id,
        type = type,
        amount = amount,
        date = today.minusDays(daysAgo),
        note = "",
        accountId = account,
        accountName = "Account $account",
        toAccountId = to,
        toAccountName = to?.let { "Account $it" },
        categoryId = null,
        categoryName = null,
        categoryColor = null,
        categoryIcon = null,
        recurringId = null,
    )

    @Test fun noRowsMakeNoDays() {
        assertTrue(groupByDay(emptyList()).isEmpty())
    }

    @Test fun groupsOneDayEachNewestFirstKeepingRowOrder() {
        val rows = listOf(row(5, 100, 0), row(4, 200, 0), row(3, 300, 2), row(2, 400, 2), row(1, 500, 3))
        val days = groupByDay(rows)
        assertEquals(listOf(today, today.minusDays(2), today.minusDays(3)), days.map { it.date })
        assertEquals(listOf(5L, 4L), days[0].rows.map { it.id })
        assertEquals(listOf(3L, 2L), days[1].rows.map { it.id })
    }

    @Test fun dayTotalsSplitOutAndInAndLeaveTransfersOut() {
        val rows = listOf(
            row(1, 1_000, 0),
            row(2, 50_000, 0, TxType.INCOME),
            row(3, 9_000, 0, TxType.TRANSFER, account = 1, to = 2),
            row(4, 250, 0),
        )
        val day = groupByDay(rows).single()
        assertEquals(1_250L, day.spent)
        assertEquals(50_000L, day.income)
    }

    @Test fun anEmptySetSummarisesToZero() {
        val s = summarize(emptyList())
        assertEquals(0, s.count)
        assertEquals(0L, s.spent)
        assertEquals(0L, s.average)
        assertNull(s.largest)
    }

    @Test fun summaryTotalsEachTypeAndReadsLargestAndAverageFromSpending() {
        val rows = listOf(
            row(1, 1_000, 0),
            row(2, 215_000, 1, TxType.INCOME),
            row(3, 90_000, 1, TxType.TRANSFER, account = 1, to = 2),
            row(4, 3_000, 4),
            row(5, 2_000, 4),
        )
        val s = summarize(rows)
        assertEquals(5, s.count)
        assertEquals(6_000L, s.spent)
        assertEquals(215_000L, s.income)
        assertEquals(90_000L, s.moved)
        assertEquals(1, s.transfers)
        assertEquals(3, s.activeDays)
        assertEquals(TxType.EXPENSE, s.basis)
        assertEquals(3, s.basisCount)
        // Neither the salary nor the card payment stands in as the largest spend.
        assertEquals(4L, s.largest?.id)
        assertEquals(2_000L, s.average)
        assertEquals(209_000L, s.net)
    }

    @Test fun theSummaryReadsTheUncappedTotalsNotTheShownRows() {
        // 1,100 matches, of which the screen lists 400: every reading comes from the SQL totals.
        val totals = LedgerTotals(
            count = 1_100, spent = 2_700_000, income = 0, moved = 0,
            expenses = 1_100, incomes = 0, transfers = 0, activeDays = 700,
        )
        val s = summaryOf(totals, largest = row(9, 240_000, 600))
        assertEquals(1_100, s.count)
        assertEquals(2_700_000L, s.spent)
        assertEquals(1_100, s.basisCount)
        assertEquals(2_454L, s.average)
        assertEquals(700, s.activeDays)
        assertEquals("An old purchase past the cap still reads as the largest", 9L, s.largest?.id)
    }

    @Test fun theSummaryPicksItsBasisFromTheTotalsAndDropsALargestOfAnotherType() {
        val incomeOnly = LedgerTotals(count = 2, spent = 0, income = 150_000, moved = 0, expenses = 0, incomes = 2, transfers = 0, activeDays = 2)
        val s = summaryOf(incomeOnly, largest = row(1, 100_000, 0, TxType.INCOME))
        assertEquals(TxType.INCOME, s.basis)
        assertEquals(75_000L, s.average)
        assertEquals(1L, s.largest?.id)
        assertNull("A largest of the wrong type, from a flow a moment behind, never shows", summaryOf(incomeOnly, row(2, 5_000, 0)).largest)
        assertEquals(LedgerSummary(), summaryOf(LedgerTotals(0, 0, 0, 0, 0, 0, 0, 0), null))
    }

    @Test fun inMemoryTotalsMatchWhatTheSqlCounts() {
        val rows = listOf(row(1, 1_000, 0), row(2, 215_000, 1, TxType.INCOME), row(3, 90_000, 1, TxType.TRANSFER, account = 1, to = 2))
        assertEquals(LedgerTotals(3, 1_000, 215_000, 90_000, 1, 1, 1, 2), totalsOf(rows))
    }

    @Test fun withoutSpendingTheReadingsFallBackToIncomeThenTransfers() {
        val income = summarize(listOf(row(1, 100_000, 0, TxType.INCOME), row(2, 50_000, 1, TxType.INCOME)))
        assertEquals(TxType.INCOME, income.basis)
        assertEquals(75_000L, income.average)
        assertEquals(1L, income.largest?.id)

        val moved = summarize(listOf(row(3, 9_000, 0, TxType.TRANSFER, account = 1, to = 2)))
        assertEquals(TxType.TRANSFER, moved.basis)
        assertEquals(9_000L, moved.largest?.amount)
    }

    @Test fun accountFlowsReadTransfersByTheirDirection() {
        val rows = listOf(
            row(1, 1_000, 0, account = 2),
            row(2, 215_000, 0, TxType.INCOME, account = 2),
            row(3, 90_000, 0, TxType.TRANSFER, account = 1, to = 2),
            row(4, 5_000, 0, TxType.TRANSFER, account = 2, to = 3),
            row(5, 7_000, 0, account = 1),
        )
        val f = accountFlows(rows, accountId = 2)
        assertEquals(305_000L, f.moneyIn)
        assertEquals(6_000L, f.moneyOut)
        assertEquals(2, f.inCount)
        assertEquals(2, f.outCount)
    }

    @Test fun netLineSaysWhichSideIsAhead() {
        val fmt: (Long) -> String = { it.toString() }
        assertEquals("Nothing in or out", netLine(0, 0, fmt))
        assertEquals("700 more in than out", netLine(300, 1_000, fmt))
        assertEquals("200 more out than in", netLine(500, 300, fmt))
    }
}
