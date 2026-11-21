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

    // ── Seeded edit fields (audit 2026-09-26, 05 and 06) ─────────────────────

    @Test
    fun anUntouchedSeedKeepsTheStoredValue() {
        // A 10.047 km cardio distance seeds as "10.0"; a note-only edit must not store 10.0.
        val km = 10.047
        val seed = distanceInputValue(km, useMiles = false)
        assertEquals(km, storedUnlessEdited(seed, seed, km) { parseToKm(it, false) }!!, 0.0)
        // A 225 lb goal seeds as "102.1" kg; re-saving it untouched keeps 225, not 225.09.
        val lb = 225.0
        val kgSeed = weightInputValue(lb, WeightUnit.KG)
        assertEquals(lb, storedUnlessEdited(kgSeed, kgSeed, lb) { parseToLb(it, WeightUnit.KG) }!!, 0.0)
    }

    @Test
    fun anEditedFieldIsParsed() {
        assertEquals(5.0, storedUnlessEdited("5", "10.0", 10.047) { parseToKm(it, false) }!!, 1e-9)
        assertEquals(null, storedUnlessEdited("", "10.0", 10.047) { parseToKm(it, false) })
        // No stored value (a new entry): the text is all there is.
        assertEquals(82.5, storedUnlessEdited(filterDecimalInput("82,5"), null, null) { it.toDoubleOrNull() }!!, 1e-9)
    }
}
