package com.tally.app.ui.invest

import com.tally.app.data.db.AccountBalance
import com.tally.core.AccountType
import com.tally.core.ActivityType
import com.tally.core.Copy
import com.tally.core.IncomeEntry
import com.tally.core.MoneyFormatter
import com.tally.core.Portfolio
import com.tally.core.PortfolioAccount
import com.tally.core.PortfolioHolding
import com.tally.core.Registration
import com.tally.core.RoomLine
import com.tally.core.SecurityKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class InvestLogicTest {

    private val money = MoneyFormatter("CAD", Locale.CANADA)
    private val today = LocalDate.of(2026, 10, 9)
    private val short: (LocalDate) -> String = { "${it.dayOfMonth}/${it.monthValue}" }

    private fun balance(id: Long, name: String, balance: Long, valuedOn: LocalDate? = null, type: AccountType = AccountType.INVESTMENT) =
        AccountBalance(id, name, type, 0L, false, id.toInt(), balance = balance, entryCount = 0, valuedOn = valuedOn)

    private fun account(id: Long, registration: Registration?, value: Long, book: Long, holdings: Int = 1, valuedOn: String? = "2026-10-08") =
        PortfolioAccount(
            id = id, name = "Account $id", registration = registration, institution = "Wealthsimple", value = value, book = book,
            gain = value - book, gainBps = null, cash = 0L, holdings = holdings, valuedOn = valuedOn,
            returnBps = null, returnAnnual = false, returnSince = null,
        )

    private fun holding(accountId: Long, registration: Registration?, symbol: String, kind: SecurityKind, value: Long, book: Long) =
        PortfolioHolding(
            accountId = accountId, account = "Account $accountId", registration = registration, securityId = symbol.hashCode().toLong(),
            symbol = symbol, name = "$symbol fund", kind = kind, currency = "CAD", quantity = 1_250_000_000L, price = null, priceDate = null,
            value = value, book = book, gain = value - book, gainBps = null, weightBps = 0, fxEstimated = false, noPrice = false,
        )

    private fun room(registration: Registration, room: Long?, contributed: Long, over: Long = 0, overTaxed: Long = over) = RoomLine(
        registration = registration, year = 2026, room = room, contributed = contributed, withdrawn = 0L,
        left = room?.let { maxOf(0L, it - contributed) }, over = over, overTaxed = overTaxed,
        deadline = roomDeadline(registration, 2026).toString(), limit = null,
    )

    private fun portfolio(
        accounts: List<PortfolioAccount>,
        holdings: List<PortfolioHolding> = emptyList(),
        income: List<IncomeEntry> = emptyList(),
        roomLines: List<RoomLine> = emptyList(),
    ) = Portfolio(
        currency = "CAD", today = today.toString(), empty = accounts.isEmpty(), asOf = "2026-10-08",
        value = accounts.sumOf { it.value }, book = accounts.sumOf { it.book }, gain = 0L, gainBps = null, cash = 0L,
        income12m = income.sumOf { it.amount }, incomeByMonth = emptyList(), accounts = accounts, allocation = emptyList(),
        kinds = emptyList(), holdings = holdings, income = income, room = roomLines, issues = emptyList(),
    )

    // ── Words ────────────────────────────────────────────────────────────────

    @Test fun percentsKeepTheirSignAndOnePlace() {
        assertEquals("8.1%", percentText(812))
        assertEquals("+8.2%", percentText(815, signed = true))
        assertEquals("−2.0%", percentText(-200))
        assertEquals("−2.0%", percentText(-200, signed = true))
        assertEquals("0.0%", percentText(-3, signed = true))
        assertEquals("18%", sharePercent(1_812))
        assertEquals("under 1%", sharePercent(20))
        assertEquals("0%", sharePercent(0))
    }

    @Test fun unitsReadAsPeopleWriteThem() {
        assertEquals("12.5", quantityText(1_250_000_000L))
        assertEquals("0.0042", quantityText(420_000L))
        assertEquals("0", quantityText(0L))
        assertEquals("12.5 shares", unitsLine(1_250_000_000L, SecurityKind.ETF))
        assertEquals("1 share", unitsLine(100_000_000L, SecurityKind.STOCK))
        assertEquals("0.0042 units", unitsLine(420_000L, SecurityKind.CRYPTO))
        assertEquals("VFV", tileSymbol(" vfv.to "))
        assertEquals("BRK.B", tileSymbol("BRK.B"))
    }

    @Test fun theHeroSaysHowFarFromWhatTheHoldingsCost() {
        assertEquals("$3,240 above what the holdings cost", costLine(324_000, 2_233_600, money))
        assertEquals("$410 below what the holdings cost", costLine(-41_000, 2_233_600, money))
        assertEquals("Level with what the holdings cost", costLine(0, 2_233_600, money))
        assertEquals("No cost on record yet", costLine(0, 0, money))
        assertEquals("+$412 · +8.1%", gainText(41_200, 812, money))
        assertEquals("−$50 · −2.0%", gainText(-5_000, -200, money))
        assertEquals("+$12", gainText(1_200, null, money))
    }

    // ── Room ─────────────────────────────────────────────────────────────────

    @Test fun theRoomTickIsAnEvenPaceToTheDeadline() {
        // TFSA: 1 January to 31 December; 9 October is day 282 of 365.
        assertEquals(282f / 365f, roomPaceFraction(Registration.TFSA, 2026, today), 1e-6f)
        // RRSP 2026: from the day after the 2025 deadline (2 March 2026) to 1 March 2027.
        assertEquals(LocalDate.of(2026, 3, 3), roomStart(Registration.RRSP, 2026))
        assertEquals(LocalDate.of(2027, 3, 1), roomDeadline(Registration.RRSP, 2026))
        assertEquals(1f / 364f, roomPaceFraction(Registration.RRSP, 2026, LocalDate.of(2026, 3, 3)), 1e-6f)
        assertEquals(0f, roomPaceFraction(Registration.RRSP, 2026, LocalDate.of(2026, 1, 10)), 1e-6f)
        assertEquals(1f, roomPaceFraction(Registration.FHSA, 2026, LocalDate.of(2027, 2, 1)), 1e-6f)
    }

    @Test fun aMonthlyPaceCountsThisMonth() {
        assertEquals(93_334L, monthlyToFill(280_000, today, LocalDate.of(2026, 12, 31)))
        assertEquals(93_400L, monthlyToFill(280_000, today, LocalDate.of(2026, 12, 31), unit = 100))
        assertEquals(280_000L, monthlyToFill(280_000, LocalDate.of(2026, 12, 2), LocalDate.of(2026, 12, 31)))
        assertNull(monthlyToFill(0, today, LocalDate.of(2026, 12, 31)))
        assertNull(monthlyToFill(5_000, LocalDate.of(2027, 1, 2), LocalDate.of(2026, 12, 31)))
    }

    @Test fun theRoomLineSaysWhatIsLeftOrWhatBeingOverCosts() {
        assertEquals(
            "$2,800 left · $934 a month uses it by 31/12",
            roomText(room(Registration.TFSA, 700_000, 420_000), today, money, short),
        )
        assertEquals("The 2026 room is used", roomText(room(Registration.TFSA, 700_000, 700_000), today, money, short))
        assertEquals(
            "Over by $300. The CRA charges 1% a month on $300",
            roomText(room(Registration.TFSA, 700_000, 730_000, over = 30_000), today, money, short),
        )
        assertEquals(
            "Over by $1,500, inside the $2,000 an RRSP may go over",
            roomText(room(Registration.RRSP, 2_000_000, 2_150_000, over = 150_000, overTaxed = 0), today, money, short),
        )
        assertEquals("Add your 2026 room from CRA My Account", roomText(room(Registration.FHSA, null, 0), today, money, short))
        assertEquals("$4,200 of $7,000", roomReading(room(Registration.TFSA, 700_000, 420_000), money))
        assertEquals("$4,200 in", roomReading(room(Registration.TFSA, null, 420_000), money))
    }

    // ── The reading ──────────────────────────────────────────────────────────

    @Test fun anAccountWithHoldingsReadsFromThemAndOneWithoutFromItsBalance() {
        val p = portfolio(listOf(account(1, Registration.TFSA, 2_557_600, 2_233_600), account(2, Registration.RRSP, 0, 0, holdings = 0, valuedOn = null)))
        val lines = investLines(p, listOf(balance(1, "TFSA", 2_500_000), balance(2, "RRSP", 900_000, LocalDate.of(2026, 9, 30)), balance(3, "Chequing", 5, type = AccountType.CHEQUING)))
        assertEquals(listOf(1L, 2L), lines.map { it.id })
        assertFalse(lines[0].byHand)
        assertEquals(2_557_600L, lines[0].worth)
        assertTrue(lines[1].byHand)
        assertEquals(900_000L, lines[1].worth)
        assertEquals(Registration.RRSP, lines[1].registration)
        assertEquals(LocalDate.of(2026, 9, 30), lines[1].valuedOn)

        val view = investView(p, listOf(balance(1, "TFSA", 2_500_000), balance(2, "RRSP", 900_000, LocalDate.of(2026, 9, 30))), null)
        assertEquals(3_457_600L, view.worth)
        assertEquals(2_557_600L, view.priced)
        assertEquals(2_233_600L, view.cost)
        assertEquals(900_000L, view.byHand)
        assertEquals(324_000L, view.gain)
        assertEquals(1_451L, view.gainBps)
        assertEquals(LocalDate.of(2026, 10, 8), view.newest)
        assertEquals(listOf("TFSA", "RRSP"), view.allocation.map { it.label })
        assertEquals(listOf(7_397L, 2_603L), view.allocation.map { it.shareBps })
    }

    @Test fun theLensReadsTheWholePageForOneKind() {
        val p = portfolio(
            accounts = listOf(account(1, Registration.TFSA, 300_000, 250_000), account(2, Registration.RRSP, 100_000, 120_000)),
            holdings = listOf(
                holding(1, Registration.TFSA, "XEQT", SecurityKind.ETF, 300_000, 250_000),
                holding(2, Registration.RRSP, "AAPL", SecurityKind.STOCK, 100_000, 120_000),
            ),
            income = listOf(IncomeEntry("2026-09-30", 1, "XEQT", ActivityType.DIVIDEND, 1_840)),
            roomLines = listOf(room(Registration.TFSA, 700_000, 420_000), room(Registration.RRSP, null, 0)),
        )
        val balances = listOf(balance(1, "TFSA", 300_000), balance(2, "RRSP", 100_000))
        val all = investView(p, balances, null)
        assertEquals(listOf("TFSA", "RRSP"), all.lenses.map { it.label })
        assertEquals(2, all.holdings.size)
        assertEquals(2, all.room.size)

        val rrsp = investView(p, balances, Registration.RRSP.name)
        assertEquals(Registration.RRSP, rrsp.lens?.registration)
        assertEquals(100_000L, rrsp.worth)
        assertEquals(-20_000L, rrsp.gain)
        assertEquals(listOf("AAPL"), rrsp.holdings.map { it.symbol })
        assertTrue(rrsp.income.isEmpty())
        assertNull(rrsp.incomeByMonth)
        assertEquals(listOf(Registration.RRSP), rrsp.room.map { it.registration })
        assertTrue("One kind needs no allocation by kind", rrsp.allocation.isEmpty())

        assertNull("A kind no longer held reads every account", investView(p, balances, Registration.FHSA.name).lens)
        assertTrue("One kind needs no lens", investView(p, balances.take(1), null).lenses.isEmpty())
    }

    @Test fun kindsCountTheCashAsCash() {
        val kinds = kindLines(
            listOf(holding(1, null, "XEQT", SecurityKind.ETF, 300_000, 0), holding(1, null, "VFV", SecurityKind.ETF, 100_000, 0)),
            cash = 100_000,
        )
        assertEquals(listOf("ETFs", "Cash"), kinds.map { it.label })
        assertEquals(listOf(8_000L, 2_000L), kinds.map { it.shareBps })
    }

    @Test fun theContextSaysHowFreshTheFiguresAre() {
        assertEquals("3 accounts · valued 8/10", investContext(3, LocalDate.of(2026, 10, 8), today, short))
        assertEquals("1 account · last valued 12/8", investContext(1, LocalDate.of(2026, 8, 12), today, short))
        assertEquals("2 accounts · no values yet", investContext(2, null, today, short))
        assertEquals("58 days since the last report", staleLine(LocalDate.of(2026, 8, 12), today))
        assertEquals("4 months since the last report", staleLine(LocalDate.of(2026, 6, 1), today))
    }

    @Test fun anAccountLineSaysItsKindItsDayAndItsReturn() {
        val read = InvestLine(1, "Wealthsimple TFSA", Registration.TFSA, 300_000, 250_000, 0, 5, false, LocalDate.of(2026, 10, 8), 620, true, LocalDate.of(2024, 3, 4))
        assertEquals("TFSA · valued 8/10 · 5 holdings · +6.2% a year since 4/3", accountMeta(read, short))
        val byHand = read.copy(registration = null, byHand = true, holdings = 0, valuedOn = null, returnBps = null)
        assertEquals("Kind not set · no value recorded", accountMeta(byHand, short))
    }

    // ── The import ───────────────────────────────────────────────────────────

    @Test fun aFileAccountGoesWhereItsNumberOrItsNameSays() {
        val held = listOf(
            balance(4, "Wealthsimple TFSA", 0),
            balance(5, "Mon CELIAPP", 0),
            balance(6, "Chequing TFSA", 0, type = AccountType.CHEQUING),
        )
        val file = listOf(
            FileAccount("HQ1", "TFSA", Registration.TFSA, null, 12),
            FileAccount("HQ2", "FHSA", Registration.FHSA, null, 3),
            FileAccount("HQ3", "RRSP", Registration.RRSP, 9L, 4),
            FileAccount("HQ4", "Personal", Registration.NON_REGISTERED, null, 1),
        )
        assertEquals(mapOf("HQ1" to 4L, "HQ2" to 5L, "HQ3" to 9L, "HQ4" to null), guessMapping(file, held))
        // Two accounts both named for the kind: the owner picks, nothing is guessed.
        assertEquals(mapOf("HQ1" to null), guessMapping(file.take(1), held + balance(7, "Old TFSA", 0)))
        assertEquals("Wealthsimple RRSP", newAccountName(FileAccount("HQ5", " ", Registration.RRSP, null, 1)))
    }

    @Test fun thePreviewAndTheDoneLineSayWhatComesIn() {
        val report = InvestPreview(KIND_HOLDINGS, LocalDate.of(2026, 5, 8), listOf(FileAccount("HQ1", "TFSA", Registration.TFSA, null, 3)), 3, 0, 3, 0, emptyList())
        assertEquals("Wealthsimple holdings report · as of 8/5", investFileLine(report, short))
        assertEquals("holdings in 1 account", investPlanLine(report, null))
        assertEquals("Import 3 holdings", investActionLabel(report))
        val statement = report.copy(kind = KIND_STATEMENT, accounts = emptyList(), holdings = 0, activities = 30, new = 0, duplicates = 30)
        assertEquals("Nothing new to import", investActionLabel(statement))
        assertFalse(InvestImport(statement, emptyMap()).ready)
        assertTrue(InvestImport(statement, emptyMap(), statementAccountId = 4).ready)
        assertEquals("Imported 12 holdings · 2 accounts added", investImportedLine(12, 0, 2, 0))
        assertEquals("Imported 3 holdings and 40 activities · 8 already in Tally", investImportedLine(3, 40, 0, 8))
        assertEquals("Nothing new to add · 30 already in Tally", investImportedLine(0, 0, 0, 30))
    }

    @Test fun copyKeepsTheVoice() {
        val lines = listOf(
            costLine(1, 2, money), costLine(-1, 2, money), costLine(0, 0, money), incomeLine(0, money), incomeLine(8_600, money),
            roomText(room(Registration.TFSA, 700_000, 730_000, over = 30_000), today, money, short),
            roomText(room(Registration.RRSP, 2_000_000, 2_150_000, over = 150_000, overTaxed = 0), today, money, short),
            roomText(room(Registration.FHSA, null, 0), today, money, short), staleLine(LocalDate.of(2026, 1, 1), today),
            investImportedLine(0, 0, 0, 0), WS_HOLDINGS_STEPS, WS_ACTIVITIES_STEPS, INVEST_FOOTER,
        ) + (REGISTRATION_CHOICES + listOf(null)).map { registrationLabel(it) } + SecurityKind.entries.map { kindLabel(it) } +
            ROOM_KINDS.map { roomSource(it) }
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" contains \"$bad\"", line.contains(bad, ignoreCase = true)) } }
    }
}
