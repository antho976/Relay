package com.forge.app.domain.parser

import com.forge.app.program.ExerciseUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WeightParserTest {

    @Test
    fun plainNumberParsesToPounds() {
        assertEquals(45.0, WeightParser.parse("45", ExerciseUnit.DUMBBELL)!!, 0.0)
        assertEquals(45.5, WeightParser.parse("45.5", ExerciseUnit.DUMBBELL)!!, 0.0)
    }

    @Test
    fun bodyweightAndBlankParseToNull() {
        assertNull(WeightParser.parse("BW", ExerciseUnit.BODYWEIGHT))
        assertNull(WeightParser.parse("bw", ExerciseUnit.BODYWEIGHT))
        assertNull(WeightParser.parse("", ExerciseUnit.DUMBBELL))
        assertNull(WeightParser.parse("   ", ExerciseUnit.DUMBBELL))
    }

    @Test
    fun plateNotationMultipliesByPlateWeight() {
        assertEquals(30.0, WeightParser.parse("2 plates", ExerciseUnit.PLATES)!!, 0.0)
        assertEquals(15.0, WeightParser.parse("1 plate", ExerciseUnit.PLATES)!!, 0.0)
        assertEquals(45.0, WeightParser.parse("3p", ExerciseUnit.PLATES)!!, 0.0)
    }

    @Test
    fun lbSuffixIsTolerated() {
        assertEquals(30.0, WeightParser.parse("30 lb", ExerciseUnit.DUMBBELL)!!, 0.0)
        assertEquals(30.0, WeightParser.parse("30lb", ExerciseUnit.DUMBBELL)!!, 0.0)
    }

    @Test
    fun bareNumberIsAPlateCountWhenUnitIsPlates() {
        // On a plate-loaded exercise the field is labelled PLATES, so a bare "2" = 2 plates.
        assertEquals(30.0, WeightParser.parse("2", ExerciseUnit.PLATES)!!, 0.0)
        // A custom plate weight scales the count.
        assertEquals(20.0, WeightParser.parse("2", ExerciseUnit.PLATES, plateLb = 10.0)!!, 0.0)
        // An explicit lb suffix still forces pounds even on a plate exercise.
        assertEquals(35.0, WeightParser.parse("35 lb", ExerciseUnit.PLATES)!!, 0.0)
    }

    @Test
    fun garbageParsesToNull() {
        assertNull(WeightParser.parse("heavy", ExerciseUnit.DUMBBELL))
    }
}
