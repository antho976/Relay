package com.margin.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class CsvCodecTest {

    private val rows = listOf(
        CsvRow(LocalDate.of(2026, 10, 3), TxType.EXPENSE, 1_250, "Dining", "Visa", null, "Lunch, with \"Sam\""),
        CsvRow(LocalDate.of(2026, 10, 4), TxType.TRANSFER, 90_000, null, "Chequing", "Visa", "Card payment"),
        CsvRow(LocalDate.of(2026, 10, 5), TxType.INCOME, 215_000, "Salary", "Chequing", null, "=SUM(A1)"),
    )

    @Test fun `round trips through encode and decode`() {
        val text = CsvCodec.encode(rows, "CAD", 2)
        val back = CsvCodec.decode(text, 2, "Chequing")
        assertTrue(back.errors.toString(), back.errors.isEmpty())
        assertEquals(rows.map { it.copy(note = it.note.removePrefix("'")) }, back.rows.map { it.copy(note = it.note.removePrefix("'")) })
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
