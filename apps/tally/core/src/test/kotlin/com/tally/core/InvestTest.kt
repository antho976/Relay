package com.tally.core

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

/** The shared tests of `docs/INVESTMENTS.md`, by id; `invest.rs` carries the same, by the same names. */
class InvestTest {

    private val unit = Invest.QTY_SCALE

    private fun date(y: Int, m: Int, d: Int): LocalDate = LocalDate.of(y, m, d)

    private val tfsa = InvestAccountRow(1, "a1", "TFSA", Registration.TFSA)

    private fun security(id: Long, symbol: String, currency: String, kind: SecurityKind) =
        SecurityRow(id, "sec:$symbol", symbol, "", currency, kind)

    private fun input(today: LocalDate) =
        PortfolioInput("CAD", today, accounts = listOf(tfsa), securities = listOf(security(1, "XEQT", "CAD", SecurityKind.ETF)))

    /** An activity in account 1; one that moves cash only names no security. */
    private fun act(uid: String, type: ActivityType, day: LocalDate, quantity: Long, amount: Long, fee: Long): ActivityRow {
        val cashOnly = type in setOf(ActivityType.DEPOSIT, ActivityType.WITHDRAWAL, ActivityType.FEE, ActivityType.TAX, ActivityType.FX)
        return ActivityRow(0, uid, 1, if (cashOnly) null else 1, type, day, quantity, amount, fee, "CAD")
    }

    private fun hold(i: PortfolioInput, day: LocalDate, quantity: Long, book: Long) =
        i.copy(holdings = i.holdings + HoldingRow(1, 1, day, quantity, book, book))

    private fun PortfolioInput.with(vararg activities: ActivityRow) = copy(activities = this.activities + activities)

    /** The one position's units and book after [acts], applied a day apart from 2026-01-01. */
    private fun after(vararg acts: Triple<ActivityType, Long, Pair<Long, Long>>): Pair<Long, Long> {
        val activities = acts.mapIndexed { n, (type, quantity, money) ->
            act("u%03d".format(n), type, date(2026, 1, 1).plusDays(n.toLong()), quantity, money.first, money.second)
        }
        val p = Invest.positions(input(date(2026, 12, 31)).copy(activities = activities)).positions.firstOrNull()
        return (p?.quantity ?: 0L) to (p?.book ?: 0L)
    }

    private fun step(type: ActivityType, quantity: Long, amount: Long, fee: Long = 0) = Triple(type, quantity, amount to fee)

    private fun cash(i: PortfolioInput, currency: String): Long =
        Invest.positions(i).cash.filter { it.currency == currency }.sumOf { it.amount }

    // ── A: fixed point ───────────────────────────────────────────────────────

    @Test fun `A1 mul div rounds half to even`() {
        assertEquals(6L, Invest.mulDivHalfEven(5, 130_000_000, 100_000_000))
        assertEquals(2L, Invest.mulDivHalfEven(1, 150_000_000, 100_000_000))
        assertEquals(-2L, Invest.mulDivHalfEven(-15, 1, 10))
        assertEquals(-2L, Invest.mulDivHalfEven(-25, 1, 10))
    }

    @Test fun `A2 market value rounds once in minor units`() {
        assertEquals(4115L, Invest.marketValue(33_333_300, 12_345_000_000, 2))
        assertEquals(0L, Invest.marketValue(50_000_000, 1_000_000, 2))
        assertEquals(2L, Invest.marketValue(150_000_000, 1_000_000, 2))
        assertEquals(2L, Invest.marketValue(250_000_000, 1_000_000, 2))
    }

    @Test fun `A3 convert rounds half to even`() {
        assertEquals(4570L, Invest.convert(3333, 2, 2, 137_125_000))
        assertEquals(6L, Invest.convert(5, 2, 2, 130_000_000))
    }

    @Test fun `A4 parse scaled reads exact decimals`() {
        assertEquals(Scaled(12_345_679, rounded = true), Invest.parseScaled("0.123456789", 8))
        assertEquals(12_345_678L, Invest.parseScaled("0.123456785", 8)?.value)
        assertEquals(12_345_678L, Invest.parseScaled("0.123456775", 8)?.value)
        assertEquals(Scaled(1_000_000_000, rounded = false), Invest.parseScaled("10", 8))
        assertEquals(123_450L, Invest.parseScaled("1,234.5", 2)?.value)
        assertEquals(-300L, Invest.parseScaled("-3", 2)?.value)
        listOf("", "abc", "1.2.3", "$5", "1e5", ".5", "5.", "--3", "\u0663").forEach { bad -> assertNull(bad, Invest.parseScaled(bad, 2)) }
    }

    @Test fun `A5 market value says when it overflows`() {
        assertThrows(ArithmeticException::class.java) { Invest.marketValue(9_000_000_000_000_000_000, 100_000_000_000_000_000, 2) }
    }

    // ── B: positions ─────────────────────────────────────────────────────────

    @Test fun `B1 book is the average cost`() {
        val buys = arrayOf(step(ActivityType.BUY, 100 * unit, 150_000), step(ActivityType.BUY, 150 * unit, 300_000))
        assertEquals(250 * unit to 450_000L, after(*buys))
        assertEquals(50 * unit to 90_000L, after(*buys, step(ActivityType.SELL, 200 * unit, 400_000)))
        assertEquals(400 * unit to 825_000L, after(*buys, step(ActivityType.SELL, 200 * unit, 400_000), step(ActivityType.BUY, 350 * unit, 735_000)))
    }

    @Test fun `B3 a fee adds to the book and a sale takes its share`() {
        assertEquals(10 * unit to 50_999L, after(step(ActivityType.BUY, 10 * unit, 50_000, 999)))
        assertEquals(6 * unit to 30_599L, after(step(ActivityType.BUY, 10 * unit, 50_000, 999), step(ActivityType.SELL, 4 * unit, 24_000, 999)))
    }

    @Test fun `B4 three sales of a third empty the book exactly`() {
        val buy = step(ActivityType.BUY, 3 * unit, 10_000)
        val sell = step(ActivityType.SELL, unit, 4_000)
        assertEquals(2 * unit to 10_000L - 3333, after(buy, sell))
        assertEquals(unit to 10_000L - 3333 - 3334, after(buy, sell, sell))
        assertEquals(0L to 0L, after(buy, sell, sell, sell))
    }

    @Test fun `B5 fractional units`() {
        val buys = arrayOf(step(ActivityType.BUY, 50_000_000, 10_000), step(ActivityType.BUY, 25_000_000, 6_000))
        assertEquals(75_000_000L to 16_000L, after(*buys))
        assertEquals(45_000_000L to 9_600L, after(*buys, step(ActivityType.SELL, 30_000_000, 9_000)))
    }

    @Test fun `B7 a split adds units and leaves the book`() {
        val i = hold(input(date(2026, 12, 31)), date(2026, 1, 1), 100 * unit, 500_000)
            .with(act("s", ActivityType.SPLIT, date(2026, 2, 1), 100 * unit, 0, 0))
        val p = Invest.positions(i).positions[0]
        assertEquals(200 * unit to 500_000L, p.quantity to p.book)
    }

    @Test fun `B9 return of capital lowers the book to zero and no further`() {
        val buy = step(ActivityType.BUY, 100 * unit, 101_000)
        assertEquals(100 * unit to 51_000L, after(buy, step(ActivityType.RETURN_OF_CAPITAL, 0, 50_000)))
        assertEquals(100 * unit to 0L, after(buy, step(ActivityType.RETURN_OF_CAPITAL, 0, 50_000), step(ActivityType.RETURN_OF_CAPITAL, 0, 60_000)))
    }

    @Test fun `B10 a reinvested distribution adds units book and income and no cash`() {
        val i = hold(input(date(2026, 3, 1)), date(2026, 1, 1), 100 * unit, 200_000)
            .with(act("r", ActivityType.REINVEST, date(2026, 2, 1), 2 * unit, 5_000, 0))
        val p = Invest.positions(i).positions[0]
        assertEquals(102 * unit to 205_000L, p.quantity to p.book)
        assertEquals(0L, cash(i, "CAD"))
        assertEquals(5_000L, Invest.portfolio(i).income12m)
    }

    @Test fun `B16 selling more than is held is an issue and empties the position`() {
        val i = input(date(2026, 12, 31)).with(
            act("b", ActivityType.BUY, date(2026, 1, 1), 5 * unit, 5_000, 0),
            act("s", ActivityType.SELL, date(2026, 1, 2), 6 * unit, 7_200, 0),
        )
        val p = Invest.positions(i)
        assertTrue("no units and no book: dropped", p.positions.isEmpty())
        assertEquals(listOf("Sold more XEQT than the ledger holds"), p.issues)
    }

    @Test fun `B21 a buy and a sell on one day apply buy first`() {
        val i = input(date(2026, 12, 31)).with(
            act("b", ActivityType.SELL, date(2026, 1, 1), 10 * unit, 12_000, 0),
            act("a", ActivityType.BUY, date(2026, 1, 1), 10 * unit, 10_000, 0),
        )
        val p = Invest.positions(i)
        assertEquals(emptyList<String>(), p.issues)
        assertTrue(p.positions.isEmpty())
    }

    @Test fun `S1 a snapshot replaces what came before it and later entries add to it`() {
        val i = hold(input(date(2026, 5, 31)), date(2026, 5, 8), 10 * unit, 30_000).with(
            act("early", ActivityType.BUY, date(2026, 5, 1), 10 * unit, 30_000, 0),
            act("late", ActivityType.BUY, date(2026, 5, 10), 2 * unit, 7_000, 0),
        )
        val p = Invest.positions(i).positions[0]
        assertEquals(12 * unit to 37_000L, p.quantity to p.book)
    }

    // ── C: cash ──────────────────────────────────────────────────────────────

    @Test fun `C1 cash follows every kind of activity`() {
        val d = date(2026, 1, 1)
        val i = input(date(2026, 12, 31)).with(
            act("1", ActivityType.DEPOSIT, d, 0, 100_000, 0),
            act("2", ActivityType.BUY, d, unit, 50_000, 999),
            act("3", ActivityType.DIVIDEND, d, 0, 1_234, 0),
            act("4", ActivityType.TAX, d, 0, 185, 0),
            act("5", ActivityType.FEE, d, 0, 500, 0),
            act("6", ActivityType.WITHDRAWAL, d, 0, 20_000, 0),
        )
        assertEquals(29_550L, cash(i, "CAD"))
    }

    @Test fun `C2 an exchange moves cash between currencies`() {
        val i = input(date(2026, 12, 31)).with(
            act("1", ActivityType.DEPOSIT, date(2026, 1, 1), 0, 100_000, 0),
            act("2", ActivityType.FX, date(2026, 1, 2), 0, 50_000, 0).copy(toAmount = 36_000, toCurrency = "USD"),
        )
        assertEquals(50_000L to 36_000L, cash(i, "CAD") to cash(i, "USD"))
    }

    // ── V: value ─────────────────────────────────────────────────────────────

    @Test fun `V1 a holding is worth its newest price and a foreign one its own ratio without a rate`() {
        var i = hold(input(date(2026, 6, 1)), date(2026, 5, 8), 10 * unit, 20_000)
        i = i.copy(prices = listOf(PriceRow(1, date(2026, 5, 8), 25 * Invest.PRICE_SCALE)))
        val h = Invest.portfolio(i).holdings[0]
        assertEquals(Triple(25_000L, 5_000L, 2_500L), Triple(h.value, h.gain, h.gainBps))
        assertTrue(!h.fxEstimated && !h.noPrice)

        var usd = input(date(2026, 6, 1)).copy(
            securities = listOf(security(1, "AAPL", "USD", SecurityKind.STOCK)),
            holdings = listOf(HoldingRow(1, 1, date(2026, 5, 8), 10 * unit, 1_000, 750)),
            prices = listOf(PriceRow(1, date(2026, 5, 8), Invest.PRICE_SCALE)),
        )
        val aapl = Invest.portfolio(usd).holdings[0]
        assertEquals(1_333L, aapl.value)
        assertTrue(aapl.fxEstimated)
        // With a rate on the books, the rate wins.
        usd = usd.copy(fxRates = listOf(FxRateRow("USD", "CAD", date(2026, 5, 1), 137_125_000)))
        assertEquals(1_371L, Invest.portfolio(usd).holdings[0].value)
    }

    // ── R: room ──────────────────────────────────────────────────────────────

    @Test fun `R1 tfsa cumulative sums the limits from 2009`() {
        assertEquals(10_900_000L, Invest.tfsaCumulative(2009, 2026))
        assertEquals(5_700_000L, Invest.tfsaCumulative(2018, 2026))
        assertEquals(700_000L, Invest.tfsaCumulative(2026, 2026))
        assertEquals(0L, Invest.tfsaCumulative(2027, 2026))
    }

    @Test fun `R4 tfsa withdrawals come back next year`() {
        assertEquals(1_100_000L, Invest.tfsaNextRoom(1_000_000, 1_000_000, 400_000, 700_000))
    }

    @Test fun `R7 fhsa room carries what went unused`() {
        assertEquals(1_400_000L, Invest.fhsaRoom(2024, mapOf(2024 to 800_000L, 2025 to 200_000L), 2026))
        assertEquals(1_600_000L, Invest.fhsaRoom(2025, emptyMap(), 2026))
        assertEquals(400_000L, Invest.fhsaRoom(2025, mapOf(2025 to 3_600_000L), 2026))
    }

    @Test fun `R8 the rrsp deadline is the sixtieth day moved off a weekend`() {
        assertEquals(date(2026, 3, 2), Invest.rrspDeadline(2025))
        assertEquals(date(2027, 3, 1), Invest.rrspDeadline(2026))
        assertEquals(date(2028, 2, 29), Invest.rrspDeadline(2027))
    }

    @Test fun `R9 an rrsp has a two thousand dollar buffer`() {
        fun rrsp(contributed: Long): RoomLine {
            val i = PortfolioInput(
                "CAD",
                date(2026, 10, 9),
                accounts = listOf(InvestAccountRow(1, "a1", "RRSP", Registration.RRSP)),
                activities = listOf(
                    act("d", ActivityType.DEPOSIT, date(2026, 4, 1), 0, contributed, 0),
                    // Before last year's deadline: it counts for last year.
                    act("e", ActivityType.DEPOSIT, date(2026, 3, 2), 0, 999_999, 0),
                ),
                roomFacts = listOf(RoomFactRow(Registration.RRSP, 2026, 2_000_000)),
            )
            return Invest.portfolio(i).room.single { it.registration == Registration.RRSP }
        }
        val r = rrsp(2_150_000)
        assertEquals(Triple(150_000L, 0L, 0L), Triple(r.over, r.overTaxed, r.left))
        assertEquals("2027-03-01", r.deadline)
        assertEquals(100_000L, rrsp(2_300_000).overTaxed)
    }

    // ── X: money-weighted return ─────────────────────────────────────────────

    private fun close(want: Double, got: Double?) {
        assertTrue("want $want, got $got", got != null && kotlin.math.abs(want - got) < 1e-9)
    }

    private fun flows(vararg f: Pair<LocalDate, Long>) = f.map { CashFlow(it.first, it.second) }

    @Test fun `X1 a year at ten percent`() {
        close(0.1, Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2026, 1, 1) to 1100)))
    }

    @Test fun `X2 a leap year counts 366 days`() {
        close(0.0997135859341, Invest.xirr(flows(date(2024, 1, 1) to -1000, date(2025, 1, 1) to 1100)))
    }

    @Test fun `X3 two deposits`() {
        close(0.1343767484042, Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2025, 7, 1) to -1000, date(2026, 1, 1) to 2200)))
    }

    @Test fun `X4 under a year shows the period return`() {
        val r = Invest.xirr(flows(date(2026, 1, 1) to -1000, date(2026, 3, 1) to 1020))
        close(0.1303279129010, r)
        close(0.02, Invest.periodReturn(r!!, 59))
    }

    @Test fun `X5 no money out has no rate`() {
        assertNull(Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2026, 1, 1) to -1000)))
    }

    @Test fun `X6 two roots take the one nearest zero`() {
        close(0.1127016653793, Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2026, 1, 1) to 3000, date(2027, 1, 1) to -2100)))
    }

    @Test fun `X7 a loss of half`() {
        close(-0.5, Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2026, 1, 1) to 500)))
    }

    @Test fun `X8 money in and out along the way`() {
        close(
            0.1006305548521,
            Invest.xirr(flows(date(2025, 1, 1) to -1000, date(2025, 6, 1) to -2000, date(2025, 9, 1) to 500, date(2026, 1, 1) to 2700)),
        )
    }

    // ── I: ids ───────────────────────────────────────────────────────────────

    @Test fun `I1 an imported line has the same uid on both devices`() {
        val parts = listOf("3f2a9c1e-0000-4000-8000-000000000001", "2026-03-02", "BUY", "sec:XTSE:XEQT", "1000000000", "35000", "CAD", "0")
        assertEquals("imp:86fe65d31110bce166befa06e9747e9a", Invest.importUid(parts, 0))
        assertEquals("imp:2523ed6741037c001850cee952d9149f", Invest.importUid(parts, 1))
    }

    // ── The reading ──────────────────────────────────────────────────────────

    @Test fun `an account with deposits from the start reads its money weighted return`() {
        var i = input(date(2026, 1, 1)).with(
            act("d", ActivityType.DEPOSIT, date(2025, 1, 1), 0, 100_000, 0),
            act("b", ActivityType.BUY, date(2025, 1, 2), 10 * unit, 100_000, 0),
        )
        i = i.copy(prices = listOf(PriceRow(1, date(2025, 12, 31), 110 * Invest.PRICE_SCALE)))
        val p = Invest.portfolio(i)
        val a = p.accounts[0]
        assertEquals(Triple(110_000L, 100_000L, 1_000L), Triple(a.value, a.book, a.gainBps))
        assertEquals(Triple(1_000L, true, "2025-01-01"), Triple(a.returnBps, a.returnAnnual, a.returnSince))
        assertEquals("2025-12-31", p.asOf)
        assertEquals(Registration.TFSA to 10_000L, p.allocation[0].registration to p.allocation[0].shareBps)
        assertEquals(SecurityKind.ETF to 10_000L, p.kinds[0].kind to p.kinds[0].shareBps)
        // A snapshot from before any activity hides where the money started.
        assertNull(Invest.portfolio(hold(i, date(2024, 12, 1), 10 * unit, 90_000)).accounts[0].returnBps)
    }

    @Test fun `income counts twelve months and lists the newest first`() {
        val i = input(date(2026, 10, 9)).with(
            act("old", ActivityType.DIVIDEND, date(2025, 10, 9), 0, 1_000, 0),
            act("in", ActivityType.DIVIDEND, date(2025, 10, 10), 0, 200, 0),
            act("new", ActivityType.INTEREST, date(2026, 10, 1), 0, 30, 0),
        )
        val p = Invest.portfolio(i)
        assertEquals("the 365 days ending today", 230L, p.income12m)
        assertEquals(12, p.incomeByMonth.size)
        assertEquals(IncomeMonth("2025-11", 0), p.incomeByMonth[0])
        assertEquals(IncomeMonth("2026-10", 30), p.incomeByMonth[11])
        assertEquals(listOf("2026-10-01", "2025-10-10", "2025-10-09"), p.income.map { it.date })
    }

    @Test fun `tfsa room is the cra figure minus what went in this year`() {
        var i = input(date(2026, 10, 9)).copy(roomFacts = listOf(RoomFactRow(Registration.TFSA, 2026, 1_000_000))).with(
            act("last year", ActivityType.DEPOSIT, date(2025, 12, 31), 0, 500_000, 0),
            act("in", ActivityType.DEPOSIT, date(2026, 2, 1), 0, 300_000, 0),
            act("out", ActivityType.WITHDRAWAL, date(2026, 3, 1), 0, 50_000, 0),
        )
        val r = Invest.portfolio(i).room[0]
        assertEquals(Registration.TFSA, r.registration)
        assertEquals(listOf(1_000_000L, 300_000L, 50_000L, 700_000L, 0L), listOf(r.room, r.contributed, r.withdrawn, r.left, r.over))
        assertEquals("2026-12-31" to 700_000L, r.deadline to r.limit)
        // An account without activities counts the transfers Tally made into it.
        i = i.copy(
            activities = emptyList(),
            transfers = listOf(TransferRow(1, date(2026, 4, 1), 200_000), TransferRow(1, date(2026, 5, 1), -20_000)),
        )
        val t = Invest.portfolio(i).room[0]
        assertEquals(200_000L to 20_000L, t.contributed to t.withdrawn)
    }

    @Test fun `in RRSP season the RRSP row reads last year`() {
        fun rrsp(today: LocalDate): RoomLine {
            val i = PortfolioInput(
                "CAD",
                today,
                accounts = listOf(InvestAccountRow(1, "a1", "RRSP", Registration.RRSP)),
                activities = listOf(
                    // Before 2025's deadline (2026-03-02): it counts for 2025.
                    act("a", ActivityType.DEPOSIT, date(2026, 2, 20), 0, 30_000, 0),
                    act("b", ActivityType.DEPOSIT, date(2026, 6, 1), 0, 50_000, 0),
                    act("c", ActivityType.DEPOSIT, date(2027, 1, 15), 0, 100_000, 0),
                ),
            )
            return Invest.portfolio(i).room.single { it.registration == Registration.RRSP }
        }
        val season = rrsp(date(2027, 2, 10))
        assertEquals(Triple(2026, 150_000L, "2027-03-01"), Triple(season.year, season.contributed, season.deadline))
        val past = rrsp(date(2027, 3, 2))
        assertEquals(2027 to 0L, past.year to past.contributed)
    }

    @Test fun `an empty portfolio says so`() {
        val p = Invest.portfolio(PortfolioInput("CAD", date(2026, 10, 9)))
        assertTrue(p.empty)
        assertEquals(Triple(0L, null, null), Triple(p.value, p.gainBps, p.asOf))
        assertTrue(p.room.isEmpty() && p.holdings.isEmpty())
    }

    @Test fun `foreign cash without a rate is counted one to one and said`() {
        var i = input(date(2026, 10, 9)).with(
            act("x", ActivityType.FX, date(2026, 1, 2), 0, 0, 0).copy(toAmount = 1_000, toCurrency = "USD"),
        )
        val p = Invest.portfolio(i)
        assertEquals(1_000L, p.cash)
        assertEquals(listOf("No USD to CAD rate: USD cash is counted one to one"), p.issues)
        i = i.copy(fxRates = listOf(FxRateRow("CAD", "USD", date(2026, 1, 1), 72_926_162)))
        assertEquals("the inverse of a CAD to USD rate", 1_371L, Invest.portfolio(i).cash)
    }

    @Test fun `the reading encodes and decodes`() {
        val i = hold(input(date(2026, 10, 9)), date(2026, 5, 8), 10 * unit, 20_000).copy(
            prices = listOf(PriceRow(1, date(2026, 5, 8), 25 * Invest.PRICE_SCALE)),
        ).with(act("d", ActivityType.DIVIDEND, date(2026, 9, 15), 0, 1_234, 0))
        val p = Invest.portfolio(i)
        assertEquals("25,000 held and 1,234 cash", 26_234L, p.value)
        assertEquals(9_530L, p.holdings.single().weightBps)
        assertEquals(listOf(SecurityKind.ETF, SecurityKind.CASH), p.kinds.map { it.kind })
        assertFalse(p.empty)
        assertEquals(p, Json.decodeFromString(Portfolio.serializer(), Json.encodeToString(Portfolio.serializer(), p)))
    }
}
