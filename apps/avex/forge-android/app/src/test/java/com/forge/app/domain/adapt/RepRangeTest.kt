package com.forge.app.domain.adapt

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * What counts as a rep range.
 *
 * The parser used to scrape every digit out of the string, which made its own doc comment untrue:
 * "Null when the text holds no digits (pure-AMRAP/timed entries stay suggestion-free)". A timed
 * hold's target IS digits — the library prescribes a plank as "30-60s" — so it parsed as 30..60
 * REPS. The advisor then congratulated a lifter for clearing the top of a rep range they had never
 * been in and told them to add a rep to a hold, and the suggestion calibrator compared their hold
 * length against it as a rep count.
 */
class RepRangeTest {

    @Test
    fun `a plain count is a range of one`() {
        assertEquals(RepRange(15, 15), RepRange.parse("15"))
    }

    @Test
    fun `a hyphenated range parses both ends`() {
        assertEquals(RepRange(8, 12), RepRange.parse("8-12"))
        assertEquals(RepRange(10, 15), RepRange.parse("10-15"))
    }

    @Test
    fun `the per-side forms the library actually uses still parse`() {
        // These are real defaultReps values — losing them would silently switch off rep
        // progression for every unilateral movement.
        assertEquals(RepRange(10, 10), RepRange.parse("10/leg"))
        assertEquals(RepRange(12, 15), RepRange.parse("12-15/leg"))
        assertEquals(RepRange(8, 10), RepRange.parse("8-10/leg"))
    }

    @Test
    fun `a hold time is not a rep range`() {
        // Every timed entry in the library, verbatim.
        assertNull(RepRange.parse("30-60s"))
        assertNull(RepRange.parse("20-40s"))
        assertNull(RepRange.parse("20-45s"))
    }

    @Test
    fun `text that is not a rep count at all yields nothing`() {
        assertNull(RepRange.parse("AMRAP"))
        assertNull(RepRange.parse(""))
        assertNull(RepRange.parse("as many as you can"))
    }

    @Test
    fun `a set-times-reps prescription is not a range either`() {
        // "3 x 10" used to scrape to 3..10, which is neither of the two numbers in it.
        assertNull(RepRange.parse("3 x 10"))
    }

    @Test
    fun `surrounding whitespace is tolerated`() {
        assertEquals(RepRange(8, 12), RepRange.parse("  8-12 "))
        assertEquals(RepRange(8, 12), RepRange.parse("8 - 12"))
    }
}
