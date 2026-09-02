package com.forge.app.ui.checkin

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M-14: the check-in's weight field distinguishes "nothing typed" from "typed something the parser
 * rejects" BEFORE any write is launched. Only the first may be skipped silently.
 */
class CheckinWeightInputTest {

    private fun valid(text: String, unit: WeightUnit): Double {
        val result = classifyCheckinWeight(text, unit)
        assertTrue("expected a valid weigh-in for '$text' in $unit, got $result", result is CheckinWeightInput.Valid)
        return (result as CheckinWeightInput.Valid).lb
    }

    @Test
    fun blankIsBlankNotInvalid() {
        assertEquals(CheckinWeightInput.Blank, classifyCheckinWeight("", WeightUnit.LB))
        assertEquals(CheckinWeightInput.Blank, classifyCheckinWeight("   ", WeightUnit.KG))
        assertEquals(CheckinWeightInput.Blank, classifyCheckinWeight("", WeightUnit.ST))
    }

    @Test
    fun aNonblankValueOutsideTheRangeIsInvalidNotBlank() {
        // The audit's reproduction: a mis-typed "8" in pounds.
        assertEquals(CheckinWeightInput.Invalid, classifyCheckinWeight("8", WeightUnit.LB))
        assertEquals(CheckinWeightInput.Invalid, classifyCheckinWeight("5000", WeightUnit.LB))
        assertEquals(CheckinWeightInput.Invalid, classifyCheckinWeight("1", WeightUnit.KG))
        // One stone is 14 lb: nonblank, parseable, implausible.
        assertEquals(CheckinWeightInput.Invalid, classifyCheckinWeight("1", WeightUnit.ST))
        assertEquals(CheckinWeightInput.Invalid, classifyCheckinWeight("abc", WeightUnit.LB))
    }

    @Test
    fun poundsPassThroughAsStored() {
        assertEquals(180.0, valid("180", WeightUnit.LB), 0.001)
        assertEquals(172.5, valid("172.5", WeightUnit.LB), 0.001)
    }

    @Test
    fun kilogramsAreConvertedToStoredPounds() {
        assertEquals(80 * 2.20462, valid("80", WeightUnit.KG), 0.05)
    }

    @Test
    fun stonesAreConvertedToStoredPounds() {
        assertEquals(12 * 14.0, valid("12", WeightUnit.ST), 0.001)
        assertEquals(12.5 * 14.0, valid("12.5", WeightUnit.ST), 0.001)
    }

    @Test
    fun theSheetFlagsOnlyInvalidInput() {
        assertFalse("blank is not an error", CheckinViewModel.UiState(weightText = "").weightInvalid)
        assertFalse("a plausible weight is not an error", CheckinViewModel.UiState(weightText = "180").weightInvalid)
        assertTrue("the audit's typo is", CheckinViewModel.UiState(weightText = "8").weightInvalid)
        assertTrue(
            "and the unit is honoured: 8 is implausible in kg too",
            CheckinViewModel.UiState(weightText = "8", weightUnit = WeightUnit.KG).weightInvalid
        )
        assertFalse(
            "while 80 kg is a real weigh-in",
            CheckinViewModel.UiState(weightText = "80", weightUnit = WeightUnit.KG).weightInvalid
        )
    }
}
