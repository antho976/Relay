package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Test

class AmountInputTest {

    @Test fun `typing digits and a decimal`() {
        val a = AmountInput().digit(1).digit(2).decimal().digit(5)
        assertEquals("12.5", a.text)
        assertEquals(1_250L, a.minor)
    }

    @Test fun `decimals stop at the currency's digits`() {
        val a = AmountInput().digit(1).decimal().digit(2).digit(3).digit(4)
        assertEquals("1.23", a.text)
    }

    @Test fun `no leading zeros and a bare decimal gets a zero`() {
        assertEquals("5", AmountInput().digit(0).digit(5).text)
        assertEquals("0.", AmountInput().decimal().text)
    }

    @Test fun `zero-decimal currencies ignore the decimal key`() {
        assertEquals("12", AmountInput(fractionDigits = 0).digit(1).digit(2).decimal().text)
    }

    @Test fun `integer digits are capped`() {
        var a = AmountInput()
        repeat(20) { a = a.digit(9) }
        assertEquals(AmountInput.MAX_INTEGER_DIGITS, a.text.length)
    }

    @Test fun `backspace and clear`() {
        val a = AmountInput().digit(4).digit(2)
        assertEquals("4", a.backspace().text)
        assertEquals(true, a.clear().isEmpty)
    }

    @Test fun `of minor units reads back the same`() {
        listOf(1L, 50L, 1_250L, 10_000L, 123_456_789L).forEach {
            assertEquals(it, AmountInput.of(it, 2).minor)
        }
        assertEquals("12.5", AmountInput.of(1_250, 2).text)
    }
}
