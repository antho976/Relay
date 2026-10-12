package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class BankStatementsTest {

    private fun ok(text: String, dayFirst: Boolean = true): Statement {
        val read = BankStatements.read(text, 2, dayFirst)
        assertTrue(read.toString(), read is StatementRead.Ok)
        return (read as StatementRead.Ok).statement
    }

    // ── Desjardins ───────────────────────────────────────────────────────────

    @Test fun `desjardins positional export reads withdrawals as out and deposits as in`() {
        val text = """
            "Caisse Desjardins du Plateau","0123456","EOP","2026/09/02",1,"Achat - METRO PLUS #123 MONTREAL QC","",68.42,"","","","","",1931.58
            "Caisse Desjardins du Plateau","0123456","EOP","2026/09/03",2,"Depot direct - PAIE EMPLOYEUR INC","","",2150.00,"","","","",4081.58
            "Caisse Desjardins du Plateau","0123456","ES1","2026/09/05",1,"Interets","","","",1.25,"","","","",501.25
        """.trimIndent()
        val s = ok(text)
        assertEquals(StatementFormat.DESJARDINS, s.format)
        assertEquals(3, s.rows.size)
        val metro = s.rows[0]
        assertEquals(LocalDate.of(2026, 9, 2), metro.date)
        assertEquals(-68_42L, metro.amount)
        assertEquals(1_931_58L, metro.balance)
        assertEquals("EOP 0123456", metro.account)
        assertEquals(2_150_00L, s.rows[1].amount)
        assertEquals("a line with only interest is money in", 1_25L, s.rows[2].amount)
        assertEquals(listOf("EOP 0123456", "ES1 0123456"), s.accounts)
    }

    @Test fun `desjardins with french headers and semicolons`() {
        val text = "Date;Description;Retrait;Dépôt;Solde\n2026-09-02;Achat IGA;54,20;;1 200,00\n2026-09-03;Virement;;300,00;1 500,00\n"
        val s = ok(text)
        assertEquals(StatementFormat.DESJARDINS, s.format)
        assertEquals(listOf(-54_20L, 300_00L), s.rows.map { it.amount })
        assertEquals(1_200_00L, s.rows[0].balance)
    }

    @Test fun `windows 1252 bytes still read their accents`() {
        val bytes = "Date,Description,Montant\n2026-09-02,Épicerie,-12.00\n".toByteArray(charset("windows-1252"))
        val text = BankStatements.decodeText(bytes)
        assertTrue(text, text.contains("Épicerie"))
        assertEquals("Épicerie", ok(text).rows.single().description)
    }

    // ── Wealthsimple ─────────────────────────────────────────────────────────

    @Test fun `wealthsimple statement with en dash minus and dollar signs`() {
        val text = "DATE,POSTED DATE,DESCRIPTION,AMOUNT (CAD),BALANCE (CAD)\n" +
            "2026-08-02,2026-08-03,Deposit,\"$14,500.00\",\"$24,802.33\"\n" +
            "2026-08-03,2026-08-03,Transfer out to Chequing,\"–$1,000.00\",\"$23,802.33\"\n"
        val s = ok(text)
        assertEquals(StatementFormat.WEALTHSIMPLE, s.format)
        assertEquals(listOf(1_450_000L, -100_000L), s.rows.map { it.amount })
        assertEquals("the transaction date, not the posted one", LocalDate.of(2026, 8, 2), s.rows[0].date)
    }

    @Test fun `older wealthsimple cash export keeps its codes`() {
        val text = "date,transaction,description,amount,balance\n2026-09-01,SPEND,Metro,-42.10,500.00\n2026-09-02,INT,Interest,0.85,500.85\n"
        val s = ok(text)
        assertEquals(StatementFormat.WEALTHSIMPLE, s.format)
        assertEquals("SPEND", s.rows[0].code)
        assertEquals("Metro", s.rows[0].description)
        assertEquals(85L, s.rows[1].amount)
    }

    @Test fun `a wealthsimple investment statement goes to the investment import`() {
        val text = "date,transaction,description,amount,balance,currency\n" +
            "2026-01-02,CONT,Contribution,500.00,500.00,CAD\n" +
            "2026-01-05,BUY,\"XEQT - iShares Core Equity ETF Portfolio: Bought 10.0000 shares at \$38.12 per share\",-381.20,118.80,CAD\n"
        assertEquals(StatementRead.Investments, BankStatements.read(text, 2))
        val cash = "date,transaction,description,amount,balance,currency\n2026-09-01,SPEND,Metro,-42.10,500.00,CAD\n"
        assertEquals("a cash statement is still the bank import's", StatementFormat.WEALTHSIMPLE, ok(cash).format)
    }

    // ── Other banks ──────────────────────────────────────────────────────────

    @Test fun `debit and credit columns with month first dates`() {
        val text = "Transaction Date,Description,Debit,Credit\n09/14/2026,STARBUCKS,5.25,\n09/15/2026,PAYROLL,,1000.00\n"
        val s = ok(text, dayFirst = true)
        assertEquals("a day above 12 in the second place settles month first", LocalDate.of(2026, 9, 14), s.rows[0].date)
        assertEquals(listOf(-5_25L, 1_000_00L), s.rows.map { it.amount })
    }

    @Test fun `ambiguous numeric dates follow the hint`() {
        val text = "Date,Description,Amount\n03/04/2026,Cafe,-3.00\n"
        assertEquals(LocalDate.of(2026, 4, 3), ok(text, dayFirst = true).rows.single().date)
        assertEquals(LocalDate.of(2026, 3, 4), ok(text, dayFirst = false).rows.single().date)
    }

    @Test fun `a preamble before the header is skipped`() {
        val text = "Account,Visa 4510\nPeriod,September\n\nDate,Merchant,Amount\n\"Sep 5, 2026\",Cineplex,-24.50\n5 sept. 2026,Remboursement,12.00 CR\n"
        val s = ok(text)
        assertEquals(LocalDate.of(2026, 9, 5), s.rows[0].date)
        assertEquals(-24_50L, s.rows[0].amount)
        assertEquals("CR marks money in", 12_00L, s.rows[1].amount)
    }

    @Test fun `a header-less file with money out and money in`() {
        val text = "2026-09-02,STM OPUS,3.75,,996.25\n2026-09-03,REFUND,,10.00,1006.25\n"
        val s = ok(text)
        assertEquals(StatementFormat.BANK, s.format)
        assertEquals(listOf(-3_75L, 10_00L), s.rows.map { it.amount })
        assertEquals(1_006_25L, s.rows[1].balance)
    }

    @Test fun `tab separated with tab delimiter detected`() {
        val text = "Date\tDescription\tAmount\n2026-09-02\tCafe\t-4.50\n"
        assertEquals('\t', BankStatements.delimiterOf(text))
        assertEquals(-4_50L, ok(text).rows.single().amount)
    }

    @Test fun `tally's own export is handed to the tally import`() {
        val text = CsvCodec.HEADER.joinToString(",") + "\n2026-09-02,expense,4.50,CAD,Dining,Chequing,,Coffee\n"
        assertEquals(StatementRead.TallyCsv, BankStatements.read(text, 2))
    }

    @Test fun `a file with no transactions says so`() {
        val read = BankStatements.read("hello,world\nfoo,bar\n", 2)
        assertTrue(read is StatementRead.Invalid)
        assertTrue(BankStatements.read("", 2) is StatementRead.Invalid)
    }

    @Test fun `an unreadable date on a money line is reported, a total line is not`() {
        val text = "Date,Description,Amount\nsoon,Thing,-1.00\n,Total,\n2026-09-02,Cafe,-2.00\n"
        val s = ok(text)
        assertEquals(1, s.rows.size)
        assertEquals(1, s.errors.size)
        assertTrue(s.errors.single(), s.errors.single().contains("soon"))
    }

    @Test fun `positive share reads a card statement`() {
        val text = "Date,Description,Amount\n2026-09-02,A,4.00\n2026-09-03,B,6.00\n2026-09-04,Payment,-10.00\n"
        assertEquals(2f / 3f, ok(text).positiveShare, 0.001f)
    }

    @Test fun `dates in every written form`() {
        val cases = mapOf(
            "2026-09-05" to LocalDate.of(2026, 9, 5),
            "2026/9/5" to LocalDate.of(2026, 9, 5),
            "2026-09-05T13:20:00" to LocalDate.of(2026, 9, 5),
            "20260905" to LocalDate.of(2026, 9, 5),
            "05-Sep-2026" to LocalDate.of(2026, 9, 5),
            "5 août 2026" to LocalDate.of(2026, 8, 5),
            "5 juil. 2026" to LocalDate.of(2026, 7, 5),
            "June 30, 2026" to LocalDate.of(2026, 6, 30),
            "31/12/26" to LocalDate.of(2026, 12, 31),
        )
        cases.forEach { (text, date) -> assertEquals(text, date, StatementDates.parse(text, dayFirst = true)) }
        assertNull(StatementDates.parse("2026-13-01", true))
        assertNull(StatementDates.parse("Metro", true))
    }
}
