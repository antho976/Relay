package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Comma-decimal locales (de, fr, es, it, pt, nl, most of Europe and South America) put a `,` on the
 * decimal keyboard. Every parser here reaches `toDoubleOrNull`, which is locale-independent and
 * takes only `.`, so "82,5" used to parse as null — and null is invisible, because the raw text is
 * still stored and shown back.
 */
class DecimalInputTest {

    @Test
    fun aLoneCommaIsTheDecimalSeparator() {
        assertEquals("82.5", normalizeDecimalInput("82,5"))
        assertEquals("0.5", normalizeDecimalInput("0,5"))
        assertEquals("2.5", normalizeDecimalInput(" 2,5 "))
    }

    @Test
    fun aPeriodIsLeftAlone() {
        assertEquals("82.5", normalizeDecimalInput("82.5"))
        assertEquals("100", normalizeDecimalInput("100"))
    }

    @Test
    fun withBothSeparatorsTheCommaIsGrouping() {
        assertEquals("1250.5", normalizeDecimalInput("1,250.5"))
        assertEquals("1250.75", normalizeDecimalInput("1,250.75"))
    }

    @Test
    fun filterAcceptsTheCommaKeyAndCanonicalisesIt() {
        assertEquals("82.5", filterDecimalInput("82,5"))
        assertEquals("82.5", filterDecimalInput("82.5"))
    }

    @Test
    fun filterStripsLettersAndCollapsesExtraSeparators() {
        assertEquals("75", filterDecimalInput("75kg"))
        // '7.5.2' must not slip through and surface as a misleading out-of-range error.
        assertEquals("7.52", filterDecimalInput("7.5.2"))
        assertEquals("7.52", filterDecimalInput("7,5,2"))
    }

    @Test
    fun filterHandlesEmptyAndSeparatorOnly() {
        assertEquals("", filterDecimalInput(""))
        assertEquals("", filterDecimalInput("abc"))
        assertEquals(".", filterDecimalInput(","))
    }
}
