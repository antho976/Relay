package com.forge.app.domain.cardio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Coverage for the cardio conditions codec (GYMAP-39). The encode/decode pair round-trips the
 * multi-select weather tags through the `cardio_entry.conditions` string column, so a bug here would
 * silently drop or reorder a logged session's tags. Order is normalised to declaration order and
 * unknown / blank codes are dropped, so an old or corrupt value never crashes a render.
 */
class CardioConditionTest {

    @Test
    fun encodeThenDecode_roundTrips() {
        val selection = setOf(CardioCondition.HOT, CardioCondition.WIND)
        val stored = CardioCondition.encode(selection)
        assertEquals(selection, CardioCondition.decode(stored))
    }

    @Test
    fun encode_empty_isNull() {
        assertNull(CardioCondition.encode(emptySet()))
    }

    @Test
    fun decode_nullOrBlank_isEmpty() {
        assertTrue(CardioCondition.decode(null).isEmpty())
        assertTrue(CardioCondition.decode("").isEmpty())
        assertTrue(CardioCondition.decode("   ").isEmpty())
    }

    @Test
    fun encode_normalisesToDeclarationOrder() {
        // Tapped in a scrambled order, but stored (and re-read) in declaration order (Hot·Cold·Rain·Wind).
        val stored = CardioCondition.encode(setOf(CardioCondition.WIND, CardioCondition.HOT, CardioCondition.RAIN))
        assertEquals("hot,rain,wind", stored)
    }

    @Test
    fun decode_dropsUnknownAndBlankCodes() {
        // A stray / retired code (e.g. from a future build) is skipped, not rendered as a raw string.
        assertEquals(
            setOf(CardioCondition.HOT, CardioCondition.RAIN),
            CardioCondition.decode("hot, ,bogus,rain")
        )
    }

    @Test
    fun fromCode_mapsKnownAndRejectsUnknown() {
        assertEquals(CardioCondition.COLD, CardioCondition.fromCode("cold"))
        assertNull(CardioCondition.fromCode("snow"))
        assertNull(CardioCondition.fromCode(null))
    }
}
