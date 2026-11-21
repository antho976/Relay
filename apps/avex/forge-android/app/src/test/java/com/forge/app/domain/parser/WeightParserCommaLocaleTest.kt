package com.forge.app.domain.parser

import com.forge.app.program.ExerciseUnit
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * A weight typed on a comma-decimal keyboard must log the weight the user meant.
 *
 * Before this, "82,5" returned null from [WeightParser.parse], so the set was stored with
 * `weightLb = null` — treated as bodyweight. It contributed nothing to volume and could never
 * register a PR, while the row still displayed "82,5" back to the user.
 */
class WeightParserCommaLocaleTest {

    @Test
    fun commaDecimalParsesLikeAPeriod() {
        val comma = WeightParser.parse("82,5", ExerciseUnit.WEIGHT)
        val period = WeightParser.parse("82.5", ExerciseUnit.WEIGHT)
        assertEquals(period!!, comma!!, 0.0)
        assertEquals(82.5, comma, 0.0)
    }

    @Test
    fun commaDecimalWorksForPlateCounts() {
        // The plate field is seeded by formatPlateCount, which used the device locale and produced
        // "2,5" on exactly these devices — so this is the round trip that was broken.
        assertEquals(37.5, WeightParser.parse("2,5", ExerciseUnit.PLATES, plateLb = 15.0)!!, 0.0)
        assertEquals(37.5, WeightParser.parse("2,5 plates", ExerciseUnit.PLATES, plateLb = 15.0)!!, 0.0)
    }

    @Test
    fun commaDecimalWorksWithAnLbSuffix() {
        assertEquals(45.5, WeightParser.parse("45,5 lb", ExerciseUnit.WEIGHT)!!, 0.0)
    }

    @Test
    fun bodyweightAndGarbageStillParseToNull() {
        assertEquals(null, WeightParser.parse("bw", ExerciseUnit.BODYWEIGHT))
        assertEquals(null, WeightParser.parse(",", ExerciseUnit.WEIGHT))
        assertEquals(null, WeightParser.parse("abc", ExerciseUnit.WEIGHT))
    }
}
