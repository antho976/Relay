package com.tally.app.ui.insights

import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.FlowRow
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate

class WorthLogicTest {

    private val today = LocalDate.of(2026, 10, 14)
    private val current = BudgetPeriod.containing(today)

    private fun account(id: Long, type: AccountType, opening: Long, archived: Boolean = false) =
        AccountBalance(id, "Account $id", type, opening, archived, id.toInt(), balance = 0, entryCount = 0)

    private val chequing = account(1, AccountType.CHEQUING, 100_000)
    private val visa = account(2, AccountType.CREDIT, 0)
    private val tfsa = account(3, AccountType.INVESTMENT, 500_000)

    private val flows = listOf(
        FlowRow(LocalDate.of(2026, 9, 1), TxType.INCOME, 300_000, 1, null),
        FlowRow(LocalDate.of(2026, 9, 10), TxType.EXPENSE, 40_000, 2, null),
        FlowRow(LocalDate.of(2026, 9, 25), TxType.TRANSFER, 40_000, 1, 2),
        FlowRow(LocalDate.of(2026, 10, 2), TxType.TRANSFER, 50_000, 1, 3),
        FlowRow(LocalDate.of(2026, 10, 5), TxType.EXPENSE, 12_000, 2, null),
    )

    @Test fun balancesAtADayCountOnlyTheEntriesUpToIt() {
        val sep30 = balancesAt(listOf(chequing, visa, tfsa), emptyList(), flows, LocalDate.of(2026, 9, 30))
        assertEquals(100_000L + 300_000 - 40_000, sep30[1])
        assertEquals(0L, sep30[2])
        assertEquals(500_000L, sep30[3])

        val now = balancesAt(listOf(chequing, visa, tfsa), emptyList(), flows, today)
        assertEquals(360_000L - 50_000, now[1])
        assertEquals(-12_000L, now[2])
        assertEquals(550_000L, now[3])
    }

    @Test fun aRecordedValueReplacesEverythingBeforeIt() {
        // The TFSA was valued at 6,000 on 3 Oct: the transfer on the 2nd is inside that value.
        val values = listOf(AccountValueEntity(1, 3, LocalDate.of(2026, 10, 3), 600_000))
        val now = balancesAt(listOf(tfsa), values, flows, today)
        assertEquals(600_000L, now[3])
        // Before the value's day, the account reads from its opening as before.
        val before = balancesAt(listOf(tfsa), values, flows, LocalDate.of(2026, 10, 2))
        assertEquals(550_000L, before[3])
    }

    @Test fun archivedAccountsAreLeftOut() {
        val closed = account(4, AccountType.SAVINGS, 99_000, archived = true)
        assertEquals(setOf(1L), balancesAt(listOf(chequing, closed), emptyList(), emptyList(), today).keys)
    }

    @Test fun theReadingSplitsHeldFromOwedAndCountsWhatWasInvested() {
        val w = worthReading(listOf(chequing, visa, tfsa), emptyList(), flows, current, 0, today)
        assertEquals(310_000L - 12_000 + 550_000, w.worth)
        assertEquals(360_000L + 0 + 500_000, w.startWorth)
        assertEquals(860_000L, w.assets)
        assertEquals(12_000L, w.debts)
        assertEquals(50_000L, w.invested)
        assertEquals(550_000L, w.investments)
        assertEquals(1, w.investmentAccounts)
        assertEquals(WORTH_PERIODS, w.points.size)
        assertEquals(w.worth, w.points.last().worth)
        // Held largest first, then what is owed.
        assertEquals(listOf(3L, 1L, 2L), w.accounts.map { it.id })
    }

    @Test fun aPastPeriodClosesOnItsLastDay() {
        val w = worthReading(listOf(chequing, visa, tfsa), emptyList(), flows, current, -1, today)
        assertEquals(360_000L + 500_000, w.worth)
        assertEquals(100_000L + 500_000, w.startWorth)
        assertEquals(0L, w.invested)
    }

    @Test fun keptShareReadsAgainstIncome() {
        assertEquals(25, keptShare(400_000, 300_000))
        assertEquals(-50, keptShare(100_000, 150_000))
        assertNull(keptShare(0, 10_000))
    }

    @Test fun aRowNeverSaysItsTypeTwice() {
        assertEquals("", worthAccountMeta("Chequing", "Chequing", null))
        assertEquals("valued 9 Oct", worthAccountMeta(" chequing ", "Chequing", "9 Oct"))
        assertEquals("Investment · valued 9 Oct", worthAccountMeta("Wealthsimple TFSA", "Investment", "9 Oct"))
        assertEquals("Credit card", worthAccountMeta("Visa", "Credit card", null))
    }
}
