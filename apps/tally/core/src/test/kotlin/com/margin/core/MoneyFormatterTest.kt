package com.margin.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.util.Locale

class MoneyFormatterTest {

    private val enCa = MoneyFormatter("CAD", Locale.CANADA)
    private val frCa = MoneyFormatter("CAD", Locale.CANADA_FRENCH)
    private val jpy = MoneyFormatter("JPY", Locale.JAPAN)

    @Test fun `formats cents in English Canada`() {
        assertEquals("$1,284.50", enCa.format(128_450))
        assertEquals("$0.05", enCa.format(5))
    }

    @Test fun `French Canada puts the symbol after the number`() {
        val s = frCa.format(128_450)
        assert(s.endsWith("$")) { s }
        assert(s.contains("284,50")) { s }
    }

    @Test fun `negative amounts use a true minus sign`() {
        assertEquals("−$12.00", enCa.format(-1_200))
    }

    @Test fun `whole formatting rounds half even`() {
        assertEquals("$12", enCa.formatWhole(1_250))
        assertEquals("$14", enCa.formatWhole(1_350))
    }

    @Test fun `compact switches to k at ten thousand`() {
        assertEquals("$9,999", enCa.formatCompact(999_900))
        assertEquals("$12.4k", enCa.formatCompact(1_243_100))
        assertEquals("$1.5M", enCa.formatCompact(150_000_000))
    }

    @Test fun `compact keeps the French symbol on its side`() {
        val s = frCa.formatCompact(1_243_100)
        assert(s.contains("12,4k")) { s }
        assert(s.endsWith("$")) { s }
    }

    @Test fun `zero-decimal currencies have no cents`() {
        assertEquals(0, jpy.fractionDigits)
        assertEquals(1500L, jpy.parse("1500"))
    }

    @Test fun `parses what people type`() {
        assertEquals(1_250L, enCa.parse("12.5"))
        assertEquals(1_250L, enCa.parse("12,50"))
        assertEquals(123_456L, enCa.parse("$1,234.56"))
        assertEquals(123_456L, enCa.parse("1 234,56 $"))
        assertEquals(123_400L, enCa.parse("1,234"))
        assertEquals(1_200L, enCa.parse("12."))
        assertNull(enCa.parse(""))
        assertNull(enCa.parse("abc"))
    }

    @Test fun `input formatting round trips through parse`() {
        listOf(0L, 5L, 1_250L, 99_999_99L).forEach { minor ->
            assertEquals(minor, enCa.parse(enCa.formatInput(minor)))
        }
    }

    @Test fun `signed formatting marks income`() {
        assertEquals("+$1,200.00", enCa.formatSigned(120_000))
        assertEquals("$0.00", enCa.formatSigned(0))
        assertEquals("\u2212$5.00", enCa.formatSigned(-500))
    }

    @Test fun `unknown currency codes fall back to CAD instead of crashing`() {
        assertEquals("CAD", MoneyFormatter("NOPE", Locale.CANADA).currency.currencyCode)
    }
}
