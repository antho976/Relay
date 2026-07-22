package com.forge.app.domain.cardio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Coverage for the per-type optional field mapping (GYMAP-38). The mapping decides which extra field
 * (incline / laps / elevation) each activity surfaces AND is what the save path gates on — so if a
 * type maps to the wrong set, a user would either lose access to a relevant field or have an
 * irrelevant one persisted. Every built-in type must resolve to a defined set (never crash).
 */
class CardioOptionalFieldsTest {

    @Test
    fun beltMachines_surfaceIncline() {
        assertEquals(setOf(CardioField.INCLINE), optionalFieldsFor(CardioType.TREADMILL))
        assertEquals(setOf(CardioField.INCLINE), optionalFieldsFor(CardioType.ELLIPTICAL))
    }

    @Test
    fun swim_surfacesLaps() {
        assertEquals(setOf(CardioField.LAPS), optionalFieldsFor(CardioType.SWIM))
    }

    @Test
    fun outdoorDistanceWork_surfacesElevation() {
        listOf(CardioType.RUN, CardioType.WALK, CardioType.HIKE, CardioType.CYCLE).forEach {
            assertEquals("$it surfaces elevation", setOf(CardioField.ELEVATION), optionalFieldsFor(it))
        }
    }

    @Test
    fun restAndSteadyTypes_surfaceNothing() {
        listOf(CardioType.REST, CardioType.HIIT, CardioType.YOGA, CardioType.ROW, CardioType.OTHER).forEach {
            assertTrue("$it surfaces no per-type field", optionalFieldsFor(it).isEmpty())
        }
    }

    @Test
    fun everyBuiltinType_hasADefinedSet() {
        // A missing branch would throw; walk them all so the mapping can never go partial.
        CardioType.entries.forEach { optionalFieldsFor(it) }
    }

    @Test
    fun builtinActivity_delegatesToTypeMapping() {
        assertEquals(
            setOf(CardioField.INCLINE),
            CardioActivity.Builtin(CardioType.TREADMILL).optionalFields
        )
        assertEquals(
            setOf(CardioField.ELEVATION),
            CardioActivity.Builtin(CardioType.HIKE).optionalFields
        )
    }

    @Test
    fun customActivity_surfacesNoPerTypeField() {
        val custom = CardioActivity.Custom(CustomCardioType(code = "custom_1", name = "Stairs", glyphKey = "run"))
        assertTrue(custom.optionalFields.isEmpty())
    }

    @Test
    fun formatInclinePct_dropsTrailingZero() {
        assertEquals("6%", formatInclinePct(6.0))
        assertEquals("6.5%", formatInclinePct(6.5))
        assertEquals("0%", formatInclinePct(0.0))
        assertEquals("12%", formatInclinePct(12.0))
    }
}
