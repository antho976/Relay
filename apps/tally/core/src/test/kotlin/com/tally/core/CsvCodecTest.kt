package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class CsvCodecTest {

    private val rows = listOf(
        CsvRow(LocalDate.of(2026, 10, 3), TxType.EXPENSE, 1_250, "Dining", "Visa", null, "Lunch, with \"Sam\""),
        CsvRow(LocalDate.of(2026, 10, 4), TxType.TRANSFER, 90_000, null, "Chequing", "Visa", "Card payment"),
        CsvRow(LocalDate.of(2026, 10, 5), TxType.INCOME, 215_000, "Salary", "Chequing", null, "=SUM(A1)"),
        CsvRow(LocalDate.of(2026, 10, 6), TxType.EXPENSE, 8_431, "@Home", "+Savings", null, "@Costco"),
        CsvRow(LocalDate.of(2026, 10, 7), TxType.TRANSFER, 5_000, null, "-Cash", "+Savings", "'=already quoted"),
        CsvRow(LocalDate.of(2026, 10, 8), TxType.EXPENSE, 499, "Épicerie", "Visa", null, "'tis the season"),
        CsvRow(LocalDate.of(2026, 10, 9), TxType.EXPENSE, 10_000, "Shopping", "Visa", null, "TV 55\" stand"),
    )

    @Test fun `round trips through encode and decode`() {
        val text = CsvCodec.encode(rows, "CAD", 2)
        val back = CsvCodec.decode(text, 2, "Chequing")
        assertTrue(back.errors.toString(), back.errors.isEmpty())
        assertEquals("names and notes come back exactly, guard apostrophes and all", rows, back.rows)
    }

    @Test fun `starts with a byte order mark so Excel reads the accents`() {
        val text = CsvCodec.encode(rows, "CAD", 2)
        assertTrue(text.startsWith("\uFEFFdate,type,amount"))
        assertEquals("the reader drops it again", "date", CsvCodec.parseRecords(text).first().first())
    }

    @Test fun `the guard apostrophe comes off only where the export put it`() {
        assertEquals("@Costco", CsvCodec.unguard("'@Costco"))
        assertEquals("'=x", CsvCodec.unguard("''=x"))
        assertEquals("'hello", CsvCodec.unguard("'hello"))
        assertEquals("'-5", CsvCodec.unguard("'-5"))
        assertEquals("=x", CsvCodec.unguard("=x"))
    }

    @Test fun `a quote inside a field is an inch mark, not the start of a quoted section`() {
        val text = listOf(
            "date,note,amount",
            "2026-10-01,TV 55\" stand,-100.00",
            "2026-10-02,Coffee,-4.25",
            "2026-10-03,Cable 6\" long,-9.99",
            "2026-10-04,Lunch,-12.00",
        ).joinToString("\n")
        val r = CsvCodec.decode(text, 2, "Visa")
        assertTrue(r.errors.toString(), r.errors.isEmpty())
        assertEquals(listOf("TV 55\" stand", "Coffee", "Cable 6\" long", "Lunch"), r.rows.map { it.note })
        assertEquals(listOf(10_000L, 425L, 999L, 1_200L), r.rows.map { it.amount })
        assertEquals((1..4).map { LocalDate.of(2026, 10, it) }, r.rows.map { it.date })
    }

    @Test fun `a quote at the start of a field still opens a quoted section, spaces before it or not`() {
        val recs = CsvCodec.parseRecords("a, \"b, c\",\"say \"\"hi\"\"\" \r\n")
        assertEquals(listOf(listOf("a", "b, c", "say \"hi\" ")), recs)
    }

    @Test fun `every way of writing a minus reads as money going out`() {
        val text = "date,amount\n2026-10-01,-4.25\n2026-10-01,−4.25\n2026-10-01,$-4.25\n2026-10-01,4.25-\n2026-10-01,(4.25)\n2026-10-01,4.25\n"
        val r = CsvCodec.decode(text, 2, "Visa")
        assertEquals(List(5) { TxType.EXPENSE } + TxType.INCOME, r.rows.map { it.type })
        assertTrue(r.rows.all { it.amount == 425L })
    }

    @Test fun `quotes commas and doubles quotes`() {
        val text = CsvCodec.encode(rows.take(1), "CAD", 2)
        assertTrue(text, text.contains("\"Lunch, with \"\"Sam\"\"\""))
    }

    @Test fun `defuses spreadsheet formulas`() {
        val text = CsvCodec.encode(rows.drop(2), "CAD", 2)
        assertTrue(text, text.contains("'=SUM(A1)"))
    }

    @Test fun `amounts use a dot whatever the locale`() {
        assertEquals("12.50", CsvCodec.majorString(1_250, 2))
        assertEquals("1500", CsvCodec.majorString(1_500, 0))
        assertEquals("0.05", CsvCodec.majorString(5, 2))
    }

    @Test fun `bank export without a type column reads signs`() {
        val text = "Date,Description,Amount\n2026-10-01,Coffee,-4.25\n2026-10-02,Refund,12.00\n"
        val r = CsvCodec.decode(text, 2, "Chequing")
        assertEquals(listOf(TxType.EXPENSE, TxType.INCOME), r.rows.map { it.type })
        assertEquals(listOf(425L, 1_200L), r.rows.map { it.amount })
        assertEquals("Coffee", r.rows[0].note)
        assertEquals("Chequing", r.rows[0].account)
    }

    @Test fun `bad lines are reported by number and skipped`() {
        val text = "date,amount,type\n2026-13-01,5,expense\n2026-10-01,abc,expense\n2026-10-01,5,gift\n2026-10-02,5,expense\n"
        val r = CsvCodec.decode(text, 2, "Cash")
        assertEquals(1, r.rows.size)
        assertEquals(3, r.errors.size)
        assertTrue(r.errors[0], r.errors[0].startsWith("Line 2"))
    }

    @Test fun `a file without the needed columns says so`() {
        val r = CsvCodec.decode("foo,bar\n1,2\n", 2, "Cash")
        assertTrue(r.rows.isEmpty())
        assertEquals(1, r.errors.size)
    }

    @Test fun `parses quoted newlines and a byte order mark`() {
        val recs = CsvCodec.parseRecords("﻿a,b\r\n\"x\ny\",2\r\n")
        assertEquals(listOf(listOf("a", "b"), listOf("x\ny", "2")), recs)
    }
}
