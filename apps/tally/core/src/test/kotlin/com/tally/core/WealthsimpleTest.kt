package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

/** The shared tests of `docs/INVESTMENTS.md`; `wealthsimple.rs` embeds the same files, by the same names. */
class WealthsimpleTest {

    /** The holdings report's header and the demo rows of a public fixture, the AAPL row in USD. */
    private val w1 = """
        Account Name,Account Type,Account Classification,Account Number,Symbol,Exchange,MIC,Name,Security Type,Quantity,Position Direction,Market Price,Market Price Currency,Book Value (CAD),Book Value Currency (CAD),Book Value (Market),Book Value Currency (Market),Market Value,Market Value Currency,Market Unrealized Returns,Market Unrealized Returns Currency
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","AAPL","NASDAQ","XNAS","Apple Inc","EQUITY","10","LONG","100","USD","1000","CAD","750","USD","1000","USD","0","USD"
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","XEQT","TSX","XTSE","iShares Core Equity ETF Portfolio","EXCHANGE_TRADED_FUND","10","LONG","25","CAD","250","CAD","250","CAD","250","CAD","0","CAD"
        "Demo TFSA","TFSA","Trade","DEMO0001CAD","ARKK","BATS","BATS","ARK Innovation ETF","EXCHANGE_TRADED_FUND","1","LONG","50","USD","50","CAD","50","USD","50","USD","0","USD"

        "As of 2026-05-08 12:00 GMT-04:00"
    """.trimIndent() + "\n"

    /** An activities export: a buy, a dividend reinvested (a DRIP pair), a contribution, an exchange's two legs, an option trade, and a footer. */
    private val w2 = """
        transaction_date,settlement_date,account_id,account_type,activity_type,activity_sub_type,direction,symbol,name,currency,quantity,unit_price,commission,net_cash_amount
        2026-08-08,2026-08-11,HQ7XFMC41CAD,TFSA,Trade,BUY,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,10,38.12,0,-381.20
        2026-08-15,2026-08-15,HQ7XFMC41CAD,TFSA,MoneyMovement,CONTRIBUTION,,,,CAD,,,,500.00
        2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Dividend,,,XEQT,iShares Core Equity ETF Portfolio,CAD,,,,4.21
        2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Trade,DRIP,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,0.1104,38.13,0,-4.21
        2026-10-01,2026-10-01,HQ7XFMC41CAD,TFSA,FxExchange,,,,,CAD,,,,-137.13
        2026-10-01,2026-10-01,HQ7XFMC41CAD,TFSA,FxExchange,,,,,USD,,,,100.00
        2026-10-02,2026-10-03,HQ7XFMC41CAD,TFSA,Trade,STO,SHORT,AAPL 261218C00250000,AAPL Dec 2026 250 Call,USD,-1,5.10,0.75,509.25
        "As of 2026-10-09 12:00 GMT-04:00"
    """.trimIndent() + "\n"

    /** An investment account's monthly statement. */
    private val w3 = """
        date,transaction,description,amount,balance,currency
        2026-01-02,CONT,Contribution (executed at 2026-01-02),500.00,500.00,CAD
        2026-01-05,BUY,"XEQT - iShares Core Equity ETF Portfolio: Bought 10.0000 shares at ${'$'}38.12 per share",-381.20,118.80,CAD
        2026-01-29,DIV,"XEQT - iShares Core Equity ETF Portfolio: Cash dividend distribution, received on 2026-01-29",4.21,123.01,CAD
        2026-01-30,LOAN,"XEQT - iShares Core Equity ETF Portfolio: Securities lending loan",0.00,123.01,CAD
    """.trimIndent() + "\n"

    /** A Wealthsimple Cash statement: the bank import's, not this one's. */
    private val cashStatement = """
        date,transaction,description,amount,balance,currency
        2026-09-01,SPEND,Metro,-42.10,500.00,CAD
        2026-09-02,INT,Interest,0.85,500.85,CAD
    """.trimIndent() + "\n"

    private val unit = Invest.QTY_SCALE

    private fun ok(text: String): WsFile {
        val read = Wealthsimple.read(text)
        assertTrue(read.toString(), read is WsRead.Ok)
        return (read as WsRead.Ok).file
    }

    private fun cad() = PlanInput("CAD")

    @Test fun `W1 the holdings report reads its day units and book`() {
        val f = ok(w1)
        assertEquals(WsKind.HOLDINGS, f.kind)
        assertEquals(LocalDate.of(2026, 5, 8), f.asOf)
        assertEquals(listOf(10 * unit, 10 * unit, unit), f.holdings.map { it.quantity })
        assertEquals(listOf(100_000L, 25_000L, 5_000L), f.holdings.map { it.book })
        assertEquals(listOf("USD", "CAD", "USD"), f.holdings.map { it.currency })
        val aapl = f.holdings[0]
        assertEquals(Triple(75_000L, 100_000L, 100 * Invest.PRICE_SCALE), Triple(aapl.bookMarket, aapl.marketValue, aapl.price))
        assertEquals(SecurityKind.STOCK to SecurityKind.ETF, aapl.kind to f.holdings[1].kind)
        assertEquals(listOf(WsAccount("DEMO0001CAD", "Demo TFSA", Registration.TFSA)), f.accounts)
        assertTrue(f.skipped.toString(), f.skipped.isEmpty())
    }

    @Test fun `a holdings report reads with a byte order mark CRLF and its columns in any order`() {
        val text = "\uFEFFquantity,SYMBOL,Book Value (CAD),Account Number,Account Type,Security Type,Extra\r\n" +
            "12.5,veqt,\"1,234.56\",ABC123CAD,RRSP,EXCHANGE_TRADED_FUND,x\r\n\r\n\"As of 2026-05-08 12:00 GMT-04:00\"\r\n"
        val f = ok(text)
        val h = f.holdings[0]
        assertEquals(listOf("VEQT", 1_250_000_000L, 123_456L, "CAD"), listOf(h.symbol, h.quantity, h.book, h.currency))
        assertEquals("no account name in the file", "Wealthsimple RRSP", f.accounts[0].name)
        assertEquals(LocalDate.of(2026, 5, 8), f.asOf)
    }

    @Test fun `W2 the activities export maps each kind of line`() {
        val f = ok(w2)
        assertEquals(WsKind.ACTIVITIES, f.kind)
        assertEquals(
            listOf(
                listOf(2, ActivityType.BUY, "XEQT", 10 * unit, 381_20L, 0L, "CAD"),
                listOf(3, ActivityType.DEPOSIT, null, 0L, 500_00L, 0L, "CAD"),
                listOf(4, ActivityType.DIVIDEND, "XEQT", 0L, 4_21L, 0L, "CAD"),
                listOf(5, ActivityType.BUY, "XEQT", 11_040_000L, 4_21L, 0L, "CAD"),
                listOf(6, ActivityType.FX, null, 0L, 137_13L, 0L, "CAD"),
            ),
            f.activities.map { listOf(it.line, it.type, it.symbol, it.quantity, it.amount, it.fee, it.currency) },
        )
        assertEquals(100_00L to "USD", f.activities[4].toAmount to f.activities[4].toCurrency)
        assertEquals("the footer is not a line", listOf(WsSkipped(8, "Options aren't tracked yet")), f.skipped)
        assertEquals(listOf(WsAccount("HQ7XFMC41CAD", "Wealthsimple TFSA", Registration.TFSA)), f.accounts)
    }

    @Test fun `a commission is the fee and the amount is the rest`() {
        val text = """
            transaction_date,account_id,account_type,activity_type,activity_sub_type,symbol,currency,quantity,commission,net_cash_amount
            2026-08-08,A1,Personal,Trade,BUY,VFV,CAD,2,-4.95,-204.95
            2026-08-09,A1,Personal,Trade,SELL,VFV,CAD,-1,-4.95,95.05
            2026-08-10,A1,Personal,Dividend,,VFV,CAD,,,-1.00
            2026-08-11,A1,Personal,FxExchange,,,USD,,,10.00
            2026-08-12,A1,Personal,Giveaway,,,CAD,,,5.00
        """.trimIndent() + "\n"
        val f = ok(text)
        assertEquals(
            listOf(Triple(ActivityType.BUY, 200_00L, 4_95L), Triple(ActivityType.SELL, 100_00L, 4_95L)),
            f.activities.map { Triple(it.type, it.amount, it.fee) },
        )
        assertEquals(Registration.NON_REGISTERED, f.accounts[0].registration)
        assertEquals(
            listOf("A reversed dividend", "A currency exchange without its other side", "Tally doesn't read Giveaway lines yet"),
            f.skipped.map { it.reason },
        )
    }

    @Test fun `W3 an investment statement reads its trades from the description and a cash one is refused`() {
        val f = ok(w3)
        assertEquals(WsKind.STATEMENT, f.kind)
        val buy = f.activities[1]
        assertEquals(
            listOf(ActivityType.BUY, "XEQT", 10 * unit, 3_812_000_000L, 381_20L),
            listOf(buy.type, buy.symbol, buy.quantity, buy.price, buy.amount),
        )
        assertEquals(listOf(ActivityType.DEPOSIT, ActivityType.BUY, ActivityType.DIVIDEND), f.activities.map { it.type })
        assertEquals("XEQT", f.activities[2].symbol)
        assertEquals("securities lending", listOf(5), f.skipped.map { it.line })
        assertEquals(WsRead.Invalid(Wealthsimple.NOT_INVESTMENT), Wealthsimple.read(cashStatement))
    }

    @Test fun `registrations read in English and French`() {
        assertEquals(Registration.FHSA, Wealthsimple.registrationOf("CELIAPP"))
        assertEquals(Registration.TFSA, Wealthsimple.registrationOf("CELI"))
        assertEquals(Registration.RRSP, Wealthsimple.registrationOf("Spousal RRSP"))
        assertEquals(Registration.LIRA, Wealthsimple.registrationOf("CRI"))
        assertEquals(Registration.NON_REGISTERED, Wealthsimple.registrationOf("Non enregistré"))
        assertEquals(Registration.NON_REGISTERED, Wealthsimple.registrationOf("Crypto"))
        assertEquals(Registration.OTHER, Wealthsimple.registrationOf("Chequing"))
    }

    @Test fun `something else is not read`() {
        assertTrue(Wealthsimple.read("") is WsRead.Invalid)
        assertTrue(Wealthsimple.read("Date,Description,Amount\n2026-09-02,Cafe,-4.50\n") is WsRead.Invalid)
        assertEquals(
            "Wealthsimple reports are in Canadian dollars; this ledger keeps EUR",
            Wealthsimple.plan(ok(w1), PlanInput("EUR")).refused,
        )
    }

    @Test fun `W4 the plan for the holdings report has the same uids on both devices`() {
        val mapped = PlanInput("CAD", accounts = mapOf("DEMO0001CAD" to "ws:DEMO0001CAD"))
        val p = Wealthsimple.plan(ok(w1), mapped)
        assertEquals(listOf("sec:AAPL", "sec:XEQT", "sec:ARKK"), p.securities.map { it.uid })
        assertEquals(
            listOf("hold:ws:DEMO0001CAD:sec:AAPL:2026-05-08", "hold:ws:DEMO0001CAD:sec:XEQT:2026-05-08", "hold:ws:DEMO0001CAD:sec:ARKK:2026-05-08"),
            p.holdings.map { it.uid },
        )
        assertEquals(listOf("px:sec:AAPL:2026-05-08", "px:sec:XEQT:2026-05-08", "px:sec:ARKK:2026-05-08"), p.prices.map { it.uid })
        assertEquals(listOf(100 * Invest.PRICE_SCALE, 25 * Invest.PRICE_SCALE, 50 * Invest.PRICE_SCALE), p.prices.map { it.price })
        // AAPL's 1,000 USD at its own 1,000 CAD / 750 USD, XEQT's 250 CAD, ARKK's 50 USD at 50 / 50.
        assertEquals(listOf("val:ws:DEMO0001CAD:2026-05-08" to 1_633_33L), p.values.map { it.uid to it.value })
        assertEquals(Triple("USD", SecurityKind.STOCK, "NASDAQ"), Triple(p.securities[0].currency, p.securities[0].kind, p.securities[0].exchange))
        val unmapped = Wealthsimple.plan(ok(w1), cad())
        assertEquals("an account number nobody mapped is the account the import creates", p, unmapped.copy(accounts = emptyList()))
        assertEquals(listOf(PlanAccount("ws:DEMO0001CAD", "Demo TFSA", Registration.TFSA, "Wealthsimple", "DEMO0001CAD")), unmapped.accounts)
    }

    @Test fun `the plan for the activities export counts identical lines`() {
        val p = Wealthsimple.plan(ok(w2), cad())
        val uids = p.activities.map { it.uid }
        val account = "ws:HQ7XFMC41CAD"
        assertEquals(Invest.importUid(listOf(account, "2026-08-08", "BUY", "sec:XEQT", "1000000000", "38120", "CAD", "0"), 0), uids[0])
        assertEquals(Invest.importUid(listOf(account, "2026-10-01", "FX", "", "0", "13713", "CAD", "0"), 0), uids[4])
        val twice = w2 + "2026-08-08,2026-08-11,HQ7XFMC41CAD,TFSA,Trade,BUY,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,10,38.12,0,-381.20\n"
        val again = Wealthsimple.plan(ok(twice), cad())
        assertEquals(p.activities, again.activities.take(5))
        assertEquals(
            Invest.importUid(listOf(account, "2026-08-08", "BUY", "sec:XEQT", "1000000000", "38120", "CAD", "0"), 1),
            again.activities.last().uid,
        )
        val statement = Wealthsimple.plan(ok(w3), PlanInput("CAD", statementAccount = "a-uid"))
        assertTrue(statement.activities.all { it.accountUid == "a-uid" })
        assertTrue("a statement names no account", Wealthsimple.plan(ok(w3), cad()).refused != null)
    }

    @Test fun `the plan's uids are these, literally`() {
        // For the parity check against `wealthsimple.rs`: W2 into an account the import creates.
        assertEquals(
            listOf(
                "imp:aaa32c148f3cea50442cb54fcf9cf2be",
                "imp:e4abbe25d44f7fabcaab617415bd4fba",
                "imp:c96fe3372f405ada2e34d69409d9fb60",
                "imp:f65185eaae8bb27219141fbb0f61a399",
                "imp:efac3a2723a76720310c10ea87894bc9",
            ),
            Wealthsimple.plan(ok(w2), cad()).activities.map { it.uid },
        )
    }

    @Test fun `a ledger in another currency is refused and a statement goes into the account it is told`() {
        assertTrue(Wealthsimple.plan(ok(w2), PlanInput("USD")).activities.isEmpty())
        val plan = Wealthsimple.plan(ok(w3), PlanInput("CAD", statementAccount = "acct-9"))
        assertEquals(emptyList<PlanAccount>(), plan.accounts)
        assertEquals("the statement's description is the note", "Contribution (executed at 2026-01-02)", plan.activities[0].note)
    }
}
