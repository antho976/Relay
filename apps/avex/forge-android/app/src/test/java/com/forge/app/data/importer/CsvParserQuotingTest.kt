package com.forge.app.data.importer

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Row-loss regressions in the CSV reader. Both of these dropped rows silently: the import summary
 * reported a successful, smaller import rather than an error, so the user had no signal that most
 * of their history had not arrived.
 */
class CsvParserQuotingTest {

    @Test
    fun anUnescapedQuoteMidCellDoesNotSwallowTheFile() {
        // `2" off chest` is an ordinary note. Treating any quote as a field opener flipped the
        // parser into quote mode and consumed every later delimiter and newline into one cell.
        val text = "a,b\n" +
            "paused 2\" off chest,1\n" +
            "second row,2\n" +
            "third row,3\n"
        val rows = CsvParser.parse(text)
        assertEquals(4, rows.size)
        assertEquals(listOf("second row", "2"), rows[2])
        assertEquals(listOf("third row", "3"), rows[3])
    }

    @Test
    fun properlyQuotedFieldsStillWork() {
        val rows = CsvParser.parse("a,b\n\"x,y\",2\n")
        assertEquals(listOf("x,y", "2"), rows[1])
    }

    @Test
    fun escapedQuotesInsideAQuotedFieldStillWork() {
        val rows = CsvParser.parse("a\n\"he said \"\"hi\"\"\"\n")
        assertEquals(listOf("he said \"hi\""), rows[1])
    }

    @Test
    fun crlfIsOneRowBreak() {
        val rows = CsvParser.parse("a,b\r\n1,2\r\n")
        assertEquals(2, rows.size)
        assertEquals(listOf("1", "2"), rows[1])
    }

    @Test
    fun aLoneCarriageReturnAlsoEndsTheRow() {
        // CR-only files collapsed into a single row and imported as empty.
        val rows = CsvParser.parse("a,b\r1,2\r3,4")
        assertEquals(3, rows.size)
        assertEquals(listOf("1", "2"), rows[1])
        assertEquals(listOf("3", "4"), rows[2])
    }
}
